//! Ownership facts for files served from `/api/uploads/{filename}`.
//!
//! An uploaded file has no owning row of its own — `portal::upload_file` writes
//! bytes to disk and hands the caller a URL, and only later does some record
//! store that URL. The referencing record is therefore the *only* evidence of
//! who a file belongs to. [`UploadReference`] is one such record found by the
//! reverse lookup in `repositories::reads::uploads`; [`UploadAccess`] is the
//! caller's side of the comparison, resolved from `AuthUser` by the handler.

use uuid::Uuid;

/// The kind of record that references an uploaded file.
///
/// The kind decides which permission covers the file when the caller is not the
/// employee the record is about: claim receipts and leave attachments belong to
/// the approvals surface, everything under Documents to the documents surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadReferenceKind {
    Claim,
    LeaveRequest,
    Document,
}

impl UploadReferenceKind {
    /// Parses the discriminator the reverse-lookup query emits.
    ///
    /// Returns `None` for anything unrecognised so a future `UNION` branch added
    /// to the query without a matching variant here fails closed — an unknown
    /// kind grants nothing rather than being treated as the first variant.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "claim" => Some(Self::Claim),
            "leave_request" => Some(Self::LeaveRequest),
            "document" => Some(Self::Document),
            _ => None,
        }
    }
}

/// A record that references an uploaded file, within one company.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadReference {
    pub kind: UploadReferenceKind,
    /// The employee the referencing record is about. `None` for a company-wide
    /// document, which has no self-service owner and is reachable only through
    /// the documents permission.
    pub employee_id: Option<Uuid>,
}

/// What the caller is entitled to, lifted out of `AuthUser` so the authorization
/// rule stays a pure function of plain data and can be unit-tested without a
/// token, a database, or a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadAccess {
    pub company_id: Uuid,
    /// The caller's own employee profile, when they have one. Users who
    /// administer a company without being an employee of it hold `None`.
    pub employee_id: Option<Uuid>,
    pub can_view_approvals: bool,
    pub can_view_documents: bool,
}
