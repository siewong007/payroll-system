//! IP prefixes, for deciding whether a client sits on an approved network.
//!
//! Hand-rolled rather than pulled from a crate because the whole type is thirty
//! lines of bit masking and the interesting part is the *policy*: which
//! addresses may be treated as identifying a place, and how wide a block an
//! administrator is allowed to approve. Both belong under test in this repo,
//! not in a dependency's changelog.
//!
//! Two rules carry the security weight here:
//!
//! 1. **Only globally-routable addresses identify a network.** A private,
//!    loopback, link-local or carrier-NAT address is either shared by an entire
//!    ISP or an artefact of our own proxy chain. `TRUST_PROXY_HEADERS=false`
//!    behind a local reverse proxy resolves *every* client to the same
//!    `172.x.x.x`; approving that would put the whole internet inside the
//!    office. [`is_identifying`] is what stops it.
//!
//! 2. **A prefix has a floor.** Approving `0.0.0.0/0` — or, less obviously, a
//!    `/8` — silently disables the control while the UI still says "enforced".
//!    [`MIN_IPV4_PREFIX_LEN`] and [`MIN_IPV6_PREFIX_LEN`] bound it.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::core::error::{AppError, AppResult};

/// Widest IPv4 block an administrator may approve: 4,096 addresses.
///
/// A real office egress is a single static address or a small allocation; an
/// ISP that rotates a customer across anything wider is not identifying a place
/// at all, and the geofence is the right control for that tenant.
pub const MIN_IPV4_PREFIX_LEN: u8 = 20;

/// Widest IPv6 block an administrator may approve.
///
/// Far more permissive than the IPv4 floor by address count, and deliberately
/// so: a site is *delegated* a /48 or /56, and the client rotates its address
/// within that on every privacy-extension refresh. Matching narrower than the
/// delegation would fail on Tuesday for a client that matched on Monday.
pub const MIN_IPV6_PREFIX_LEN: u8 = 48;

/// A network address plus a prefix length — `203.0.113.0/24`, `2001:db8::/56`.
///
/// Always canonical: host bits are cleared on construction, so `203.0.113.5/24`
/// and `203.0.113.0/24` are the same value and cannot both be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpPrefix {
    network: IpAddr,
    prefix_len: u8,
}

impl IpPrefix {
    /// Build from an address and prefix length, masking off the host bits.
    ///
    /// Rejects a prefix length longer than the address family allows. Does not
    /// apply the [`MIN_IPV4_PREFIX_LEN`] policy floor — that is
    /// [`Self::parse_approvable`], so the learning pipeline can still *describe*
    /// an observation it would refuse to approve.
    pub fn new(addr: IpAddr, prefix_len: u8) -> AppResult<Self> {
        let max = Self::max_prefix_len(&addr);
        if prefix_len > max {
            return Err(AppError::BadRequest(format!(
                "A /{prefix_len} prefix is not valid for this address family (maximum /{max})"
            )));
        }
        Ok(Self {
            network: mask(canonical(addr), prefix_len),
            prefix_len,
        })
    }

    /// Parse `"203.0.113.0/24"`, or a bare address as its own host route
    /// (`"203.0.113.5"` → `203.0.113.5/32`).
    pub fn parse(value: &str) -> AppResult<Self> {
        let value = value.trim();
        let (addr_part, len_part) = match value.split_once('/') {
            Some((addr, len)) => (addr, Some(len)),
            None => (value, None),
        };

        let addr: IpAddr = addr_part.parse().map_err(|_| {
            AppError::BadRequest(format!("'{addr_part}' is not a valid IP address"))
        })?;
        let addr = canonical(addr);

        let prefix_len = match len_part {
            Some(len) => len.trim().parse::<u8>().map_err(|_| {
                AppError::BadRequest(format!("'{len}' is not a valid prefix length"))
            })?,
            None => Self::max_prefix_len(&addr),
        };

        Self::new(addr, prefix_len)
    }

    /// Parse *and* apply the approval policy: the address must identify a
    /// network, and the block must not be wider than the family's floor.
    ///
    /// This is the constructor every admin-supplied value goes through.
    pub fn parse_approvable(value: &str) -> AppResult<Self> {
        let prefix = Self::parse(value)?;

        if !is_identifying(prefix.network) {
            return Err(AppError::BadRequest(format!(
                "{} is a private, loopback, link-local or carrier-NAT range. \
                 It is shared by many networks, so it cannot identify your office.",
                prefix.network
            )));
        }

        let floor = prefix.policy_floor();
        if prefix.prefix_len < floor {
            return Err(AppError::BadRequest(format!(
                "/{} is too broad to identify one office — use /{floor} or narrower.",
                prefix.prefix_len
            )));
        }

        Ok(prefix)
    }

