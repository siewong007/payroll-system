-- Scope QR-token revocation to the display surface that issued the token.
--
-- Generating a token revokes the company's previously issued ones, so the code
-- on screen is the only valid one. That is right for a single kiosk, but the
-- kiosk-credential feature supports several labelled kiosks per company: on
-- staggered refresh cycles each kiosk was revoking the others' tokens, so an
-- employee scanning a still-displayed QR got "revoked". Recording which kiosk
-- issued a token lets revocation stay per-surface. NULL means an
-- admin/console-generated token, which forms its own group.
ALTER TABLE attendance_qr_tokens
    ADD COLUMN kiosk_credential_id uuid
        REFERENCES attendance_kiosk_credentials(id) ON DELETE CASCADE;

-- Revocation and validation both look up live tokens for one issuer.
CREATE INDEX idx_attendance_qr_tokens_issuer
    ON attendance_qr_tokens (company_id, kiosk_credential_id)
    WHERE used = FALSE;
