//! Authorized reads of files stored under the API's local `uploads/` directory.
//!
//! `GET /api/uploads/{filename}` was previously unauthenticated. The UUID prefix
//! `portal::upload_file` puts on a stored name made a URL hard to guess, but a
//! capability URL only holds while nobody else ever sees it — and these leak by
//! design: they are pasted into chats, kept in browser history, and recorded by
//! every proxy in front of the API. Anyone holding one could read another
//! tenant's claim receipt or medical certificate.
//!
//! A download is now authorized against the record that references the file.
//! Because an upload has no owning row of its own, that reverse lookup *is* the
//! ownership model — see `repositories::reads::uploads`.

use std::path::{Path, PathBuf};

use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::upload::{UploadAccess, UploadReference, UploadReferenceKind};
use crate::repositories::reads::uploads;

/// Where `portal::upload_file` writes, relative to the API's working directory.
pub const UPLOAD_DIR: &str = "uploads";

/// The URL prefix stored in `claims.receipt_url` and its siblings.
pub const UPLOAD_URL_PREFIX: &str = "/api/uploads/";

/// File types the product accepts. Anything else is refused on the way in *and*
/// on the way back in from a backup archive.
pub const ALLOWED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "pdf", "doc", "docx", "xls", "xlsx",
];

/// Largest upload the API will store, enforced on both entry paths.
pub const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// The lowercased extension of a stored or original file name.
pub fn extension_of(filename: &str) -> String {
    filename.rsplit('.').next().unwrap_or("").to_lowercase()
}

/// Whether the leading bytes match the type the extension claims.
///
/// Stops a script or executable arriving under a `.png` name — it is the only
/// check that looks at content rather than at what the caller asserted.
pub fn validate_magic_bytes(data: &[u8], claimed_ext: &str) -> bool {
    match claimed_ext {
        "pdf" => data.starts_with(b"%PDF"),
        "jpg" | "jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => data.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "gif" => data.starts_with(b"GIF8"),
        "webp" => data.len() >= 12 && &data[8..12] == b"WEBP",
        "doc" | "xls" => data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]),
        "docx" | "xlsx" => data.starts_with(&[0x50, 0x4B, 0x03, 0x04]),
        _ => false,
    }
}

/// The full content gate: allowed extension, size cap, and matching magic bytes.
///
/// Shared by the upload endpoint and by backup restore so a file cannot enter
/// the store through the archive that would have been refused at the door.
pub fn validate_upload_bytes(filename: &str, data: &[u8]) -> AppResult<()> {
    let ext = extension_of(filename);

    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!(
            "File type .{} is not allowed. Allowed: {}",
            ext,
            ALLOWED_EXTENSIONS.join(", ")
        )));
    }

    if data.len() > MAX_UPLOAD_SIZE {
        return Err(AppError::BadRequest(format!(
            "File too large. Maximum size is {} MB",
            MAX_UPLOAD_SIZE / 1024 / 1024
        )));
    }

    if !validate_magic_bytes(data, &ext) {
        return Err(AppError::BadRequest(
            "File content does not match its extension".into(),
        ));
    }

    Ok(())
}

/// Resolves a request path segment to a file inside [`UPLOAD_DIR`].
///
/// Rejects anything that is not a single plain path component. The explicit
/// `..`/separator checks are redundant with the component count on their own,
/// but they are cheap and they keep the intent legible next to the traversal
/// they exist to stop.
///
/// Public because backup restore resolves attacker-controlled names against the
/// same directory and must not grow a second, weaker implementation.
pub fn safe_upload_path(filename: &str) -> AppResult<PathBuf> {
    let is_plain_name = !filename.is_empty()
        && !filename.contains("..")
        && !filename.contains('/')
        && !filename.contains('\\')
        && Path::new(filename).components().count() == 1;

    if !is_plain_name {
        return Err(AppError::BadRequest("Invalid filename".into()));
    }

    Ok(Path::new(UPLOAD_DIR).join(filename))
}

/// Whether `access` may read a file reachable through `reference`.
///
/// Two ways in, and only two: the file is attached to the caller's own record,
/// or the caller holds the permission that covers the surface the record lives
/// on. A company-wide document (`employee_id IS NULL`) has no owner and is
/// therefore always the permission path.
fn permits(access: &UploadAccess, reference: &UploadReference) -> bool {
    let is_own_record =
        reference.employee_id.is_some() && reference.employee_id == access.employee_id;
    if is_own_record {
        return true;
    }

    match reference.kind {
        UploadReferenceKind::Claim | UploadReferenceKind::LeaveRequest => access.can_view_approvals,
        UploadReferenceKind::Document => access.can_view_documents,
    }
}