    /// Whether `addr` falls inside this prefix.
    ///
    /// Mixed families never match: a v4 client is not inside a v6 office block,
    /// and vice versa. IPv4-mapped v6 addresses are canonicalised first, so a
    /// client arriving as `::ffff:203.0.113.5` still matches `203.0.113.0/24`.
    pub fn contains(&self, addr: IpAddr) -> bool {
        let addr = canonical(addr);
        match (self.network, addr) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => mask4(ip, self.prefix_len) == net,
            (IpAddr::V6(net), IpAddr::V6(ip)) => mask6(ip, self.prefix_len) == net,
            _ => false,
        }
    }

    pub fn network(&self) -> IpAddr {
        self.network
    }

    pub fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    /// Number of addresses this prefix covers, saturating for the enormous
    /// IPv6 blocks where the exact figure is not useful to anyone.
    pub fn address_count(&self) -> u128 {
        let host_bits = Self::max_prefix_len(&self.network) - self.prefix_len;
        1u128.checked_shl(host_bits as u32).unwrap_or(u128::MAX)
    }

    /// Whether this prefix covers exactly one address.
    pub fn is_host_route(&self) -> bool {
        self.prefix_len == Self::max_prefix_len(&self.network)
    }

    fn policy_floor(&self) -> u8 {
        match self.network {
            IpAddr::V4(_) => MIN_IPV4_PREFIX_LEN,
            IpAddr::V6(_) => MIN_IPV6_PREFIX_LEN,
        }
    }

    fn max_prefix_len(addr: &IpAddr) -> u8 {
        match addr {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        }
    }
}

impl fmt::Display for IpPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix_len)
    }
}

/// Unwrap an IPv4-mapped IPv6 address (`::ffff:a.b.c.d`) to plain IPv4.
///
/// A dual-stack listener reports v4 clients in this form, so without it the
/// same physical office would need both a v4 and a v6 entry — and, worse, the
/// v4 policy floor would not apply to the v6-shaped copy.
pub fn canonical(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        v4 => v4,
    }
}

/// Whether an address can stand for a *place*.
///
/// False for every range that is either reused by unrelated networks (private,
/// carrier NAT, link-local) or an artefact of our own infrastructure
/// (loopback, and the private addresses a local reverse proxy presents when
/// `TRUST_PROXY_HEADERS` is off). Approving one of those would admit every
/// client on earth.
///
/// The documentation ranges (`192.0.2.0/24` and friends) are deliberately
/// *not* excluded: nothing routes them, so they are harmless in production and
/// they keep the test fixtures in this repo readable.
pub fn is_identifying(addr: IpAddr) -> bool {
    match canonical(addr) {
        IpAddr::V4(v4) => is_identifying_v4(v4),
        IpAddr::V6(v6) => is_identifying_v6(v6),
    }
}

fn is_identifying_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();

    // Shared or non-routable by definition.
    if ip.is_private() || ip.is_loopback() || ip.is_link_local() {
        return false;
    }
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() {
        return false;
    }
    // 100.64.0.0/10 — carrier-grade NAT. The single most dangerous range to
    // approve: one mobile carrier's entire subscriber base can share it, so
    // "on the office network" would include every phone on that network.
    if a == 100 && (64..=127).contains(&b) {
        return false;
    }
    // 0.0.0.0/8 "this network", and 240.0.0.0/4 reserved.
    if a == 0 || a >= 240 {
        return false;
    }
    true
}

fn is_identifying_v6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    let segments = ip.segments();
    // fe80::/10 link-local.
    if segments[0] & 0xffc0 == 0xfe80 {
        return false;
    }
    // fc00::/7 unique local — the v6 equivalent of RFC1918.
    if segments[0] & 0xfe00 == 0xfc00 {
        return false;
    }
    true
}

fn mask(addr: IpAddr, prefix_len: u8) -> IpAddr {
    match addr {
        IpAddr::V4(v4) => IpAddr::V4(mask4(v4, prefix_len)),
        IpAddr::V6(v6) => IpAddr::V6(mask6(v6, prefix_len)),
    }
}

