//! Attachment bytes carried in a company backup archive.
//!
//! Both directions resolve a filename that ultimately came from outside the
//! process — a database column on export, an uploaded JSON key on import — so
//! neither joins it onto `uploads/` directly. `core::upload_path::stored_path`
//! is the single containment check; restore additionally re-applies the content
//! gate from the upload endpoint, because an archive must not be able to plant a
//! file that would have been refused at the door.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine;

use crate::core::upload_path::{UPLOAD_DIR, UPLOAD_URL_PREFIX, stored_path};
use crate::models::backup::{ClaimExport, DocumentExport, LeaveRequestExport};
use crate::services::upload_service::validate_upload_bytes;

pub fn collect_backup_files(
    documents: &[DocumentExport],
    leave_requests: &[LeaveRequestExport],
    claims: &[ClaimExport],
) -> HashMap<String, String> {
    let mut files = HashMap::new();
    let b64 = base64::engine::general_purpose::STANDARD;

    let mut collect_file = |url: Option<&String>| {
        if let Some(u) = url
            && let Some(filename) = u.strip_prefix(UPLOAD_URL_PREFIX)
            // `documents.file_url` is free text an administrator types, so this
            // side is not trustworthy either.
            && let Ok(path) = stored_path(filename)
            && let Ok(data) = std::fs::read(&path)
        {
            files.insert(u.clone(), b64.encode(&data));
        }
    };

    for document in documents {
        collect_file(Some(&document.file_url));
    }
    for leave_request in leave_requests {
        collect_file(leave_request.attachment_url.as_ref());
    }
    for claim in claims {
        collect_file(claim.receipt_url.as_ref());
    }

    files
}

/// Writes the archive's attachments back into the uploads directory.
///
/// Returns operator-facing warnings. Rejections are reported rather than
/// swallowed: silence would let an operator believe a restore was complete when
/// attachments had been dropped.
pub async fn restore_backup_files(files: &HashMap<String, String>) -> Vec<String> {
    if files.is_empty() {
        return Vec::new();
    }

    let _ = tokio::fs::create_dir_all(Path::new(UPLOAD_DIR)).await;
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut restored = 0usize;
    let mut rejected = 0usize;

    for (url, data_b64) in files {
        // Every key in this map came from the uploaded archive. Before this
        // guard, a key of `/api/uploads/../../<path>` escaped the uploads
        // directory entirely and wrote wherever the container user could.
        let Some(filename) = url.strip_prefix(UPLOAD_URL_PREFIX) else {
            rejected += 1;
            continue;
        };
        let Ok(path) = stored_path(filename) else {
            rejected += 1;
            continue;
        };
        let Ok(data) = b64.decode(data_b64) else {
            rejected += 1;
            continue;
        };
        if validate_upload_bytes(filename, &data).is_err() {
            rejected += 1;
            continue;
        }

        if tokio::fs::write(&path, &data).await.is_ok() {
            restored += 1;
        } else {
            rejected += 1;
        }
    }

    let mut warnings = Vec::new();
    if restored > 0 {
        warnings.push(format!("{restored} file(s) restored to uploads directory."));
    }
    if rejected > 0 {
        warnings.push(format!(
            "{rejected} attachment(s) in the archive were rejected and not restored \
             (unsafe file name, disallowed type, or content that did not match its extension)."
        ));
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(entries: &[(&str, &[u8])]) -> HashMap<String, String> {
        let b64 = base64::engine::general_purpose::STANDARD;
        entries
            .iter()
            .map(|(url, data)| ((*url).to_string(), b64.encode(data)))
            .collect()
    }

    /// The traversal this module exists to stop. Nothing is written and the
    /// operator is told the entry was dropped.
    #[tokio::test]
    async fn a_traversal_key_is_rejected_and_reported() {
        let warnings = restore_backup_files(&archive(&[
            ("/api/uploads/../../escaped.pdf", b"%PDF-1.4"),
            ("/api/uploads/..\\escaped.pdf", b"%PDF-1.4"),
            ("/api/uploads/nested/escaped.pdf", b"%PDF-1.4"),
        ]))
        .await;

        assert!(
            !Path::new("escaped.pdf").exists() && !Path::new("../escaped.pdf").exists(),
            "a traversal key must not write outside the uploads directory"
        );
        assert!(
            warnings.iter().any(|w| w.contains("3 attachment(s)")),
            "every rejected entry must be reported, got {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("restored to uploads")),
            "nothing was restorable, got {warnings:?}"
        );
    }

    /// An archive must not be a side door around the upload endpoint's checks.
    #[tokio::test]
    async fn content_that_contradicts_its_extension_is_rejected() {
        let warnings = restore_backup_files(&archive(&[(
            "/api/uploads/payload.pdf",
            b"<?php echo 1; ?>",
        )]))
        .await;

        assert!(!Path::new(UPLOAD_DIR).join("payload.pdf").exists());
        assert!(warnings.iter().any(|w| w.contains("1 attachment(s)")));
    }

    #[tokio::test]
    async fn a_disallowed_extension_is_rejected() {
        let warnings =
            restore_backup_files(&archive(&[("/api/uploads/script.sh", b"#!/bin/sh\n")])).await;

        assert!(!Path::new(UPLOAD_DIR).join("script.sh").exists());
        assert!(warnings.iter().any(|w| w.contains("1 attachment(s)")));
    }

    #[tokio::test]
    async fn an_empty_archive_produces_no_warnings() {
        assert!(restore_backup_files(&HashMap::new()).await.is_empty());
    }

    #[tokio::test]
    async fn a_well_formed_entry_is_restored() {
        let name = format!("{}_restored.pdf", uuid::Uuid::new_v4());
        let url = format!("{UPLOAD_URL_PREFIX}{name}");
        let warnings = restore_backup_files(&archive(&[(&url, b"%PDF-1.4 restored")])).await;

        let path = Path::new(UPLOAD_DIR).join(&name);
        let written = path.exists();
        let _ = std::fs::remove_file(&path);

        assert!(written, "a valid attachment must be restored");
        assert!(warnings.iter().any(|w| w.contains("1 file(s) restored")));
    }
}
