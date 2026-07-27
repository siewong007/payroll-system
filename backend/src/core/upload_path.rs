//! Validation of client-supplied upload URLs and of the on-disk paths derived
//! from them.
//!
//! Every filesystem sink under `uploads/` used to build its path by joining a
//! string that had arrived in request JSON. `Path::join` with an absolute
//! component silently *discards* the base, so a stored `file_url` of
//! `/api/uploads//etc/ssl/private/key.pem` resolved to `/etc/ssl/private/key.pem`
//! and the document-delete path unlinked it. Splitting the question in two —
//! "is this URL safe to store?" at write time, "which file on disk does it
//! name?" at use time — is what makes that unrepresentable: no call site builds
//! a path under [`UPLOAD_DIR`] itself, it asks this module for one.
//!
//! Pure: no I/O and no SQL, so it belongs in `core/` and is callable from both
//! handlers and services without breaking the layering rule.

use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};

/// URL prefix under which locally stored uploads are served (`handlers::portal::serve_upload`).
pub const UPLOAD_URL_PREFIX: &str = "/api/uploads/";

/// Directory the API container writes uploads to, relative to its working directory.
pub const UPLOAD_DIR: &str = "uploads";

/// Extensions accepted on upload, and the only ones a restored backup blob may
/// be written under. Kept here rather than in the upload handler because the
/// backup restore path needs the same list and a second copy would drift.
pub const ALLOWED_UPLOAD_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "pdf", "doc", "docx", "xls", "xlsx",
];

/// Generous relative to what the upload handler produces (~92 chars: uuid +
/// 50-char sanitized stem + extension). It exists to bound the work every sink
/// does on a hostile value, not to constrain legitimate names.
const MAX_STORED_NAME_LEN: usize = 200;

fn invalid_name() -> AppError {
    AppError::BadRequest("Invalid file name".into())
}

/// A bare stored name proven to be exactly one safe path component.
///
/// The explicit character rejections are not redundant with the `components()`
/// check: they make the result platform-independent. On Linux `a\b` is a single
/// legal `Component::Normal`, on Windows it is two — and the same binary must
/// not accept a value on one that escapes the upload directory on the other.
pub fn sanitize_stored_name(name: &str) -> AppResult<&str> {
    if name.is_empty() || name.len() > MAX_STORED_NAME_LEN {
        return Err(invalid_name());
    }
    if name.contains(['/', '\\', '\0', ':']) || name.contains("..") {
        return Err(invalid_name());
    }

    // Rejects `.`, `..`, drive prefixes and anything else the OS would read as
    // navigation rather than as a file name.
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(only)), None) if only.to_str() == Some(name) => Ok(name),
        _ => Err(invalid_name()),
    }
}

/// `uploads/<name>` — the only permitted way to build a path under [`UPLOAD_DIR`].
pub fn stored_path(name: &str) -> AppResult<PathBuf> {
    Ok(Path::new(UPLOAD_DIR).join(sanitize_stored_name(name)?))
}

/// Mint a safe server-owned stored name while preserving only an allow-listed
/// extension from an untrusted archive key.
pub fn generated_stored_name(archive_key: &str) -> Option<String> {
    let extension = Path::new(archive_key)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    ALLOWED_UPLOAD_EXTENSIONS
        .contains(&extension.as_str())
        .then(|| format!("{}_restored.{extension}", Uuid::new_v4()))
}

/// On-disk path a stored URL names.
///
/// `Ok(None)` means the URL is not a local upload at all (an external link),
/// which every sink must treat as "nothing to read, write or unlink" rather
/// than as an error. `Err` means it claims to be a local upload but does not
/// name a safe one — a value that should never have reached the database.
pub fn local_upload_path(file_url: &str) -> AppResult<Option<PathBuf>> {
    match file_url.strip_prefix(UPLOAD_URL_PREFIX) {
        Some(name) => stored_path(name).map(Some),
        None => Ok(None),
    }
}

/// Write-time gate for every `*_url` column whose value can name a stored upload.
///
/// Two shapes are accepted. `/api/uploads/<single safe component>` is the
/// invariant the filesystem sinks depend on. Any `http(s)` URL is accepted
/// because `documents.file_url` is a free-text field in the current UI and
/// tenants legitimately point it at links that are not local uploads; rejecting
/// every *other* scheme is what closes the stored-XSS variant, where a
/// `javascript:` value is rendered straight into an `href`.
pub fn validate_file_url(file_url: &str) -> AppResult<()> {
    if let Some(name) = file_url.strip_prefix(UPLOAD_URL_PREFIX) {
        sanitize_stored_name(name)?;
        return Ok(());
    }
    if has_http_scheme(file_url) {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "File URL must be an uploaded file (/api/uploads/<name>) or an http(s) link".into(),
    ))
}