fn mask4(ip: Ipv4Addr, prefix_len: u8) -> Ipv4Addr {
    // A shift of 32 is undefined in Rust as much as in C, and `/0` is exactly
    // the case that would hit it.
    let bits = u32::from(ip);
    let masked = match prefix_len {
        0 => 0,
        n if n >= 32 => bits,
        n => bits & (!0u32 << (32 - n)),
    };
    Ipv4Addr::from(masked)
}

fn mask6(ip: Ipv6Addr, prefix_len: u8) -> Ipv6Addr {
    let bits = u128::from(ip);
    let masked = match prefix_len {
        0 => 0,
        n if n >= 128 => bits,
        n => bits & (!0u128 << (128 - n)),
    };
    Ipv6Addr::from(masked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("valid ip")
    }

    fn prefix(value: &str) -> IpPrefix {
        IpPrefix::parse(value).expect("valid prefix")
    }

    #[test]
    fn a_bare_address_parses_as_its_own_host_route() {
        assert_eq!(prefix("203.0.113.5").to_string(), "203.0.113.5/32");
        assert_eq!(prefix("2001:db8::1").to_string(), "2001:db8::1/128");
        assert!(prefix("203.0.113.5").is_host_route());
    }

    #[test]
    fn host_bits_are_cleared_on_construction() {
        // Storing the address the admin typed rather than the network would
        // let the same block be added twice under two spellings, and each
        // would then have to be matched separately.
        assert_eq!(prefix("203.0.113.57/24").to_string(), "203.0.113.0/24");
        assert_eq!(prefix("2001:db8:1:2::5/48").to_string(), "2001:db8:1::/48");
        assert_eq!(prefix("203.0.113.57/24"), prefix("203.0.113.0/24"));
    }

    #[test]
    fn containment_covers_the_whole_block_and_stops_at_its_edges() {
        let net = prefix("203.0.113.0/24");
        assert!(net.contains(ip("203.0.113.0")));
        assert!(net.contains(ip("203.0.113.255")));
        assert!(!net.contains(ip("203.0.114.0")));
        assert!(!net.contains(ip("203.0.112.255")));
    }

    #[test]
    fn a_host_route_matches_only_itself() {
        let net = prefix("198.51.100.7");
        assert!(net.contains(ip("198.51.100.7")));
        assert!(!net.contains(ip("198.51.100.8")));
        assert!(!net.contains(ip("198.51.100.6")));
    }

    #[test]
    fn the_families_never_match_each_other() {
        // A v6 client must not be admitted by a v4 office entry just because
        // the office has one — that would be a silent bypass on the day the
        // ISP turns v6 on.
        assert!(!prefix("203.0.113.0/24").contains(ip("2001:db8::1")));
        assert!(!prefix("2001:db8::/48").contains(ip("203.0.113.5")));
    }

    #[test]
    fn ipv4_mapped_v6_clients_match_the_ipv4_block() {
        // A dual-stack listener reports v4 peers as ::ffff:a.b.c.d. Treating
        // that as a different address would break check-in for every employee
        // the moment the socket is opened dual-stack.
        assert!(prefix("203.0.113.0/24").contains(ip("::ffff:203.0.113.5")));
        assert_eq!(prefix("::ffff:203.0.113.5").to_string(), "203.0.113.5/32");
    }

    #[test]
    fn a_slash_zero_prefix_masks_to_the_whole_space_without_overflowing() {
        // The shift-by-width case: `!0u32 << 32` is undefined behaviour in
        // release mode and a panic in debug, and /0 is precisely the input an
        // attacker would like to see accepted.
        let net = prefix("203.0.113.5/0");
        assert_eq!(net.to_string(), "0.0.0.0/0");
        assert!(net.contains(ip("8.8.8.8")));

        let v6 = prefix("2001:db8::1/0");
        assert_eq!(v6.to_string(), "::/0");
    }

    #[test]
    fn a_prefix_longer_than_the_family_is_rejected() {
        assert!(matches!(
            IpPrefix::parse("203.0.113.5/33"),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            IpPrefix::parse("2001:db8::1/129"),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn malformed_input_is_rejected_rather_than_coerced() {
        for value in [
            "",
            "not-an-ip",
            "203.0.113.5/",
            "203.0.113.5/abc",
            "203.0.113.999/24",
            "203.0.113.5/24/16",
        ] {
            assert!(IpPrefix::parse(value).is_err(), "should reject {value:?}");
        }
    }

    // ─── Policy: which addresses may identify a place ───

    #[test]
    fn private_ranges_cannot_identify_a_network() {
        // The failure this prevents: with TRUST_PROXY_HEADERS off behind a
        // local reverse proxy, every client resolves to the same 172.x address.
        // Approving it once would admit the entire internet.
        for value in ["10.0.0.7", "172.16.4.1", "172.31.255.254", "192.168.1.1"] {
            assert!(!is_identifying(ip(value)), "{value} is private");
        }
    }

    #[test]
    fn carrier_grade_nat_cannot_identify_a_network() {
        // 100.64.0.0/10 is shared across a mobile carrier's whole subscriber
        // base — approving it would put every phone on that network "in the
        // office".
        for value in ["100.64.0.1", "100.100.50.3", "100.127.255.255"] {
            assert!(!is_identifying(ip(value)), "{value} is CGNAT");
        }
        // The neighbouring blocks are ordinary public space and must not be
        // caught by the /10 test.
        assert!(is_identifying(ip("100.63.255.255")));
        assert!(is_identifying(ip("100.128.0.1")));
    }

    #[test]
    fn loopback_link_local_and_reserved_cannot_identify_a_network() {
        for value in [
            "127.0.0.1",
            "169.254.1.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "240.0.0.1",
        ] {
            assert!(!is_identifying(ip(value)), "{value} must be rejected");
        }
    }

    #[test]
    fn v6_local_ranges_cannot_identify_a_network() {
        for value in ["::1", "::", "fe80::1", "fc00::1", "fd12:3456::1", "ff02::1"] {
            assert!(!is_identifying(ip(value)), "{value} must be rejected");
        }
        assert!(is_identifying(ip("2001:db8::1")));
        assert!(is_identifying(ip("2404:6800:4001::200e")));
    }

    #[test]
    fn a_v4_mapped_private_address_is_rejected_through_its_v6_spelling() {
        // Otherwise `::ffff:10.0.0.7` sneaks a private address past the guard
        // that rejects `10.0.0.7`.
        assert!(!is_identifying(ip("::ffff:10.0.0.7")));
        assert!(!is_identifying(ip("::ffff:127.0.0.1")));
    }

    #[test]
    fn approval_rejects_blocks_wider_than_the_policy_floor() {
        assert!(IpPrefix::parse_approvable("203.0.113.0/24").is_ok());
        assert!(IpPrefix::parse_approvable("203.0.113.5").is_ok());
        assert!(IpPrefix::parse_approvable("203.0.0.0/20").is_ok());

        for value in ["203.0.0.0/19", "203.0.0.0/8", "0.0.0.0/0"] {
            assert!(
                matches!(
                    IpPrefix::parse_approvable(value),
                    Err(AppError::BadRequest(_))
                ),
                "should refuse {value} as too broad"
            );
        }
    }

    #[test]
    fn approval_applies_the_v6_floor_separately() {
        assert!(IpPrefix::parse_approvable("2001:db8::/48").is_ok());
        assert!(IpPrefix::parse_approvable("2001:db8:0:1::/64").is_ok());
        assert!(matches!(
            IpPrefix::parse_approvable("2001:db8::/32"),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn approval_rejects_non_identifying_addresses_before_checking_width() {
        // A /32 host route is narrow enough, but 10.0.0.7 still must not be
        // approvable — the width check alone would have let it through.
        let err = IpPrefix::parse_approvable("10.0.0.7");
        assert!(matches!(err, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn address_count_matches_the_prefix_width() {
        assert_eq!(prefix("203.0.113.5").address_count(), 1);
        assert_eq!(prefix("203.0.113.0/24").address_count(), 256);
        assert_eq!(prefix("203.0.0.0/20").address_count(), 4096);
        assert_eq!(prefix("2001:db8::/128").address_count(), 1);
        // /0 covers the entire v6 space, which does not fit in a u128 count.
        assert_eq!(prefix("::/0").address_count(), u128::MAX);
    }

    #[test]
    fn the_policy_floors_are_themselves_approvable() {
        // A floor that its own validator rejects would make every entry fail.
        assert!(IpPrefix::parse_approvable(&format!("203.0.0.0/{MIN_IPV4_PREFIX_LEN}")).is_ok());
        assert!(IpPrefix::parse_approvable(&format!("2001:db8::/{MIN_IPV6_PREFIX_LEN}")).is_ok());
    }
}
