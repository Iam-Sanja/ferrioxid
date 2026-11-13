//! TOTP nach RFC 6238 mit SHA-1, Wiederherstellungscodes und kurzlebige Beweise.
//!
//! SHA-1 ist hier nur der HMAC des Zählers, weil Authenticator-Apps ihn erwarten.
//! Das Geheimnis hat 160 Bit und liegt verschlüsselt in der Datenbank.

use crate::error::AuthError;
use crate::service::constant_eq;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::Sha256;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Schlüssel, mit dem ältere TOTP-Geheimnisse und Recovery-Hashes gebaut wurden.
/// Das ist HMAC-SHA256 über das CSRF-Geheimnis, nicht der aktuelle MFA-Schlüssel.
pub fn legacy_mfa_key(csrf_secret: &[u8; 32]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(csrf_secret).expect("mfa key length");
    mac.update(b"ferrioxid-mfa-v1");
    mac.finalize().into_bytes().into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealFormat {
    /// `v2.` und 24 Hex-Zeichen. Nur mit `FERRIOXID_MFA_KEY` zu öffnen.
    Current,
    /// Genau 24 Hex-Zeichen. Nur mit dem vom CSRF-Geheimnis abgeleiteten Schlüssel zu öffnen.
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryFormat {
    /// `v2.` und 64 Hex-Zeichen. HMAC, kein Geheimtext.
    Current,
    /// Genau 64 Hex-Zeichen. HMAC mit dem Legacy-Schlüssel, kein Geheimtext.
    Legacy,
}

pub fn seal_format(nonce: &str) -> Result<SealFormat, AuthError> {
    if let Some(rest) = nonce.strip_prefix("v2.") {
        if is_hex(rest, 24) {
            return Ok(SealFormat::Current);
        }
        return Err(AuthError::Validation("mfa secret could not be read".into()));
    }
    if is_hex(nonce, 24) {
        Ok(SealFormat::Legacy)
    } else {
        Err(AuthError::Validation("mfa secret could not be read".into()))
    }
}

pub fn recovery_format(value: &str) -> Option<RecoveryFormat> {
    if let Some(rest) = value.strip_prefix("v2.") {
        return is_hex(rest, 64).then_some(RecoveryFormat::Current);
    }
    is_hex(value, 64).then_some(RecoveryFormat::Legacy)
}

pub fn current_nonce(nonce_hex: &str) -> String {
    format!("v2.{nonce_hex}")
}

pub fn open_stored(
    mfa_key: &[u8; 32],
    legacy_key: &[u8; 32],
    user_id: Uuid,
    nonce: &str,
    ciphertext: &str,
) -> Result<Vec<u8>, AuthError> {
    match seal_format(nonce)? {
        SealFormat::Current => {
            let Some(nonce_hex) = nonce.strip_prefix("v2.") else {
                return Err(AuthError::Validation("mfa secret could not be read".into()));
            };
            open(mfa_key, user_id, nonce_hex, ciphertext)
        }
        SealFormat::Legacy => open(legacy_key, user_id, nonce, ciphertext),
    }
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|char| char.is_ascii_hexdigit())
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
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: user_id.as_bytes(),
            },
        )
        .map_err(|_| AuthError::Validation("mfa secret could not be stored".into()))?;
    Ok((hex(nonce.as_slice()), hex(&ciphertext)))
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
    keyed_recovery(key, b"ferrioxid-recovery-v1", user_id, normalized)
}

pub fn current_recovery_record(key: &[u8; 32], user_id: Uuid, normalized: &str) -> String {
    format!(
        "v2.{}",
        keyed_recovery(key, b"ferrioxid-recovery-v2", user_id, normalized)
    )
}

fn keyed_recovery(key: &[u8; 32], label: &[u8], user_id: Uuid, normalized: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("recovery key");
    mac.update(label);
    mac.update(user_id.as_bytes());
    mac.update(normalized.as_bytes());
    hex(&mac.finalize().into_bytes())
}

pub fn reauth_proof(key: &[u8; 32], user_id: Uuid, session_id: Uuid, expires_at: i64) -> String {
    format!(
        "v2.{expires_at}.{}",
        reauth_mac(key, b"ferrioxid-reauth-v2", user_id, session_id, expires_at)
    )
}

#[cfg(test)]
pub fn legacy_reauth_proof(
    key: &[u8; 32],
    user_id: Uuid,
    session_id: Uuid,
    expires_at: i64,
) -> String {
    format!(
        "v1.{expires_at}.{}",
        reauth_mac(key, b"ferrioxid-reauth-v1", user_id, session_id, expires_at)
    )
}

