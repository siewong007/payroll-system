use aes_gcm::aead::{Aead, Generate};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

use super::error::{AppError, AppResult};

const NONCE_LEN: usize = 12;

/// Derives a symmetric key from JWT_SECRET, scoped by domain so it can never
/// collide with the JWT signing use of the same secret.
fn derive_key(jwt_secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"totp-secret-encryption-key:");
    hasher.update(jwt_secret.as_bytes());
    hasher.finalize().into()
}

/// Fills all 12 bytes straight from the OS CSPRNG. This used to truncate a
/// UUIDv4, whose fixed version/variant bits left only ~90 bits of variability.
fn random_nonce() -> [u8; NONCE_LEN] {
    <[u8; NONCE_LEN]>::generate()
}

/// Encrypts a secret at rest (AES-256-GCM) with a key derived from JWT_SECRET.
/// Returns base64(nonce || ciphertext). Rotating JWT_SECRET makes prior
/// ciphertexts undecryptable, which is acceptable — rotating it already
/// invalidates all sessions.
pub fn encrypt_secret(plaintext: &str, jwt_secret: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new_from_slice(&derive_key(jwt_secret))
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

/// Reverses `encrypt_secret`.
pub fn decrypt_secret(encoded: &str, jwt_secret: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new_from_slice(&derive_key(jwt_secret))
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
        let jwt_secret = "test-jwt-secret";

        let encrypted = encrypt_secret(secret, jwt_secret).unwrap();
        assert_ne!(encrypted, secret);

        let decrypted = decrypt_secret(&encrypted, jwt_secret).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn different_nonces_each_call() {
        let secret = "JBSWY3DPEHPK3PXP";
        let jwt_secret = "test-jwt-secret";

        let a = encrypt_secret(secret, jwt_secret).unwrap();
        let b = encrypt_secret(secret, jwt_secret).unwrap();
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
}
