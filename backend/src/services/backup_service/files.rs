//! The two filesystem sinks of the backup workflow.
//!
//! Both used to build a path by joining a string that came out of (export) or
//! straight from (import) client-controlled JSON. Neither trusts a supplied name
//! any more: the export reads only what [`upload_path::local_upload_path`]
//! resolves, and the restore ignores the archive's key entirely and writes each
//! blob under a server-generated name, rewriting the rows to match.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use base64::Engine;

use crate::core::upload_path;
use crate::models::backup::{ClaimExport, DocumentExport, LeaveRequestExport};

/// Per-file warnings are the point — a link silently vanishing from a restored
/// tenant is the failure class this pass exists to stop — but an archive can
/// carry thousands of blobs and `ImportResult` is a JSON response, so the tail
/// is summarized instead of listed.
const MAX_LISTED_WARNINGS: usize = 25;

/// Archive keys and document titles are attacker-controlled free text that is
/// echoed back in warnings, so they are bounded before they reach the response.
pub(super) fn short_label(value: &str) -> String {
    const MAX_LABEL_CHARS: usize = 100;

    if value.chars().count() <= MAX_LABEL_CHARS {
        return value.to_owned();
    }
    let head: String = value.chars().take(MAX_LABEL_CHARS).collect();
    format!("{head}…")
}

/// Keep at most [`MAX_LISTED_WARNINGS`] individual lines, then say how many
/// more there were. `noun` completes "… and 12 more <noun>.".
pub(super) fn cap_warnings(mut lines: Vec<String>, noun: &str) -> Vec<String> {
    if lines.len() > MAX_LISTED_WARNINGS {
        let remaining = lines.len() - MAX_LISTED_WARNINGS;
        lines.truncate(MAX_LISTED_WARNINGS);
        lines.push(format!("… and {remaining} more {noun}."));
    }
    lines
}

