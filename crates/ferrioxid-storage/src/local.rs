//! Dateien liegen unter `{root}/{owner}/{id}`. Der Client-Dateiname wird nie zum Pfad.

use crate::Storage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredFile {
    pub id: Uuid,
    pub owner: Uuid,
    pub original_name: String,
    pub content_type: String,
    pub size: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StorageError {
    #[error("file was not found")]
    NotFound,
    #[error("file type is not allowed")]
    TypeRejected,
    #[error("upload is too large")]
    TooLarge,
    #[error("upload could not be stored")]
    Failed,
}

#[derive(Clone)]
pub struct LocalStorage {
    root: PathBuf,
    max_bytes: u64,
    allow: Vec<String>,
}

impl LocalStorage {
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    pub fn open(
        root: impl Into<PathBuf>,
        max_bytes: u64,
        allow: &[&str],
    ) -> Result<Self, StorageError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|_| StorageError::Failed)?;
        Ok(Self {
            root,
            max_bytes,
            allow: allow.iter().map(|value| (*value).to_owned()).collect(),
        })
    }

    fn owner_dir(&self, owner: Uuid) -> PathBuf {
        self.root.join(owner.to_string())
    }

    fn contained(root: &Path, candidate: &Path) -> Result<PathBuf, StorageError> {
        let root = canonicalize_existing(root)?;
        let file = canonicalize_existing(candidate)?;
        if file.starts_with(&root) {
            Ok(file)
        } else {
            Err(StorageError::NotFound)
        }
    }
}

#[async_trait]
impl Storage for LocalStorage {
    async fn save(
        &self,
        owner: Uuid,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<StoredFile, StorageError> {
        if bytes.len() as u64 > self.max_bytes {
            return Err(StorageError::TooLarge);
        }
        let Some(content_type) = detect_type(bytes) else {
            return Err(StorageError::TypeRejected);
        };
        if !self.allow.iter().any(|allowed| allowed == content_type) {
            return Err(StorageError::TypeRejected);
        }
        let id = Uuid::new_v4();
        let directory = self.owner_dir(owner);
        std::fs::create_dir_all(&directory).map_err(|_| StorageError::Failed)?;
        let partial = directory.join(format!("{id}.partial"));
        let target = directory.join(id.to_string());
        let meta_path = directory.join(format!("{id}.json"));
        if let Err(error) = write_new(&partial, bytes) {
            let _ = std::fs::remove_file(&partial);
            return Err(error);
        }
        if std::fs::rename(&partial, &target).is_err() {
            let _ = std::fs::remove_file(&partial);
            let _ = std::fs::remove_file(&target);
            return Err(StorageError::Failed);
        }
        let stored = StoredFile {
            id,
            owner,
            original_name: sanitize_name(original_name),
            content_type: content_type.to_owned(),
            size: bytes.len() as u64,
        };
        let json = serde_json::to_vec(&stored).map_err(|_| StorageError::Failed)?;
        if write_new(&meta_path, &json).is_err() {
            let _ = std::fs::remove_file(&target);
            let _ = std::fs::remove_file(&meta_path);
            return Err(StorageError::Failed);
        }
        Ok(stored)
    }

    async fn read(&self, owner: Uuid, id: Uuid) -> Result<(StoredFile, Vec<u8>), StorageError> {
        let meta = self.metadata(owner, id).await?;
        let path = Self::contained(
            &self.owner_dir(owner),
            &self.owner_dir(owner).join(id.to_string()),
        )?;
        let bytes = std::fs::read(path).map_err(|_| StorageError::NotFound)?;
        Ok((meta, bytes))
    }

    async fn delete(&self, owner: Uuid, id: Uuid) -> Result<(), StorageError> {
        let directory = self.owner_dir(owner);
        let file = Self::contained(&directory, &directory.join(id.to_string()))?;
        let meta = Self::contained(&directory, &directory.join(format!("{id}.json")))?;
        std::fs::remove_file(file).map_err(|_| StorageError::NotFound)?;
        let _ = std::fs::remove_file(meta);
        Ok(())
    }

    async fn exists(&self, owner: Uuid, id: Uuid) -> Result<bool, StorageError> {
        Ok(self.metadata(owner, id).await.is_ok())
    }

    async fn metadata(&self, owner: Uuid, id: Uuid) -> Result<StoredFile, StorageError> {
        let directory = self.owner_dir(owner);
        let path = Self::contained(&directory, &directory.join(format!("{id}.json")))?;
        let bytes = std::fs::read(path).map_err(|_| StorageError::NotFound)?;
        let stored: StoredFile =
            serde_json::from_slice(&bytes).map_err(|_| StorageError::Failed)?;
        if stored.owner != owner || stored.id != id {
            return Err(StorageError::NotFound);
        }
        Ok(stored)
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, StorageError> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(StorageError::NotFound),
        Err(_) => Err(StorageError::Failed),
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| StorageError::Failed)?;
    std::io::Write::write_all(&mut file, bytes).map_err(|_| StorageError::Failed)?;
    Ok(())
}

pub fn detect_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("application/pdf");
    }
    if bytes.starts_with(b"MZ") || bytes.starts_with(b"\x7fELF") {
        return None;
    }
    if looks_plain_text(bytes) {
        return Some("text/plain");
    }
    None
}

fn looks_plain_text(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.len() <= 1_048_576
        && !contains_markup(bytes)
        && bytes
            .iter()
            .all(|byte| matches!(byte, b'\n' | b'\r' | b'\t' | 0x20..=0x7f))
}

fn contains_markup(bytes: &[u8]) -> bool {
    let lower = bytes.to_ascii_lowercase();
    lower
        .windows(5)
        .any(|window| window == b"<html" || window == b"<svg " || window == b"<script")
        || lower.windows(4).any(|window| window == b"<svg")
}

pub fn sanitize_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("file");
    let cleaned: String = base
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '_'))
        .take(80)
        .collect();
    if cleaned.is_empty() {
        "file".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_type, sanitize_name, LocalStorage};
    use crate::Storage;
    use uuid::Uuid;

    #[tokio::test]
    async fn stores_reads_and_isolates_owners() {
        let root = std::env::temp_dir().join(format!("ferrioxid-storage-{}", Uuid::new_v4()));
        let storage = LocalStorage::open(&root, 32, &["text/plain", "image/png"]).unwrap();
        let ada = Uuid::new_v4();
        let grace = Uuid::new_v4();
        let stored = storage
            .save(ada, "../secret.txt", b"hello ferrioxid")
            .await
            .unwrap();
        let again = storage
            .save(ada, "../secret.txt", b"hello ferrioxid")
            .await
            .unwrap();
        assert_ne!(stored.id, again.id);
        assert_eq!(stored.original_name, "secret.txt");
        assert_ne!(stored.id.to_string(), stored.original_name);
        let (_, bytes) = storage.read(ada, stored.id).await.unwrap();
        assert_eq!(bytes, b"hello ferrioxid");
        assert!(storage.read(grace, stored.id).await.is_err());
        storage.delete(ada, stored.id).await.unwrap();
        assert!(storage.read(ada, stored.id).await.is_err());
        assert!(storage
            .save(ada, "big.txt", b"0123456789abcdef0123456789abcdef!!!")
            .await
            .is_err());
        assert!(storage
            .save(ada, "page.html", b"<html>nope</html>")
            .await
            .is_err());
        assert_eq!(detect_type(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sanitize_name("../../etc/passwd"), "passwd");
        let _ = std::fs::remove_dir_all(root);
    }
}
