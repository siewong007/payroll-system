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
//!
//! This module owns *who may read a file* and the content gate applied to bytes
//! entering the store. It owns no path logic: resolving a name to a location
//! under the upload directory belongs to [`crate::core::upload_path`], which is
//! the single validator for every filesystem sink. Two implementations of that
//! check briefly existed here and in `core` after parallel fixes for the same
//! traversal defect; the stricter one won.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::{self, ALLOWED_UPLOAD_EXTENSIONS};
use crate::models::upload::{UploadAccess, UploadReference, UploadReferenceKind};
use crate::repositories::reads::uploads;
use crate::repositories::uploaded_files;

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
        "gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        "webp" => data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP",
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

    if !ALLOWED_UPLOAD_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!(
            "File type .{} is not allowed. Allowed: {}",
            ext,
            ALLOWED_UPLOAD_EXTENSIONS.join(", ")
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

/// Whether `access` may read a file reachable through `reference`.
///
/// Two ways in, and only two: the file is attached to the caller's own record,
/// or the caller holds the permission that covers the surface the record lives
/// on. A company-wide document (`employee_id IS NULL`) has no owner and is
/// therefore always the permission path.
fn permits(access: &UploadAccess, reference: &UploadReference) -> bool {
    if reference.is_confidential && !access.can_view_confidential {
        return false;
    }

    let is_own_record =
        reference.employee_id.is_some() && reference.employee_id == access.employee_id;
    if is_own_record {
        return true;
    }

    match reference.kind {
        UploadReferenceKind::Claim | UploadReferenceKind::LeaveRequest => access.can_view_approvals,
        UploadReferenceKind::Document => access.can_view_documents,
        UploadReferenceKind::CompanyLogo => true,
    }
}

/// Reject attempts to attach another tenant's local upload path to a record.
///
/// Legacy SQL-seeded URLs without an `uploaded_files` row remain allowed here
/// so seeded fixtures keep working; newly uploaded files always carry a row
/// proving which company created them.
pub async fn ensure_upload_belongs_to_company(
    pool: &PgPool,
    company_id: Uuid,
    file_url: Option<&str>,
) -> AppResult<()> {
    let Some(url) = file_url else {
        return Ok(());
    };
    let Some(stored_name) = url.strip_prefix(upload_path::UPLOAD_URL_PREFIX) else {
        return Ok(());
    };
    upload_path::sanitize_stored_name(stored_name)?;

    if let Some(owner) = uploaded_files::find_by_stored_name(pool, stored_name).await?
        && owner.company_id != company_id
    {
        return Err(AppError::BadRequest("Invalid file URL".into()));
    }
    Ok(())
}

/// Build an [`UploadAccess`] capability summary from an authenticated user.
pub fn access_from_auth(auth: &crate::core::auth::AuthUser, company_id: Uuid) -> UploadAccess {
    use crate::core::auth::Permission;
    UploadAccess {
        company_id,
        user_id: auth.0.sub,
        employee_id: auth.0.employee_id,
        can_view_approvals: auth.can(Permission::ViewApprovals),
        can_view_documents: auth.can(Permission::ViewDocuments),
        can_view_confidential: auth.can(Permission::ManageEmployees)
            || auth.is_payroll_privileged(),
    }
}

/// Verify that `access` may bind `file_url` onto a new or edited record.
///
/// Prevents same-company IDOR (binding another user's unreferenced upload or an
/// inaccessible/confidential record's upload to gain read permission via
/// `find_references` OR-semantics) as well as cross-tenant attachment.
pub async fn ensure_upload_attach_authorized(
    pool: &PgPool,
    access: &UploadAccess,
    file_url: Option<&str>,
    target_allows_confidential: bool,
) -> AppResult<()> {
    let Some(url) = file_url.filter(|u| !u.is_empty()) else {
        return Ok(());
    };
    let Some(stored_name) = url.strip_prefix(upload_path::UPLOAD_URL_PREFIX) else {
        return Ok(());
    };
    upload_path::sanitize_stored_name(stored_name)?;

    let Some(owner) = uploaded_files::find_by_stored_name(pool, stored_name).await? else {
        return Ok(());
    };
    if owner.company_id != access.company_id {
        return Err(AppError::BadRequest("Invalid file URL".into()));
    }

    let full_url = format!("{}{}", upload_path::UPLOAD_URL_PREFIX, stored_name);
    let references = uploads::find_references(pool, access.company_id, &full_url).await?;
    if references.is_empty() {
        if owner.uploaded_by == Some(access.user_id) {
            return Ok(());
        }
        return Err(AppError::BadRequest("Invalid file URL".into()));
    }

    if references.iter().any(|r| r.is_confidential) && !target_allows_confidential {
        return Err(AppError::BadRequest("Invalid file URL".into()));
    }
    if references.iter().all(|r| permits(access, r)) {
        Ok(())
    } else {
        Err(AppError::BadRequest("Invalid file URL".into()))
    }
}

/// Remove unreferenced uploaded blobs older than `min_age_hours` from DB and disk.
pub async fn purge_unreferenced_uploads(pool: &PgPool, min_age_hours: i32) -> AppResult<u64> {
    let removed = uploaded_files::purge_unreferenced_older_than_hours(pool, min_age_hours).await?;
    let count = removed.len() as u64;
    for stored_name in removed {
        if let Ok(path) = upload_path::stored_path(&stored_name) {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    Ok(count)
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
    let path = upload_path::stored_path(filename)?;
    let owner = uploaded_files::find_by_stored_name(pool, filename).await?;
    if let Some(ref record) = owner
        && record.company_id != access.company_id
    {
        return Err(AppError::NotFound("File not found".into()));
    }

    let file_url = format!("{}{}", upload_path::UPLOAD_URL_PREFIX, filename);
    let references = uploads::find_references(pool, access.company_id, &file_url).await?;
    let permitted = if references.is_empty() {
        // Newly uploaded preview before form submission: visible only to the uploader.
        owner
            .as_ref()
            .and_then(|record| record.uploaded_by)
            .is_some_and(|uploader| uploader == access.user_id)
    } else {
        references
            .iter()
            .any(|reference| permits(access, reference))
    };

    if !permitted {
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
            user_id: Uuid::new_v4(),
            employee_id,
            can_view_approvals: approvals,
            can_view_documents: documents,
            can_view_confidential: false,
        }
    }

    fn reference(kind: UploadReferenceKind, employee_id: Option<Uuid>) -> UploadReference {
        UploadReference {
            kind,
            employee_id,
            is_confidential: false,
        }
    }

    // Path-shape cases (traversal, separators, absolute, NUL, UNC, length) are
    // owned by `core::upload_path`, which this module now delegates to. The
    // tests below cover what is unique here: the content gate, and who may read
    // a file once its path has been resolved.

    #[test]
    fn the_content_gate_requires_an_allow_listed_extension() {
        assert!(validate_upload_bytes("payload.sh", b"#!/bin/sh\n").is_err());
        assert!(validate_upload_bytes("noextension", b"%PDF-1.4").is_err());
        assert!(validate_upload_bytes("doc.pdf", b"%PDF-1.4").is_ok());
    }

    /// The check that looks at content rather than at what the caller asserted:
    /// a script renamed to `.png` must not enter the store.
    #[test]
    fn the_content_gate_rejects_bytes_that_contradict_the_extension() {
        assert!(validate_upload_bytes("x.png", b"<?php echo 1; ?>").is_err());
        assert!(validate_upload_bytes("x.pdf", b"GIF89a").is_err());
        assert!(validate_upload_bytes("x.png", &[0x89, 0x50, 0x4E, 0x47, 0x0D]).is_ok());
    }

    /// `webp` is the only arm that indexes rather than matching a prefix, so a
    /// short file must not panic it.
    #[test]
    fn a_truncated_webp_is_rejected_without_panicking() {
        assert!(!validate_magic_bytes(b"RIFF", "webp"));
        assert!(!validate_magic_bytes(b"", "webp"));
        assert!(validate_magic_bytes(b"RIFF\0\0\0\0WEBPVP8 ", "webp"));
    }

    #[test]
    fn the_content_gate_enforces_the_size_cap() {
        let mut oversized = vec![0x25, 0x50, 0x44, 0x46]; // "%PDF"
        oversized.resize(MAX_UPLOAD_SIZE + 1, 0);
        assert!(validate_upload_bytes("big.pdf", &oversized).is_err());
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
    fn confidential_documents_require_confidential_privilege() {
        let employee = Uuid::new_v4();
        let plain_viewer = UploadAccess {
            company_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            employee_id: Some(employee),
            can_view_approvals: true,
            can_view_documents: true,
            can_view_confidential: false,
        };
        let hr_viewer = UploadAccess {
            can_view_confidential: true,
            ..plain_viewer
        };
        let confidential_doc = UploadReference {
            kind: UploadReferenceKind::Document,
            employee_id: Some(employee),
            is_confidential: true,
        };
        assert!(!permits(&plain_viewer, &confidential_doc));
        assert!(permits(&hr_viewer, &confidential_doc));
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