/// Read every locally stored attachment the export references, base64-encoded
/// by stored URL.
///
/// A value that claims to be a local upload but does not name a safe one is
/// skipped, never read. The result is embedded in the JSON archive the caller
/// downloads, so reading whatever path such a value names would hand it the
/// contents of that file — the exfiltration half of the traversal defect.
pub(super) fn collect_backup_files(
    documents: &[DocumentExport],
    leave_requests: &[LeaveRequestExport],
    claims: &[ClaimExport],
) -> HashMap<String, String> {
    let mut files = HashMap::new();
    let b64 = base64::engine::general_purpose::STANDARD;

    let mut collect_file = |url: Option<&String>| {
        let Some(url) = url else {
            return;
        };
        match upload_path::local_upload_path(url) {
            Ok(Some(path)) => {
                if let Ok(data) = std::fs::read(&path) {
                    files.insert(url.clone(), b64.encode(&data));
                }
            }
            // An external link: there is no local file to embed.
            Ok(None) => {}
            Err(_) => tracing::warn!(
                file_url = %short_label(url),
                "refusing to export a stored file URL that does not name a safe upload"
            ),
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

/// Where each archived blob will be written — and which ones will not be.
///
/// Decided up front so the rows can be pointed at the server-generated names in
/// the same pass, before the restore transaction opens.
#[derive(Debug, Default)]
pub(super) struct RestorePlan {
    /// Archive key → the `/api/uploads/<generated name>` URL its blob is written under.
    rewrites: HashMap<String, String>,
    /// Archive keys carrying no allow-listed extension. Sorted, so the warning
    /// text is stable across restores of the same archive.
    dropped: BTreeSet<String>,
}

impl RestorePlan {
    /// The URL a row referencing `archive_key` must be rewritten to.
    pub(super) fn rewritten_url(&self, archive_key: &str) -> Option<&str> {
        self.rewrites.get(archive_key).map(String::as_str)
    }

    /// Whether the archive carried this blob but it will not be restored.
    pub(super) fn is_dropped(&self, archive_key: &str) -> bool {
        self.dropped.contains(archive_key)
    }
}

/// Assign every blob in the archive a server-generated destination.
///
/// The key is used for one thing only: its extension. A key that carries no
/// allow-listed extension names nothing this system is willing to write, so its
/// blob is dropped rather than restored under some fallback name — which is
/// also what happens to every traversal payload, since `../../app/.env` and
/// `/etc/passwd` have no extension we accept.
pub(super) fn plan_restore(files: &HashMap<String, String>) -> RestorePlan {
    let mut plan = RestorePlan::default();

    for archive_key in files.keys() {
        match upload_path::generated_stored_name(archive_key) {
            Some(name) => {
                plan.rewrites.insert(
                    archive_key.clone(),
                    format!("{}{name}", upload_path::UPLOAD_URL_PREFIX),
                );
            }
            None => {
                plan.dropped.insert(archive_key.clone());
            }
        }
    }

    plan
}

/// Write every planned blob under the name the plan minted for it.
///
/// Returns the warnings to surface on `ImportResult`. Failures here are per file
/// and never fatal: the rows are already committed by the time this runs, so
/// aborting on one unwritable blob would only cost the tenant the rest of them.
pub(super) async fn restore_backup_files(
    files: &HashMap<String, String>,
    plan: &RestorePlan,
) -> Vec<String> {
    let mut problems: Vec<String> = plan
        .dropped
        .iter()
        .map(|archive_key| {
            format!(
                "Attached file \"{}\" was not restored: its name carries no supported file extension.",
                short_label(archive_key)
            )
        })
        .collect();

    if plan.rewrites.is_empty() {
        return cap_warnings(problems, "unrestored file(s)");
    }

    let upload_dir = Path::new(upload_path::UPLOAD_DIR);
    if let Err(error) = tokio::fs::create_dir_all(upload_dir).await {
        problems.push(format!(
            "No attached files were restored: the uploads directory could not be created ({error})."
        ));
        return cap_warnings(problems, "unrestored file(s)");
    }

    let b64 = base64::engine::general_purpose::STANDARD;
    let mut files_restored = 0usize;

    for (archive_key, data_b64) in files {
        let Some(stored_name) = plan
            .rewritten_url(archive_key)
            .and_then(|url| url.strip_prefix(upload_path::UPLOAD_URL_PREFIX))
        else {
            continue;
        };
        // The name is server-generated, so this cannot fail — but building the
        // path any other way is exactly what the defect was.
        let Ok(path) = upload_path::stored_path(stored_name) else {
            continue;
        };

        match b64.decode(data_b64) {
            Ok(data) => match tokio::fs::write(&path, &data).await {
                Ok(()) => files_restored += 1,
                Err(error) => problems.push(format!(
                    "Attached file \"{}\" could not be written ({error}).",
                    short_label(archive_key)
                )),
            },
            Err(error) => problems.push(format!(
                "Attached file \"{}\" could not be decoded ({error}).",
                short_label(archive_key)
            )),
        }
    }

    // `files` is a map, so iteration order — and therefore the order failures
    // were appended in — is not stable. Sort so the same archive reports the
    // same warnings in the same order.
    problems.sort();
    let mut warnings = cap_warnings(problems, "unrestored file(s)");
    if files_restored > 0 {
        warnings.insert(
            0,
            format!("{files_restored} file(s) restored to the uploads directory under new names."),
        );
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use uuid::Uuid;

    fn document_with_url(file_url: &str) -> DocumentExport {
        DocumentExport {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            employee_id: None,
            category_id: None,
            title: "Handbook".into(),
            description: None,
            file_name: "handbook.pdf".into(),
            file_url: file_url.into(),
            file_size: None,
            mime_type: None,
            status: "active".into(),
            issue_date: None,
            expiry_date: None,
            is_confidential: None,
            tags: None,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn archive(keys: &[&str]) -> HashMap<String, String> {
        keys.iter()
            .map(|key| ((*key).to_owned(), String::new()))
            .collect()
    }

    /// The export is base64-embedded in a JSON download, so a traversal value
    /// stored in `file_url` must not be read at all. `uploads/../Cargo.toml`
    /// resolves to a file that really exists next to the test process, which is
    /// what makes this a regression test rather than a vacuous one.
    #[test]
    fn export_refuses_to_read_outside_the_upload_directory() {
        let documents = [
            document_with_url("/api/uploads/../Cargo.toml"),
            document_with_url("/api/uploads//etc/ssl/private/key.pem"),
            document_with_url("/api/uploads/../../proc/self/environ"),
            document_with_url("https://example.com/handbook.pdf"),
        ];

        let files = collect_backup_files(&documents, &[], &[]);

        assert!(
            files.is_empty(),
            "no file outside uploads/ may be embedded in an export: {files:?}"
        );
    }

    #[test]
    fn plan_mints_a_safe_name_for_every_restorable_blob() {
        let files = archive(&["/api/uploads/a.pdf", "/api/uploads/b.PNG", "receipt.jpeg"]);

        let plan = plan_restore(&files);

        for key in files.keys() {
            let url = plan
                .rewritten_url(key)
                .unwrap_or_else(|| panic!("no destination planned for {key}"));
            let name = url
                .strip_prefix(upload_path::UPLOAD_URL_PREFIX)
                .expect("planned url is served from the upload prefix");
            assert!(
                upload_path::sanitize_stored_name(name).is_ok(),
                "planned name {name:?} is not a safe single component"
            );
            assert_ne!(
                url,
                key.as_str(),
                "the archive's own name must never be reused"
            );
            assert!(!plan.is_dropped(key));
        }
    }

    #[test]
    fn plan_gives_every_blob_its_own_destination() {
        let files = archive(&["/api/uploads/a.pdf", "/api/uploads/b.pdf"]);

        let plan = plan_restore(&files);

        let a = plan.rewritten_url("/api/uploads/a.pdf").unwrap();
        let b = plan.rewritten_url("/api/uploads/b.pdf").unwrap();
        assert_ne!(a, b, "two blobs must not be planned onto the same file");
    }

    #[test]
    fn plan_drops_traversal_payloads_rather_than_renaming_them() {
        let hostile = [
            "/api/uploads/../../app/.env",
            "/api/uploads//etc/ssl/private/key.pem",
            "/api/uploads/../../proc/self/environ",
            "payload.sh",
            "",
        ];
        let files = archive(&hostile);

        let plan = plan_restore(&files);

        for key in hostile {
            assert!(
                plan.rewritten_url(key).is_none(),
                "{key:?} must not be restored at all"
            );
            assert!(plan.is_dropped(key), "{key:?} must be reported as dropped");
        }
    }

    /// Nothing is written when the plan restores nothing, so this exercises the
    /// warning text without touching the filesystem.
    #[tokio::test]
    async fn every_dropped_blob_is_named_in_a_warning() {
        let files = archive(&["/api/uploads/../../app/.env"]);
        let plan = plan_restore(&files);

        let warnings = restore_backup_files(&files, &plan).await;

        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("../../app/.env") && warnings[0].contains("not restored"),
            "unexpected warning: {:?}",
            warnings[0]
        );
    }

    #[test]
    fn warning_lists_are_bounded() {
        let lines: Vec<String> = (0..MAX_LISTED_WARNINGS + 4)
            .map(|i| format!("line {i}"))
            .collect();

        let capped = cap_warnings(lines, "unrestored file(s)");

        assert_eq!(capped.len(), MAX_LISTED_WARNINGS + 1);
        assert!(capped.last().unwrap().contains("and 4 more unrestored"));
    }

    #[test]
    fn labels_are_bounded_too() {
        let short = short_label("a.pdf");
        assert_eq!(short, "a.pdf");

        let long = short_label(&"é".repeat(500));
        assert!(long.chars().count() <= 101, "label was not truncated");
    }
}
