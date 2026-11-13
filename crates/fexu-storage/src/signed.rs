//! HMAC-Download-Links. Der Schlüssel kommt vom Aufrufer und steht nicht im Token.

use crate::StorageError;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

type MacSha256 = Hmac<Sha256>;

pub fn sign_download(
    key: &[u8],
    owner: Uuid,
    id: Uuid,
    expires_at: i64,
) -> Result<String, StorageError> {
    if key.len() < 32 || expires_at <= 0 {
        return Err(StorageError::Failed);
    }
    let body = format!("v1.{owner}.{id}.{expires_at}");
    let mac = mac(key, body.as_bytes())?;
    Ok(format!("{body}.{mac}"))
}

pub fn open_download(key: &[u8], token: &str, now: i64) -> Result<(Uuid, Uuid), StorageError> {
    if key.len() < 32 {
        return Err(StorageError::Failed);
    }
    let mut parts = token.split('.');
    let version = parts.next().unwrap_or("");
    let owner = parts.next().unwrap_or("");
    let id = parts.next().unwrap_or("");
    let expires = parts.next().unwrap_or("");
    let presented = parts.next().unwrap_or("");
    if version != "v1" || parts.next().is_some() {
        return Err(StorageError::NotFound);
    }
    let expires_at: i64 = expires.parse().map_err(|_| StorageError::NotFound)?;
    let owner_id = Uuid::parse_str(owner).map_err(|_| StorageError::NotFound)?;
    let file_id = Uuid::parse_str(id).map_err(|_| StorageError::NotFound)?;
    let body = format!("v1.{owner}.{id}.{expires}");
    let expected = mac(key, body.as_bytes())?;
    if !constant_eq(presented.as_bytes(), expected.as_bytes()) {
        return Err(StorageError::NotFound);
    }
    if expires_at <= now {
        return Err(StorageError::Expired);
    }
    Ok((owner_id, file_id))
}

fn mac(key: &[u8], body: &[u8]) -> Result<String, StorageError> {
    let mut mac = MacSha256::new_from_slice(key).map_err(|_| StorageError::Failed)?;
    mac.update(body);
    Ok(hex(&mac.finalize().into_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::{open_download, sign_download};
    use uuid::Uuid;

    #[test]
    fn signed_download_rejects_tampering_and_hides_the_key() {
        let key = b"0123456789abcdef0123456789abcdef";
        let owner = Uuid::new_v4();
        let id = Uuid::new_v4();
        let token = sign_download(key, owner, id, 1_700).expect("sign");
        assert!(!token.contains("0123456789abcdef"));
        assert_eq!(
            open_download(key, &token, 1_699).expect("open"),
            (owner, id)
        );
        assert_eq!(
            open_download(key, &token, 1_700),
            Err(crate::StorageError::Expired)
        );
        let mut forged = token.clone().into_bytes();
        let last = forged.len() - 1;
        forged[last] = if forged[last] == b'0' { b'1' } else { b'0' };
        let forged = String::from_utf8(forged).expect("hex");
        assert_eq!(
            open_download(key, &forged, 1_699),
            Err(crate::StorageError::NotFound)
        );
        assert_eq!(
            open_download(key, &forged, 1_700),
            Err(crate::StorageError::NotFound)
        );
        let other = b"abcdef0123456789abcdef0123456789";
        assert!(open_download(other, &token, 1_699).is_err());
        assert!(sign_download(b"short", owner, id, 1_700).is_err());
    }
}
