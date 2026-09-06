use crate::error::{AppError, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn random_token() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
pub fn hash(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(s.as_bytes()))
}
pub fn equal(a: &str, b: &str) -> bool {
    // Hash to a fixed length before comparison, including malformed input.
    bool::from(Sha256::digest(a.as_bytes()).ct_eq(&Sha256::digest(b.as_bytes())))
}
pub fn sign(key: &[u8; 32], purpose: &str, message: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("fixed key length");
    mac.update(purpose.as_bytes());
    mac.update(&[0]);
    mac.update(message.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}
pub fn encrypt(key: &[u8; 32], aad: &str, plain: &[u8]) -> Result<String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| AppError::internal())?;
    let mut bytes = nonce.to_vec();
    bytes.extend(encrypted);
    Ok(format!("v1.{}", URL_SAFE_NO_PAD.encode(bytes)))
}
pub fn decrypt(key: &[u8; 32], aad: &str, encrypted: &str) -> Result<Vec<u8>> {
    let encoded = encrypted
        .strip_prefix("v1.")
        .ok_or_else(AppError::internal)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| AppError::internal())?;
    if bytes.len() < 40 {
        return Err(AppError::internal());
    }
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&bytes[..24]),
            Payload {
                msg: &bytes[24..],
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| AppError::internal())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_encryption_rejects_substitution() {
        let key = [7; 32];
        let encrypted = encrypt(&key, "owner/connection", b"private").unwrap();
        assert_eq!(
            decrypt(&key, "owner/connection", &encrypted).unwrap(),
            b"private"
        );
        assert!(decrypt(&key, "other/connection", &encrypted).is_err());
        assert!(decrypt(&[8; 32], "owner/connection", &encrypted).is_err());
        assert_ne!(
            encrypted,
            encrypt(&key, "owner/connection", b"private").unwrap()
        );
    }
    #[test]
    fn signatures_are_domain_separated() {
        assert_ne!(sign(&[1; 32], "csrf", "a"), sign(&[1; 32], "feed", "a"));
        assert!(equal("hello", "hello"));
        assert!(!equal("hello", "hell"));
    }
}
