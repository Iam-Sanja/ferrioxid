//! TOTP nach RFC 6238 mit SHA-1, Wiederherstellungscodes und kurzlebige Beweise.
//!
//! SHA-1 ist hier nur der HMAC des Zählers, weil Authenticator-Apps ihn erwarten.
//! Das Geheimnis hat 160 Bit und liegt verschlüsselt in der Datenbank.

use crate::error::AuthError;
use crate::service::constant_eq;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::Sha256;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn mfa_key(secret: &[u8; 32]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).expect("mfa key length");
    mac.update(b"ferrioxid-mfa-v1");
    mac.finalize().into_bytes().into()
}

pub fn random_secret() -> Vec<u8> {
    let mut bytes = vec![0u8; 20];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

pub fn seal(
    key: &[u8; 32],
    user_id: Uuid,
    plaintext: &[u8],
) -> Result<(String, String), AuthError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AuthError::Validation("mfa key is invalid".into()))?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: user_id.as_bytes(),
            },
        )
        .map_err(|_| AuthError::Validation("mfa secret could not be stored".into()))?;
    Ok((hex(&nonce), hex(&ciphertext)))
}

pub fn open(
    key: &[u8; 32],
    user_id: Uuid,
    nonce_hex: &str,
    cipher_hex: &str,
) -> Result<Vec<u8>, AuthError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AuthError::Validation("mfa key is invalid".into()))?;
    let nonce = decode_hex(nonce_hex)?;
    let ciphertext = decode_hex(cipher_hex)?;
    cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: user_id.as_bytes(),
            },
        )
        .map_err(|_| AuthError::Validation("mfa secret could not be read".into()))
}

pub fn totp_code(secret: &[u8], unix: i64, digits: u32) -> Result<String, AuthError> {
    if !(6..=8).contains(&digits) {
        return Err(AuthError::Validation("totp digits are invalid".into()));
    }
    let counter = unix.div_euclid(30) as u64;
    let mut mac = <HmacSha1 as Mac>::new_from_slice(secret)
        .map_err(|_| AuthError::Validation("totp secret is invalid".into()))?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    let modulus = 10u32.pow(digits);
    Ok(format!(
        "{:0width$}",
        binary % modulus,
        width = digits as usize
    ))
}

/// Prüft das aktuelle Fenster und lehnt einen bereits verbrauchten Schritt ab.
pub fn accept_totp(
    secret: &[u8],
    code: &str,
    unix: i64,
    last_step: Option<i64>,
) -> Result<i64, AuthError> {
    let current = unix.div_euclid(30);
    for step in [current - 1, current, current + 1] {
        if last_step.is_some_and(|used| step <= used) {
            continue;
        }
        let candidate = totp_code(secret, step.saturating_mul(30), 6)?;
        if constant_eq(&candidate, code.trim()) {
            return Ok(step);
        }
    }
    Err(AuthError::InvalidCredentials)
}

pub fn base32(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((buffer >> bits) & 0x1f) as usize;
            output.push(ALPHABET[index] as char);
        }
    }
    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0x1f) as usize;
        output.push(ALPHABET[index] as char);
    }
    output
}

pub fn otpauth_uri(email: &str, secret: &[u8]) -> String {
    format!(
        "otpauth://totp/Ferrioxid:{}?secret={}&issuer=Ferrioxid&algorithm=SHA1&digits=6&period=30",
        encode_component(email),
        base32(secret)
    )
}

pub fn recovery_codes() -> Vec<String> {
    (0..8).map(|_| recovery_code()).collect()
}

