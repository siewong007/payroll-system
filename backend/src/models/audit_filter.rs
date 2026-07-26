//! The vocabulary available in the audit trail's filters.
//!
//! The filter options are read back out of the tenant's own `audit_logs` rows
//! rather than declared anywhere. There is no list of entity types or actions
//! in Rust, in TypeScript, or in a lookup table — the dropdown is a projection
//! of history, so it cannot drift from what the code writes and cannot offer an
//! option that matches nothing.
//!
//! That matters here more than usual. The 31 action strings the backend writes
//! follow three incompatible conventions — bare verbs (`create`), entity-suffixed
//! verbs (`create_employee`), and verb+entity+`_admin` (`create_claim_admin`) —
//! and one of them is a runtime `&str` parameter in `payroll_lifecycle_service`,
//! so no static list could be complete even in principle. The frontend's
//! hardcoded list had drifted in both directions: it offered `login`, which is
//! never written, and omitted 22 actions that are.
//!
//! Labels are *derived*, not tabulated. The only table here is a set of tokens
//! that should stay upper-case, which is a fact about English rather than a
//! second copy of the vocabulary — a new entity type gets a sensible label with
//! no edit to this file.

use serde::Serialize;

/// One selectable value plus how it should read in the UI.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FilterOption {
    pub value: String,
    pub label: String,
}

impl FilterOption {
    pub fn new(value: String) -> Self {
        let label = humanize(&value);
        Self { value, label }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditFilterOptions {
    pub entity_types: Vec<FilterOption>,
    pub actions: Vec<FilterOption>,
}

/// Tokens that read wrong in sentence case. Acronyms and initialisms only —
/// nothing here names an entity or an action, so this does not grow when the
/// audited vocabulary does.
const UPPERCASE_TOKENS: &[&str] = &[
    "qr", "epf", "socso", "eis", "pcb", "ea", "id", "ic", "tin", "hr", "ics", "pdf", "csv", "ip",
    "api", "url", "otp", "sql", "tp3",
];

/// `create_claim_admin` → `Create claim`, `attendance_qr_token` → `Attendance QR token`.
///
/// The trailing `_admin` marker distinguishes an administrator acting on
/// someone's behalf from the employee's own portal action. That is a fact about
/// *who* acted, which the audit row already records in `user_id`, so repeating
/// it in the label only makes the dropdown harder to scan.
pub fn humanize(key: &str) -> String {
    let trimmed = key.strip_suffix("_admin").unwrap_or(key);

    let mut label = String::with_capacity(trimmed.len());
    for (index, token) in trimmed.split('_').filter(|t| !t.is_empty()).enumerate() {
        if index > 0 {
            label.push(' ');
        }
        if UPPERCASE_TOKENS.contains(&token) {
            label.push_str(&token.to_uppercase());
        } else if index == 0 {
            let mut chars = token.chars();
            if let Some(first) = chars.next() {
                label.extend(first.to_uppercase());
                label.push_str(chars.as_str());
            }
        } else {
            label.push_str(token);
        }
    }

    if label.is_empty() {
        // A blank or all-underscore value should never reach here, but showing
        // the raw key beats showing an empty dropdown entry.
        return key.to_string();
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_verbs_read_as_sentences() {
        assert_eq!(humanize("create"), "Create");
        assert_eq!(humanize("submit_approval"), "Submit approval");
        assert_eq!(humanize("return_changes"), "Return changes");
    }

    #[test]
    fn the_admin_suffix_is_dropped() {
        // `user_id` already records who acted; repeating it in every label just
        // makes the list harder to scan.
        assert_eq!(humanize("create_claim_admin"), "Create claim");
        assert_eq!(
            humanize("cancel_leave_request_admin"),
            "Cancel leave request"
        );
        assert_eq!(humanize("delete_overtime_admin"), "Delete overtime");
    }

    #[test]
    fn acronyms_stay_upper_case() {
        assert_eq!(humanize("attendance_qr_token"), "Attendance QR token");
        assert_eq!(humanize("qr_generated"), "QR generated");
    }

    #[test]
    fn entity_types_read_as_sentences() {
        assert_eq!(humanize("payroll_run"), "Payroll run");
        assert_eq!(humanize("user_group_member"), "User group member");
        assert_eq!(humanize("company_setting"), "Company setting");
        assert_eq!(
            humanize("attendance_kiosk_credential"),
            "Attendance kiosk credential"
        );
    }

    /// Every value the backend writes today must produce a readable label
    /// without an entry in an override table. This is not a registry of the
    /// vocabulary — nothing reads it at runtime and a missing entry breaks
    /// nothing — it is a sample proving the derivation covers the real shapes.
    #[test]
    fn todays_vocabulary_derives_without_overrides() {
        let cases = [
            ("approve_leave", "Approve leave"),
            ("reject_overtime", "Reject overtime"),
            ("bulk_import", "Bulk import"),
            ("create_employee", "Create employee"),
            ("update_employee", "Update employee"),
            ("send", "Send"),
            ("revoke", "Revoke"),
            ("lock", "Lock"),
            ("process", "Process"),
            ("letter", "Letter"),
            ("geofence_mode", "Geofence mode"),
            ("platform_attendance_method", "Platform attendance method"),
        ];
        for (key, expected) in cases {
            assert_eq!(humanize(key), expected, "label for {key}");
        }
    }

    #[test]
    fn an_unknown_future_value_still_reads_sensibly() {
        // The point of deriving rather than tabulating: a value added next
        // month gets a label with no edit here.
        assert_eq!(humanize("invoice_batch"), "Invoice batch");
        assert_eq!(humanize("archive_pdf_export"), "Archive PDF export");
    }

    #[test]
    fn degenerate_input_falls_back_to_the_raw_key() {
        assert_eq!(humanize("___"), "___");
        assert_eq!(humanize(""), "");
    }

    #[test]
    fn filter_option_labels_itself() {
        let option = FilterOption::new("team_member".to_string());
        assert_eq!(option.value, "team_member");
        assert_eq!(option.label, "Team member");
    }
}
