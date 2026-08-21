use aes_gcm::aead::{Aead, Generate};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

use super::error::{AppError, AppResult};

const NONCE_LEN: usize = 12;

/// Domain string for the current derivation, from `TOTP_ENCRYPTION_KEY`.
const KEY_DOMAIN: &[u8] = b"totp-secret-encryption-key:v2:";
/// Domain string of the pre-v2 derivation, which keyed off `JWT_SECRET`.
/// Kept only so [`decrypt_with_legacy_jwt_derivation`] can migrate rows that
/// were written before the dedicated key existed.
const LEGACY_KEY_DOMAIN: &[u8] = b"totp-secret-encryption-key:";

/// Derives a symmetric key from key material, scoped by domain.
fn derive_key(domain: &[u8], material: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(material.as_bytes());
    hasher.finalize().into()
}

/// Fills all 12 bytes straight from the OS CSPRNG. This used to truncate a
/// UUIDv4, whose fixed version/variant bits left only ~90 bits of variability.
fn random_nonce() -> [u8; NONCE_LEN] {
    <[u8; NONCE_LEN]>::generate()
}

/// Encrypts a secret at rest (AES-256-GCM) under the dedicated TOTP
/// encryption key. Returns base64(nonce || ciphertext).
///
/// The key is deliberately *not* derived from `JWT_SECRET`: rotating the JWT
/// secret after a leak must not turn every 2FA enrolment into ciphertext
/// nobody can open, which is exactly what the old shared derivation did.
pub fn encrypt_secret(plaintext: &str, totp_encryption_key: &str) -> AppResult<String> {
    encrypt_with_domain(KEY_DOMAIN, plaintext, totp_encryption_key)
}

/// Reverses `encrypt_secret`.
pub fn decrypt_secret(encoded: &str, totp_encryption_key: &str) -> AppResult<String> {
    decrypt_with_domain(KEY_DOMAIN, encoded, totp_encryption_key)
}

/// Decrypts rows written before TOTP had its own key, when the AES key was
/// `SHA256("totp-secret-encryption-key:" ‖ jwt_secret)`. Used only by the
/// startup re-encryption pass; live paths never accept this derivation.
pub fn decrypt_with_legacy_jwt_derivation(encoded: &str, jwt_secret: &str) -> AppResult<String> {
    decrypt_with_domain(LEGACY_KEY_DOMAIN, encoded, jwt_secret)
}

/// Encrypts under the legacy domain — test-only, standing in for the pre-v2
/// writer so migration tests can produce genuine legacy rows.
#[cfg(test)]
pub(crate) fn encrypt_with_legacy_jwt_derivation(
    plaintext: &str,
    jwt_secret: &str,
) -> AppResult<String> {
    encrypt_with_domain(LEGACY_KEY_DOMAIN, plaintext, jwt_secret)
}

fn encrypt_with_domain(domain: &[u8], plaintext: &str, key_material: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new_from_slice(&derive_key(domain, key_material))
        .map_err(|e| AppError::Internal(format!("Failed to init cipher: {e}")))?;

    let nonce_bytes = random_nonce();
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| AppError::Internal(format!("Failed to encrypt secret: {e}")))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(combined))
}

fn decrypt_with_domain(domain: &[u8], encoded: &str, key_material: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new_from_slice(&derive_key(domain, key_material))
        .map_err(|e| AppError::Internal(format!("Failed to init cipher: {e}")))?;

    let combined = BASE64
        .decode(encoded)
        .map_err(|e| AppError::Internal(format!("Failed to decode secret: {e}")))?;

    if combined.len() < NONCE_LEN {
        return Err(AppError::Internal("Corrupt encrypted secret".into()));
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
    let nonce_arr: [u8; NONCE_LEN] = nonce_bytes
        .try_into()
        .map_err(|_| AppError::Internal("Corrupt encrypted secret".into()))?;
    let nonce = Nonce::from(nonce_arr);

    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| AppError::Internal("Failed to decrypt secret".into()))?;

    String::from_utf8(plaintext).map_err(|e| AppError::Internal(format!("Corrupt secret: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let secret = "JBSWY3DPEHPK3PXP";
        let totp_key = "test-totp-encryption-key";

        let encrypted = encrypt_secret(secret, totp_key).unwrap();
        assert_ne!(encrypted, secret);

        let decrypted = decrypt_secret(&encrypted, totp_key).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn different_nonces_each_call() {
        let secret = "JBSWY3DPEHPK3PXP";
        let totp_key = "test-totp-encryption-key";

        let a = encrypt_secret(secret, totp_key).unwrap();
        let b = encrypt_secret(secret, totp_key).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn nonce_has_no_fixed_bits() {
        // Guards against reintroducing a nonce derived from a structured source
        // (e.g. UUIDv4, whose version/variant bits are constant).
        let mut ever_set = [0u8; NONCE_LEN];
        let mut ever_clear = [0u8; NONCE_LEN];

        for _ in 0..256 {
            let nonce = random_nonce();
            for i in 0..NONCE_LEN {
                ever_set[i] |= nonce[i];
                ever_clear[i] |= !nonce[i];
            }
        }

        assert_eq!(ever_set, [0xffu8; NONCE_LEN], "some bit was never set");
        assert_eq!(ever_clear, [0xffu8; NONCE_LEN], "some bit was never clear");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let secret = "JBSWY3DPEHPK3PXP";
        let encrypted = encrypt_secret(secret, "key-a").unwrap();
        assert!(decrypt_secret(&encrypted, "key-b").is_err());
    }

    #[test]
    fn legacy_rows_still_decrypt_under_the_jwt_derivation() {
        let secret = "JBSWY3DPEHPK3PXP";
        let jwt_secret = "the-rotated-away-jwt-secret";

        // A row written by the pre-v2 writer (key derived from JWT_SECRET).
        let legacy = encrypt_with_legacy_jwt_derivation(secret, jwt_secret).unwrap();

        assert_eq!(
            decrypt_with_legacy_jwt_derivation(&legacy, jwt_secret).unwrap(),
            secret
        );
        // …and the current key cannot read it, which is why the migration pass
        // must run before live paths see the row.
        assert!(decrypt_secret(&legacy, "any-totp-key").is_err());
    }
}