pub fn normalize_recovery(code: &str) -> Result<String, AuthError> {
    let normalized: String = code
        .chars()
        .filter(|char| *char != '-' && !char.is_whitespace())
        .map(|char| char.to_ascii_uppercase())
        .collect();
    if normalized.len() == 10
        && normalized
            .bytes()
            .all(|byte| RECOVERY_ALPHABET.contains(&byte))
    {
        Ok(normalized)
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

pub fn recovery_hash(key: &[u8; 32], user_id: Uuid, normalized: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("recovery key");
    mac.update(b"ferrioxid-recovery-v1");
    mac.update(user_id.as_bytes());
    mac.update(normalized.as_bytes());
    hex(&mac.finalize().into_bytes())
}

pub fn reauth_proof(key: &[u8; 32], user_id: Uuid, session_id: Uuid, expires_at: i64) -> String {
    format!(
        "v1.{expires_at}.{}",
        reauth_mac(key, user_id, session_id, expires_at)
    )
}

pub fn reauth_valid(
    key: &[u8; 32],
    user_id: Uuid,
    session_id: Uuid,
    proof: &str,
    now: i64,
) -> bool {
    let mut parts = proof.split('.');
    let version = parts.next().unwrap_or("");
    let expires = parts.next().unwrap_or("");
    let mac = parts.next().unwrap_or("");
    if version != "v1" || parts.next().is_some() {
        return false;
    }
    let Ok(expires_at) = expires.parse::<i64>() else {
        return false;
    };
    if expires_at <= now || expires_at > now.saturating_add(300) {
        return false;
    }
    constant_eq(&reauth_mac(key, user_id, session_id, expires_at), mac)
}

fn reauth_mac(key: &[u8; 32], user_id: Uuid, session_id: Uuid, expires_at: i64) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("reauth key");
    mac.update(b"ferrioxid-reauth-v1");
    mac.update(user_id.as_bytes());
    mac.update(session_id.as_bytes());
    mac.update(expires_at.to_string().as_bytes());
    hex(&mac.finalize().into_bytes())
}

fn recovery_code() -> String {
    let mut chars = String::new();
    while chars.len() < 10 {
        let mut byte = [0u8; 1];
        OsRng.fill_bytes(&mut byte);
        let index = (byte[0] & 31) as usize;
        chars.push(RECOVERY_ALPHABET[index] as char);
    }
    format!("{}-{}", &chars[..5], &chars[5..])
}

fn encode_component(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Result<Vec<u8>, AuthError> {
    if !value.len().is_multiple_of(2) || !value.chars().all(|char| char.is_ascii_hexdigit()) {
        return Err(AuthError::Validation("mfa secret could not be read".into()));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| AuthError::Validation("mfa secret could not be read".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{accept_totp, mfa_key, open, reauth_valid, recovery_hash, seal, totp_code};
    use uuid::Uuid;

    #[test]
    fn rfc6238_sha1_vectors() {
        let secret = b"12345678901234567890";
        let cases = [
            (59_i64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
            (20_000_000_000, "65353130"),
        ];
        for (unix, code) in cases {
            assert_eq!(totp_code(secret, unix, 8).unwrap(), code);
        }
    }

    #[test]
    fn sealed_secret_is_bound_to_the_user_and_codes_do_not_repeat() {
        let key = mfa_key(b"ferrioxid-dev-csrf-key-32-bytes!");
        let user = Uuid::new_v4();
        let secret = b"12345678901234567890";
        let (nonce, ciphertext) = seal(&key, user, secret).unwrap();
        assert!(!ciphertext.contains("12345678901234567890"));
        assert_eq!(open(&key, user, &nonce, &ciphertext).unwrap(), secret);
        assert!(open(&key, Uuid::new_v4(), &nonce, &ciphertext).is_err());
        let code = totp_code(secret, 1_700_000_000, 6).unwrap();
        let step = accept_totp(secret, &code, 1_700_000_000, None).unwrap();
        assert!(accept_totp(secret, &code, 1_700_000_000, Some(step)).is_err());
        let hash = recovery_hash(&key, user, "ABCDEFGHJK");
        assert_ne!(hash, "ABCDEFGHJK");
        let proof = super::reauth_proof(&key, user, user, 1_300);
        assert!(reauth_valid(&key, user, user, &proof, 1_000));
        assert!(!reauth_valid(&key, user, Uuid::new_v4(), &proof, 1_000));
        let expired = super::reauth_proof(&key, user, user, 1_000);
        assert!(!reauth_valid(&key, user, user, &expired, 1_000));
        let too_long = super::reauth_proof(&key, user, user, 1_301);
        assert!(!reauth_valid(&key, user, user, &too_long, 1_000));
        assert!(!proof.to_string().contains("ferrioxid-dev"));
    }
}
