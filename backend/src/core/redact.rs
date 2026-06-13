//! Centralized redaction helpers for logs, traces, and error context.
//!
//! Use these whenever a value that may contain PII (email, phone, IC number)
//! would otherwise be written to a log line or span. Prefer redaction at the
//! call site over logging the raw value and relying on downstream scrubbing.

/// Mask an email address for logging: keep the first character of the local
/// part and the domain, replace the rest of the local part with `*`.
///
/// `alice@example.com` -> `a***@example.com`
/// `x@example.com`     -> `*@example.com`
/// `not-an-email`      -> `***` (no `@`, so redact entirely)
pub fn email(value: &str) -> String {
    match value.split_once('@') {
        Some((local, domain)) if !local.is_empty() => {
            let first = local.chars().next().unwrap();
            if local.len() == 1 {
                format!("*@{}", domain)
            } else {
                format!("{}***@{}", first, domain)
            }
        }
        _ => "***".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_local_part() {
        assert_eq!(email("alice@example.com"), "a***@example.com");
    }

    #[test]
    fn masks_single_char_local() {
        assert_eq!(email("x@example.com"), "*@example.com");
    }

    #[test]
    fn redacts_non_email() {
        assert_eq!(email("not-an-email"), "***");
        assert_eq!(email("@example.com"), "***");
    }
}
