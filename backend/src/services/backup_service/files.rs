use std::collections::HashMap;
use std::path::Path;

use base64::Engine;

use crate::models::backup::{ClaimExport, DocumentExport, LeaveRequestExport};

pub fn collect_backup_files(
    documents: &[DocumentExport],
    leave_requests: &[LeaveRequestExport],
    claims: &[ClaimExport],
) -> HashMap<String, String> {
    let mut files = HashMap::new();
    let upload_dir = Path::new("uploads");
    let b64 = base64::engine::general_purpose::STANDARD;

    let mut collect_file = |url: Option<&String>| {
        if let Some(u) = url
            && let Some(filename) = u.strip_prefix("/api/uploads/")
        {
            let path = upload_dir.join(filename);
            if let Ok(data) = std::fs::read(&path) {
                files.insert(u.clone(), b64.encode(&data));
            }
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

pub async fn restore_backup_files(files: &HashMap<String, String>) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    let upload_dir = Path::new("uploads");
    let _ = tokio::fs::create_dir_all(upload_dir).await;
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut files_restored = 0usize;

    for (url, data_b64) in files {
        if let Some(filename) = url.strip_prefix("/api/uploads/")
            && let Ok(data) = b64.decode(data_b64)
        {
            let path = upload_dir.join(filename);
            if tokio::fs::write(&path, &data).await.is_ok() {
                files_restored += 1;
            }
        }
    }

    (files_restored > 0)
        .then(|| format!("{} file(s) restored to uploads directory.", files_restored))
}