pub fn reauth_valid(
    mfa_key: &[u8; 32],
    legacy_key: &[u8; 32],
    user_id: Uuid,
    session_id: Uuid,
    proof: &str,
    now: i64,
) -> bool {
    let mut parts = proof.split('.');
    let version = parts.next().unwrap_or("");
    let expires = parts.next().unwrap_or("");
    let mac = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return false;
    }
    let key = match version {
        "v2" => mfa_key,
        "v1" => legacy_key,
        _ => return false,
    };
    let label: &[u8] = if version == "v2" {
        b"ferrioxid-reauth-v2"
    } else {
        b"ferrioxid-reauth-v1"
    };
    let Ok(expires_at) = expires.parse::<i64>() else {
        return false;
    };
    if expires_at <= now || expires_at > now.saturating_add(300) {
        return false;
    }
    constant_eq(
        &reauth_mac(key, label, user_id, session_id, expires_at),
        mac,
    )
}

fn reauth_mac(
    key: &[u8; 32],
    label: &[u8],
    user_id: Uuid,
    session_id: Uuid,
    expires_at: i64,
) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("reauth key");
    mac.update(label);
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
    use super::{
        accept_totp, legacy_mfa_key, legacy_reauth_proof, open, open_stored, recovery_format,
        recovery_hash, seal, seal_format, totp_code, RecoveryFormat, SealFormat,
    };
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
        let legacy = legacy_mfa_key(b"ferrioxid-dev-csrf-key-32-bytes!");
        let mfa = *b"ferrioxid-dev-mfa-key-32-bytes!!";
        let user = Uuid::new_v4();
        let secret = b"12345678901234567890";
        let (nonce, ciphertext) = seal(&legacy, user, secret).unwrap();
        assert!(!ciphertext.contains("12345678901234567890"));
        assert_eq!(open(&legacy, user, &nonce, &ciphertext).unwrap(), secret);
        assert!(open(&legacy, Uuid::new_v4(), &nonce, &ciphertext).is_err());
        assert_eq!(seal_format(&nonce).unwrap(), SealFormat::Legacy);
        assert!(seal_format(&super::current_nonce(&nonce)).is_ok());
        let code = totp_code(secret, 1_700_000_000, 6).unwrap();
        let step = accept_totp(secret, &code, 1_700_000_000, None).unwrap();
        assert!(accept_totp(secret, &code, 1_700_000_000, Some(step)).is_err());
        let hash = recovery_hash(&legacy, user, "ABCDEFGHJK");
        assert_eq!(recovery_format(&hash), Some(RecoveryFormat::Legacy));
        assert!(seal_format(&hash).is_err());
        assert_ne!(hash, "ABCDEFGHJK");
        let current = super::current_recovery_record(&mfa, user, "ABCDEFGHJK");
        assert_eq!(recovery_format(&current), Some(RecoveryFormat::Current));
        assert!(seal_format(&current).is_err());
        assert_ne!(current, hash);
        assert!(recovery_format("$argon2id$v=19$m=19456,t=2,p=1$salt$hash").is_none());
        let stored = open_stored(&mfa, &legacy, user, &nonce, &ciphertext).unwrap();
        assert_eq!(stored, secret);
        let proof = super::reauth_proof(&mfa, user, user, 1_300);
        assert!(super::reauth_valid(
            &mfa, &legacy, user, user, &proof, 1_000
        ));
        assert!(!super::reauth_valid(
            &mfa,
            &legacy,
            user,
            Uuid::new_v4(),
            &proof,
            1_000
        ));
        let legacy_proof = legacy_reauth_proof(&legacy, user, user, 1_300);
        assert!(super::reauth_valid(
            &mfa,
            &legacy,
            user,
            user,
            &legacy_proof,
            1_000
        ));
        let other_legacy = legacy_mfa_key(b"rotated-csrf-secret-32-bytes!!!!");
        assert!(!super::reauth_valid(
            &mfa,
            &other_legacy,
            user,
            user,
            &legacy_proof,
            1_000
        ));
        assert!(super::reauth_valid(
            &mfa,
            &other_legacy,
            user,
            user,
            &proof,
            1_000
        ));
        let expired = super::reauth_proof(&mfa, user, user, 1_000);
        assert!(!super::reauth_valid(
            &mfa, &legacy, user, user, &expired, 1_000
        ));
        let too_long = super::reauth_proof(&mfa, user, user, 1_301);
        assert!(!super::reauth_valid(
            &mfa, &legacy, user, user, &too_long, 1_000
        ));
        assert!(!proof.contains("ferrioxid-dev"));
        assert!(!legacy_proof.contains("ferrioxid-dev"));
    }
}
