-- Backstop for the application-level validator in `core::upload_path`.
--
-- Every column here is joined onto a filesystem path somewhere (document delete,
-- backup export, backup restore), and each sink did that by concatenating a
-- client-supplied string onto `uploads/`. `validate_file_url` now settles the
-- shape at write time; this constraint is what still holds if a *future* write
-- path forgets to call it — which is exactly how the defect arose in the first
-- place.
--
-- The rule is deliberately narrow: a value that claims to be a locally stored
-- upload (prefix '/api/uploads/') must name exactly one path component — no '/',
-- no '\', no '..'. NULLs and external http(s) links are unaffected, and no value
-- the application legitimately produces can trip it (uploads are
-- '/api/uploads/<uuid>_<name>.<ext>'). A stricter, full URL-shape whitelist is
-- NOT warranted: `documents.file_url` is a free-text field in the current UI, so
-- a strict CHECK would turn a user typo into a 500-level database error instead
-- of the 400 the validator returns.
--
-- '/api/uploads/' is 13 characters, hence substring(... from 14).
--
-- NOT VALID on purpose: migrations run on every `cargo run`, so a tenant whose
-- rows were poisoned before this fix must not be able to wedge the deploy. The
-- constraint still applies to every INSERT and UPDATE from now on.
--
-- companies.logo_url is deliberately left unconstrained: it never reaches a
-- filesystem sink. If a logo-delete path is ever added it needs both the
-- validator and a constraint of its own.

ALTER TABLE documents
    ADD CONSTRAINT documents_file_url_no_traversal CHECK (
        file_url NOT LIKE '/api/uploads/%'
        OR (
            substring(file_url from 14) ~ '^[^/\\]+$'
            AND substring(file_url from 14) !~ '\.\.'
        )
    ) NOT VALID;

ALTER TABLE leave_requests
    ADD CONSTRAINT leave_requests_attachment_url_no_traversal CHECK (
        attachment_url NOT LIKE '/api/uploads/%'
        OR (
            substring(attachment_url from 14) ~ '^[^/\\]+$'
            AND substring(attachment_url from 14) !~ '\.\.'
        )
    ) NOT VALID;

ALTER TABLE claims
    ADD CONSTRAINT claims_receipt_url_no_traversal CHECK (
        receipt_url NOT LIKE '/api/uploads/%'
        OR (
            substring(receipt_url from 14) ~ '^[^/\\]+$'
            AND substring(receipt_url from 14) !~ '\.\.'
        )
    ) NOT VALID;
