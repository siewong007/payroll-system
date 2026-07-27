//! The only sanctioned way to fetch a client-supplied URL from inside the API
//! container.
//!
//! `calendar_service::import_from_ics` used to hand the raw `url` field of an
//! `ImportIcsRequest` to a bare `reqwest::Client::new()`. That gave anyone
//! holding `ManageCalendar` a request-forgery primitive with every amplifier
//! attached: no scheme check (so `http://169.254.169.254/latest/meta-data/`
//! reached the instance metadata service), no address policy (so `10.x` and
//! `127.0.0.1:<port>` were reachable), redirects followed by default (so a
//! public host could bounce the fetch into either), no timeout, and
//! `Response::text()` buffering an unbounded body into memory.
//!
//! Worse than any of those individually: both error arms interpolated the
//! transport error into the response, so connection-refused, connection-timeout
//! and TLS-handshake-failure were distinguishable to the caller. That is a blind
//! internal port scanner. Every unreachable outcome here therefore collapses to
//! `UNREACHABLE`, with the real cause logged at `warn!` where an operator —
//! and only an operator — can see it.
//!
//! Shaped like [`crate::core::upload_path`]: a policy module the call sites must
//! go through, so no future caller can assemble the unsafe thing itself.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use url::{Host, Url};

use crate::core::error::{AppError, AppResult, payload_too_large};

/// Largest calendar this system will accept, whether fetched by URL or uploaded.
///
/// A national holiday feed is a few kilobytes; two megabytes is generous for a
/// decade of them. `handlers::calendar::ICS_FILE_MAX_BYTES` aliases this so the
/// two ingest paths cannot drift apart.
pub const MAX_ICS_BYTES: usize = 2 * 1024 * 1024;

/// Whole-request ceiling, connect included.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Separate and shorter, so a filtered (silently dropped) address fails fast
/// instead of holding a worker for the full request budget.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The single text every transport-level and non-2xx outcome reports.
///
/// Uniformity is the point: a caller must not be able to tell a refused
/// connection from a timeout from a 404 from a redirect, because that
/// distinction is what makes an SSRF primitive into a port scanner. The timeout
/// and the size cap below do not close that on their own.
const UNREACHABLE: &str = "Could not fetch the calendar URL.";

fn unreachable_error() -> AppError {
    AppError::BadGateway(UNREACHABLE.to_string())
}

/// Addresses no tenant-supplied URL may resolve to.
///
/// `std`'s `is_shared`/`is_benchmarking`/`is_reserved`/`is_unique_local`/
/// `is_unicast_link_local` are all still unstable, so the blocks they would
/// cover are written out as explicit octet and segment predicates rather than
/// left out.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();

    // The blocks with no stable predicate of their own: 0/8 "this network",
    // 100.64/10 carrier-grade NAT, 192.0.0/24 IETF protocol assignments,
    // 198.18/15 benchmarking, and 240/4 reserved — which subsumes the
    // 255.255.255.255 broadcast address.
    let reserved = a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 198 && (18..=19).contains(&b))
        || a >= 240;

    // `is_link_local` is 169.254/16, where every cloud provider parks its
    // instance metadata service — the address the report named.
    reserved
        || ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
}

fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    // `::ffff:169.254.169.254` and `::ffff:127.0.0.1` are otherwise a clean
    // bypass of the v4 policy, and both are dialable.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }

    let segments = ip.segments();

    // `::a.b.c.d`, the deprecated IPv4-compatible form. Deprecated is not the
    // same as unroutable, and `::1` lands here too (as 0.0.0.1, still blocked).
    if segments[..6].iter().all(|&s| s == 0) {
        let bits = (u32::from(segments[6]) << 16) | u32::from(segments[7]);
        return is_blocked_v4(Ipv4Addr::from(bits));
    }

    // fc00::/7 unique local and fe80::/10 link local, again unstable in `std`.
    let seg0 = segments[0];
    let unique_or_link_local = (seg0 & 0xfe00) == 0xfc00 || (seg0 & 0xffc0) == 0xfe80;

    unique_or_link_local || ip.is_unspecified() || ip.is_loopback() || ip.is_multicast()
}