/// [`validate_file_url`] for the nullable columns (`attachment_url`, `receipt_url`).
pub fn validate_optional_file_url(file_url: Option<&str>) -> AppResult<()> {
    file_url.map_or(Ok(()), validate_file_url)
}

/// Scheme match is case-insensitive: `HTTPS://…` is as safe as `https://…`, and
/// treating it as unsafe would only reject valid tenant data.
fn has_http_scheme(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_name_the_upload_handler_produces() {
        let name = "0198f2c4-1f3a-7c21-9b0e-2f6a1c8d4e55_offer_letter.pdf";
        assert_eq!(sanitize_stored_name(name).unwrap(), name);
        assert_eq!(
            stored_path(name).unwrap(),
            Path::new("uploads").join(name),
            "stored_path must anchor under the upload directory"
        );
    }

    #[test]
    fn rejects_every_traversal_and_absolute_shape() {
        for hostile in [
            "",
            ".",
            "..",
            "../x.pdf",
            "..\\x.pdf",
            "a/b.pdf",
            "a\\b.pdf",
            "/etc/passwd",
            "/etc/ssl/private/key.pem",
            "C:\\Windows\\win.ini",
            "\\\\server\\share\\x.pdf",
            "x\0y.pdf",
            "....//x.pdf",
            "sub/../x.pdf",
        ] {
            assert!(
                sanitize_stored_name(hostile).is_err(),
                "accepted hostile name {hostile:?}"
            );
        }
    }

    #[test]
    fn bounds_the_name_length() {
        let at_limit = "a".repeat(MAX_STORED_NAME_LEN);
        assert!(sanitize_stored_name(&at_limit).is_ok());

        let over_limit = "a".repeat(MAX_STORED_NAME_LEN + 1);
        assert!(sanitize_stored_name(&over_limit).is_err());
    }

    #[test]
    fn local_upload_path_resolves_only_conforming_urls() {
        assert_eq!(
            local_upload_path("/api/uploads/a.pdf").unwrap(),
            Some(Path::new("uploads").join("a.pdf"))
        );

        // The reported exploit: `join` on an absolute component drops the base.
        assert!(local_upload_path("/api/uploads//etc/ssl/private/key.pem").is_err());
        assert!(local_upload_path("/api/uploads/../../app/.env").is_err());
        assert!(local_upload_path("/api/uploads/../../proc/self/environ").is_err());
        assert!(local_upload_path("/api/uploads/").is_err());
    }

    #[test]
    fn local_upload_path_reports_external_links_as_absent_not_invalid() {
        // An external link is not an error at a filesystem sink — there is simply
        // no local file to read, write or unlink.
        assert_eq!(
            local_upload_path("https://example.com/a.pdf").unwrap(),
            None
        );
        assert_eq!(local_upload_path("/uploads/a.pdf").unwrap(), None);
    }

    #[test]
    fn validate_file_url_accepts_the_two_supported_shapes() {
        assert!(validate_file_url("/api/uploads/a.pdf").is_ok());
        assert!(validate_file_url("https://example.com/handbook.pdf").is_ok());
        assert!(validate_file_url("http://intranet.local/handbook.pdf").is_ok());
        assert!(validate_file_url("HTTPS://Example.com/handbook.pdf").is_ok());
    }

    #[test]
    fn validate_file_url_rejects_traversal_other_schemes_and_bare_paths() {
        for hostile in [
            "/api/uploads//etc/ssl/private/key.pem",
            "/api/uploads/../../app/.env",
            "/api/uploads/",
            "/uploads/document.pdf",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///etc/passwd",
            "",
        ] {
            assert!(
                validate_file_url(hostile).is_err(),
                "accepted hostile url {hostile:?}"
            );
        }
    }

    #[test]
    fn optional_file_url_treats_absence_as_valid() {
        assert!(validate_optional_file_url(None).is_ok());
        assert!(validate_optional_file_url(Some("/api/uploads/a.pdf")).is_ok());
        assert!(validate_optional_file_url(Some("/api/uploads/../x")).is_err());
    }
}