/// Reads an uploaded file, or fails as though it did not exist.
///
/// Every denial is a 404, never a 403: a 403 would confirm that a filename is
/// real in some other tenant, which is exactly the fact the old capability URL
/// leaked. The one 4xx that is not a 404 is a malformed filename, which reveals
/// nothing because it is rejected before any lookup happens.
pub async fn read_authorized(
    pool: &PgPool,
    filename: &str,
    access: &UploadAccess,
) -> AppResult<Vec<u8>> {
    let path = safe_upload_path(filename)?;
    let file_url = format!("{UPLOAD_URL_PREFIX}{filename}");

    let references = uploads::find_references(pool, access.company_id, &file_url).await?;
    if !references
        .iter()
        .any(|reference| permits(access, reference))
    {
        return Err(AppError::NotFound("File not found".into()));
    }

    tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound("File not found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn access(employee_id: Option<Uuid>, approvals: bool, documents: bool) -> UploadAccess {
        UploadAccess {
            company_id: Uuid::new_v4(),
            employee_id,
            can_view_approvals: approvals,
            can_view_documents: documents,
        }
    }

    fn reference(kind: UploadReferenceKind, employee_id: Option<Uuid>) -> UploadReference {
        UploadReference { kind, employee_id }
    }

    #[test]
    fn rejects_traversal_and_separators() {
        for filename in [
            "",
            "..",
            "../secrets.env",
            "..\\secrets.env",
            "nested/file.pdf",
            "nested\\file.pdf",
            "/etc/passwd",
        ] {
            assert!(
                safe_upload_path(filename).is_err(),
                "{filename:?} must not resolve to a path"
            );
        }
    }

    #[test]
    fn accepts_a_stored_name() {
        let stored = format!("{}_receipt.pdf", Uuid::new_v4());
        let path = safe_upload_path(&stored).expect("a plain stored name resolves");
        assert_eq!(path, Path::new(UPLOAD_DIR).join(&stored));
    }

    #[test]
    fn an_employee_reads_their_own_attachment() {
        let employee = Uuid::new_v4();
        let caller = access(Some(employee), false, false);

        for kind in [
            UploadReferenceKind::Claim,
            UploadReferenceKind::LeaveRequest,
        ] {
            assert!(
                permits(&caller, &reference(kind, Some(employee))),
                "an employee holds no permissions but owns {kind:?}"
            );
        }
    }

    #[test]
    fn an_employee_cannot_read_a_colleagues_attachment() {
        let caller = access(Some(Uuid::new_v4()), false, false);
        let colleague = reference(UploadReferenceKind::Claim, Some(Uuid::new_v4()));

        assert!(!permits(&caller, &colleague));
    }

    /// The bug this module exists to fix: two callers with no employee profile
    /// must not match each other through a pair of `None`s.
    #[test]
    fn a_missing_employee_profile_never_counts_as_ownership() {
        let caller = access(None, false, false);
        let company_wide = reference(UploadReferenceKind::Document, None);
        let someone_elses = reference(UploadReferenceKind::Claim, Some(Uuid::new_v4()));

        assert!(!permits(&caller, &company_wide));
        assert!(!permits(&caller, &someone_elses));
    }

    #[test]
    fn approvals_permission_opens_claims_and_leave_but_not_documents() {
        let caller = access(None, true, false);

        assert!(permits(
            &caller,
            &reference(UploadReferenceKind::Claim, Some(Uuid::new_v4()))
        ));
        assert!(permits(
            &caller,
            &reference(UploadReferenceKind::LeaveRequest, Some(Uuid::new_v4()))
        ));
        assert!(!permits(
            &caller,
            &reference(UploadReferenceKind::Document, None)
        ));
    }

    #[test]
    fn documents_permission_opens_documents_but_not_claims() {
        let caller = access(None, false, true);

        assert!(permits(
            &caller,
            &reference(UploadReferenceKind::Document, None)
        ));
        assert!(!permits(
            &caller,
            &reference(UploadReferenceKind::Claim, Some(Uuid::new_v4()))
        ));
    }

    #[test]
    fn an_unknown_discriminator_maps_to_no_kind() {
        assert!(UploadReferenceKind::from_wire("payslip").is_none());
        assert_eq!(
            UploadReferenceKind::from_wire("claim"),
            Some(UploadReferenceKind::Claim)
        );
    }
}