/// Parse a tenant-supplied URL, or say precisely why it is not fetchable.
///
/// These rejections are deliberately specific where the `UNREACHABLE` ones are
/// not: every one of them is decidable from the string alone, so answering them
/// honestly tells the caller nothing about the network they cannot already work
/// out offline — and an operator pasting an `http://` feed deserves to be told
/// that, not "could not fetch".
pub fn parse_public_https_url(raw: &str) -> AppResult<Url> {
    let url = Url::parse(raw.trim())
        .map_err(|_| AppError::BadRequest("The calendar URL is not a valid URL.".into()))?;

    // Also what kills `file:///etc/passwd`, `gopher://` and every other scheme
    // the underlying client might otherwise be willing to dial.
    if url.scheme() != "https" {
        return Err(AppError::BadRequest(
            "The calendar URL must start with https://".into(),
        ));
    }

    // `https://trusted.example@169.254.169.254/` reads as a trusted host to a
    // human and resolves to the metadata service.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::BadRequest(
            "The calendar URL must not contain a username or password.".into(),
        ));
    }

    match url.host() {
        Some(Host::Domain(_)) => Ok(url),
        Some(Host::Ipv4(v4)) if !is_blocked_v4(v4) => Ok(url),
        Some(Host::Ipv6(v6)) if !is_blocked_v6(v6) => Ok(url),
        Some(_) => Err(internal_address()),
        None => Err(AppError::BadRequest("The calendar URL has no host.".into())),
    }
}

fn internal_address() -> AppError {
    AppError::BadRequest("The calendar URL resolves to an internal address.".into())
}

