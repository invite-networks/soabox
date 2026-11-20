//! Password hashing, TOTP, encryption at rest, and random tokens.

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

/// Minimum length for local passwords.
pub const MIN_PASSWORD_LEN: usize = 12;

/// Hashes a password with argon2id and a random salt.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))
}

/// Checks a password against a stored hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

/// A hash of a fixed dummy password, used to keep sign-in timing similar
/// whether or not the username exists.
pub fn dummy_verify(password: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let hash = DUMMY.get_or_init(|| hash_password("dummy-password-for-timing").unwrap_or_default());
    let _ = verify_password(password, hash);
}

/// `n` random bytes from the operating system.
pub fn random_bytes<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| anyhow::anyhow!("random source failed: {e}"))?;
    Ok(b)
}

/// A random URL-safe token with 256 bits of entropy.
pub fn random_token() -> anyhow::Result<String> {
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes::<32>()?))
}

/// SHA-256 of a token, hex encoded. Used for session ids at rest.
pub fn token_digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Encrypts with XChaCha20-Poly1305. Output is nonce followed by ciphertext.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce_bytes = random_bytes::<24>()?;
    let nonce = XNonce::from(nonce_bytes);
    let mut out = nonce_bytes.to_vec();
    out.extend(
        cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| anyhow::anyhow!("encryption failed"))?,
    );
    Ok(out)
}

/// Decrypts data from [`encrypt`], trying each key in order.
pub fn decrypt(keys: &[&[u8; 32]], data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 24 {
        return None;
    }
    let (nonce_bytes, ct) = data.split_at(24);
    let nonce_arr: [u8; 24] = nonce_bytes.try_into().ok()?;
    let nonce = XNonce::from(nonce_arr);
    keys.iter()
        .find_map(|k| XChaCha20Poly1305::new((*k).into()).decrypt(&nonce, ct).ok())
}

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 without padding.
pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &b in data {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// A TOTP code (RFC 6238, SHA-1, 6 digits, 30 seconds) for a time step.
pub fn totp_at(secret: &[u8], step: u64) -> String {
    let Ok(mut mac) = <Hmac<Sha1> as hmac::KeyInit>::new_from_slice(secret) else {
        return String::new();
    };
    mac.update(&step.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let offset = (h[h.len() - 1] & 0x0f) as usize;
    let code = (u32::from(h[offset] & 0x7f) << 24)
        | (u32::from(h[offset + 1]) << 16)
        | (u32::from(h[offset + 2]) << 8)
        | u32::from(h[offset + 3]);
    format!("{:06}", code % 1_000_000)
}

/// Checks a TOTP code at `now`, allowing one step of clock drift.
pub fn totp_check(secret: &[u8], code: &str, now: u64) -> bool {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let step = now / 30;
    [step.saturating_sub(1), step, step + 1]
        .iter()
        .any(|s| constant_eq(totp_at(secret, *s).as_bytes(), code.as_bytes()))
}

/// The `otpauth://` URL authenticator apps read.
pub fn otpauth_url(secret: &[u8], username: &str) -> String {
    let raw_label = format!("SOAbox:{username}");
    let label =
        percent_encoding::utf8_percent_encode(&raw_label, percent_encoding::NON_ALPHANUMERIC);
    format!(
        "otpauth://totp/{label}?secret={}&issuer=SOAbox&algorithm=SHA1&digits=6&period=30",
        base32_encode(secret)
    )
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Constant-time comparison of two strings.
pub fn constant_eq_str(a: &str, b: &str) -> bool {
    constant_eq(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_round_trip() {
        let h = hash_password("correct horse battery").expect("hash");
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &h));
        assert!(!verify_password("wrong", &h));
        assert!(!verify_password("x", "not a hash"));
        dummy_verify("anything");
    }

    #[test]
    fn rfc6238_vectors() {
        // RFC 6238 appendix B, SHA-1 secret "12345678901234567890", 8 digits
        // truncated to the last 6.
        let secret = b"12345678901234567890";
        assert_eq!(totp_at(secret, 59 / 30), "287082");
        assert_eq!(totp_at(secret, 1_111_111_109 / 30), "081804");
        assert_eq!(totp_at(secret, 1_234_567_890 / 30), "005924");
    }

    #[test]
    fn totp_check_allows_one_step_drift() {
        let secret = b"12345678901234567890";
        let now = 1_234_567_890;
        let code = totp_at(secret, now / 30 - 1);
        assert!(totp_check(secret, &code, now));
        let far = totp_at(secret, now / 30 - 3);
        assert!(!totp_check(secret, &far, now));
        assert!(!totp_check(secret, "12345", now));
        assert!(!totp_check(secret, "abcdef", now));
    }

    #[test]
    fn base32() {
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_encode(b"f"), "MY");
        assert_eq!(base32_encode(b""), "");
    }

    #[test]
    fn encrypt_round_trip_and_rotation() {
        let k1 = [1u8; 32];
        let k2 = [2u8; 32];
        let ct = encrypt(&k1, b"secret").expect("enc");
        assert_ne!(&ct[24..], b"secret");
        assert_eq!(decrypt(&[&k2, &k1], &ct).expect("dec"), b"secret");
        assert!(decrypt(&[&k2], &ct).is_none());
        assert!(decrypt(&[&k1], b"short").is_none());
    }

    #[test]
    fn tokens_and_urls() {
        let t = random_token().expect("token");
        assert_eq!(t.len(), 43);
        assert_eq!(token_digest(&t).len(), 64);
        let url = otpauth_url(b"12345", "a b");
        assert!(url.starts_with("otpauth://totp/SOAbox%3Aa%20b?secret="));
        assert!(constant_eq_str("abc", "abc"));
        assert!(!constant_eq_str("abc", "abd"));
    }
}