/// Fetch a public HTTPS URL as text, refusing anything that could reach inside.
///
/// The DNS lookup happens here rather than inside the client so that *every*
/// A/AAAA record can be vetted — checking only the first is a one-extra-record
/// dodge — and so the vetted addresses can be pinned with `resolve_to_addrs`.
/// Pinning is what removes the rebinding window between the check and the
/// connect: without it the name is resolved a second time by the connector and
/// may answer differently.
pub async fn fetch_public_text(raw: &str, max_bytes: usize) -> AppResult<String> {
    let url = parse_public_https_url(raw)?;
    let port = url.port_or_known_default().unwrap_or(443);

    let (domain, addrs) = match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.to_string();
            let resolved: Vec<SocketAddr> = tokio::net::lookup_host((domain.as_str(), port))
                .await
                .map_err(|e| {
                    tracing::warn!("ICS fetch: could not resolve {}: {}", domain, e);
                    unreachable_error()
                })?
                .collect();
            (Some(domain), resolved)
        }
        // Already vetted by `parse_public_https_url`; re-checked below anyway so
        // the policy has exactly one enforcement point.
        Some(Host::Ipv4(v4)) => (None, vec![SocketAddr::new(IpAddr::V4(v4), port)]),
        Some(Host::Ipv6(v6)) => (None, vec![SocketAddr::new(IpAddr::V6(v6), port)]),
        None => return Err(unreachable_error()),
    };

    if addrs.is_empty() {
        tracing::warn!("ICS fetch: {} resolved to no addresses", url);
        return Err(unreachable_error());
    }
    if addrs.iter().any(|addr| is_blocked_ip(addr.ip())) {
        return Err(internal_address());
    }

    // `Policy::none()` is load-bearing: reqwest follows up to ten redirects by
    // default, so a public host answering 302 with a link-local `Location`
    // defeats the whole address policy in one hop.
    let mut builder = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    if let Some(domain) = &domain {
        builder = builder.resolve_to_addrs(domain, &addrs);
    }
    let client = builder.build().map_err(|e| {
        tracing::warn!("ICS fetch: could not build the HTTP client: {}", e);
        unreachable_error()
    })?;

    let mut response = client.get(url.clone()).send().await.map_err(|e| {
        tracing::warn!("ICS fetch of {} failed: {}", url, e);
        unreachable_error()
    })?;

    // 3xx lands here now that redirects are off, and reports as unreachable
    // rather than as a redirect — same reason as everything else.
    if !response.status().is_success() {
        tracing::warn!("ICS fetch of {} returned {}", url, response.status());
        return Err(unreachable_error());
    }

    // Cheap pre-check only: the header is attacker-controlled and absent under
    // chunked encoding, so the accumulator below is the real ceiling.
    if response
        .content_length()
        .is_some_and(|len| len > max_bytes as u64)
    {
        return Err(payload_too_large("The calendar", max_bytes));
    }

    // The real ceiling, and the fix for the unbounded `text()` that preceded it:
    // the accumulator is checked before every append, so a hostile server
    // streaming for ever is cut off at `max_bytes` rather than at OOM.
    let mut body: Vec<u8> = Vec::new();
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!("ICS fetch of {} failed mid-body: {}", url, e);
                return Err(unreachable_error());
            }
        };

        if body.len() + chunk.len() > max_bytes {
            return Err(payload_too_large("The calendar", max_bytes));
        }
        body.extend_from_slice(&chunk);
    }

    String::from_utf8(body)
        .map_err(|_| AppError::BadRequest("The calendar is not valid UTF-8 text.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(raw: &str) -> IpAddr {
        raw.parse().expect("test address should parse")
    }

    #[test]
    fn public_addresses_are_allowed() {
        for public in [
            "8.8.8.8",
            "1.1.1.1",
            "142.250.185.68",
            "203.0.114.9",
            "2001:4860:4860::8888",
        ] {
            assert!(!is_blocked_ip(ip(public)), "blocked {public}");
        }
    }

    #[test]
    fn every_private_and_reserved_block_is_refused() {
        for blocked in [
            "0.0.0.0",
            "0.1.2.3",
            "127.0.0.1",
            "127.1.2.3",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.1",
            // The reported exploit: the cloud instance metadata service.
            "169.254.169.254",
            "100.64.0.1",
            "192.0.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
        ] {
            assert!(is_blocked_ip(ip(blocked)), "allowed {blocked}");
        }
    }

    #[test]
    fn ipv6_private_and_link_local_blocks_are_refused() {
        for blocked in [
            "::",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "fe80::dead:beef",
            "ff02::1",
        ] {
            assert!(is_blocked_ip(ip(blocked)), "allowed {blocked}");
        }
    }

    #[test]
    fn the_ipv4_mapped_form_is_not_a_bypass() {
        // `::ffff:169.254.169.254` dials the same metadata service as the bare
        // v4 literal; a v6-only policy would wave it straight through.
        for blocked in [
            "::ffff:169.254.169.254",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::169.254.169.254",
        ] {
            assert!(is_blocked_ip(ip(blocked)), "allowed {blocked}");
        }
        assert!(!is_blocked_ip(ip("::ffff:8.8.8.8")));
    }

    #[test]
    fn only_https_urls_with_a_public_host_are_accepted() {
        for allowed in [
            "https://calendar.google.com/calendar/ical/x/basic.ics",
            // Scheme comparison is on the parsed (lower-cased) scheme, as
            // `upload_path::has_http_scheme` already does for stored URLs.
            "HTTPS://Example.com/holidays.ics",
            "  https://example.com/x.ics  ",
            // A non-standard port is legitimate on a public host, so there is
            // deliberately no port allow-list.
            "https://example.com:8443/x.ics",
            "https://8.8.8.8/x.ics",
        ] {
            assert!(
                parse_public_https_url(allowed).is_ok(),
                "rejected legitimate url {allowed:?}"
            );
        }
    }

    #[test]
    fn hostile_urls_are_rejected_before_any_socket_is_opened() {
        for hostile in [
            "http://example.com/x.ics",
            "http://169.254.169.254/latest/meta-data/",
            "https://169.254.169.254/latest/meta-data/",
            "https://127.0.0.1:6379/",
            "https://10.0.0.1/x.ics",
            "https://[::1]/x.ics",
            "https://[::ffff:169.254.169.254]/",
            "file:///etc/passwd",
            "gopher://127.0.0.1:11211/",
            "ftp://example.com/x.ics",
            "https://trusted.example@169.254.169.254/",
            "https://user:pass@example.com/x.ics",
            "example.com/x.ics",
            "",
        ] {
            assert!(
                parse_public_https_url(hostile).is_err(),
                "accepted hostile url {hostile:?}"
            );
        }
    }

    #[test]
    fn every_unreachable_outcome_reports_one_uniform_text() {
        // DNS failure, refused connection, TLS failure, non-2xx and a redirect
        // all construct their error through `unreachable_error()`, and this
        // pins what it says. A regression reintroducing `format!("…{e}")` on
        // any of those arms reopens the port-scan oracle, and fails here.
        let (status, message) = unreachable_error().client_response();
        assert_eq!(status, axum::http::StatusCode::BAD_GATEWAY);
        assert_eq!(message, UNREACHABLE);
        assert!(
            !message.contains("connect") && !message.contains("refused"),
            "the uniform text must not describe the transport outcome"
        );
    }
}
