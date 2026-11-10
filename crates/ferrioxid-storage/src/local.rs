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
    /// The header is not a bounded PNG or JPEG. This is not a full pixel decode.
    #[error("image was rejected")]
    InvalidImage,
}

#[derive(Clone)]
pub struct LocalStorage {
    root: PathBuf,
    max_bytes: u64,
    allow: Vec<String>,
    check: Option<std::sync::Arc<dyn BeforeSave>>,
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
            check: None,
        })
    }

    /// Runs `check` before a file is created. A rejection leaves no file behind.
    pub fn with_check(mut self, check: std::sync::Arc<dyn BeforeSave>) -> Self {
        self.check = Some(check);
        self
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
        if let Some(check) = &self.check {
            check.check(original_name, bytes)?;
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

/// Optional gate before [`LocalStorage`] creates a file.
pub trait BeforeSave: Send + Sync {
    fn check(&self, original_name: &str, bytes: &[u8]) -> Result<(), StorageError>;
}

/// Bounds for [`inspect_image`]. Zero width or height is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageLimits {
    pub max_bytes: u64,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Checks a PNG or JPEG header, its declared size and that the container ends.
///
/// PNG chunks are not inflated and JPEG scans are not decoded. A later decoder
/// can still use more memory than `max_pixels` if it ignores these numbers.
/// [`detect_type`] only looks at a signature and is a weaker check.
pub fn inspect_image(bytes: &[u8], limits: ImageLimits) -> Result<ImageInfo, StorageError> {
    if bytes.len() as u64 > limits.max_bytes {
        return Err(StorageError::TooLarge);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        inspect_png(bytes, limits)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        inspect_jpeg(bytes, limits)
    } else {
        Err(StorageError::InvalidImage)
    }
}

/// Rejects images that fail [`inspect_image`].
pub struct ImageHeaderCheck {
    pub limits: ImageLimits,
}

impl BeforeSave for ImageHeaderCheck {
    fn check(&self, _original_name: &str, bytes: &[u8]) -> Result<(), StorageError> {
        inspect_image(bytes, self.limits).map(|_| ())
    }
}

fn inspect_png(bytes: &[u8], limits: ImageLimits) -> Result<ImageInfo, StorageError> {
    if bytes.len() < 33 {
        return Err(StorageError::InvalidImage);
    }
    let mut offset = 8;
    let mut info = None;
    let mut ended = false;
    while offset + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &bytes[offset + 4..offset + 8];
        let data_at = offset + 8;
        let Some(next) = data_at
            .checked_add(length)
            .and_then(|value| value.checked_add(4))
        else {
            return Err(StorageError::InvalidImage);
        };
        if next > bytes.len() {
            return Err(StorageError::InvalidImage);
        }
        if kind == b"IHDR" {
            if length != 13 || info.is_some() {
                return Err(StorageError::InvalidImage);
            }
            let width = u32::from_be_bytes(bytes[data_at..data_at + 4].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[data_at + 4..data_at + 8].try_into().unwrap());
            check_dimensions(width, height, limits)?;
            info = Some(ImageInfo {
                format: "image/png",
                width,
                height,
            });
        } else if kind == b"IEND" {
            if length != 0 {
                return Err(StorageError::InvalidImage);
            }
            ended = true;
            break;
        }
        offset = next;
    }
    if !ended {
        return Err(StorageError::InvalidImage);
    }
    info.ok_or(StorageError::InvalidImage)
}

fn inspect_jpeg(bytes: &[u8], limits: ImageLimits) -> Result<ImageInfo, StorageError> {
    if bytes.len() < 4 {
        return Err(StorageError::InvalidImage);
    }
    let mut offset = 2;
    let mut info = None;
    let mut ended = false;
    while offset < bytes.len() {
        if bytes[offset] != 0xFF {
            return Err(StorageError::InvalidImage);
        }
        while offset < bytes.len() && bytes[offset] == 0xFF {
            offset += 1;
        }
        if offset >= bytes.len() {
            break;
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xD9 {
            ended = true;
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if offset + 2 > bytes.len() {
            return Err(StorageError::InvalidImage);
        }
        let length = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if length < 2 || offset + length > bytes.len() {
            return Err(StorageError::InvalidImage);
        }
        if matches!(marker, 0xC0..=0xC2) && length >= 7 {
            let height = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]) as u32;
            check_dimensions(width, height, limits)?;
            info = Some(ImageInfo {
                format: "image/jpeg",
                width,
                height,
            });
        }
        offset += length;
    }
    if !ended {
        return Err(StorageError::InvalidImage);
    }
    info.ok_or(StorageError::InvalidImage)
}

fn check_dimensions(width: u32, height: u32, limits: ImageLimits) -> Result<(), StorageError> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > limits.max_width
        || height > limits.max_height
        || pixels > limits.max_pixels
    {
        Err(StorageError::InvalidImage)
    } else {
        Ok(())
    }
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

    fn limits() -> super::ImageLimits {
        super::ImageLimits {
            max_bytes: 1024,
            max_width: 8,
            max_height: 8,
            max_pixels: 16,
        }
    }

    fn png(width: u32, height: u32, end: bool) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        if end {
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(b"IEND");
            bytes.extend_from_slice(&[0, 0, 0, 0]);
        }
        bytes
    }

    #[test]
    fn image_header_check_rejects_truncated_and_oversized_images() {
        let image = png(1, 1, true);
        let info = super::inspect_image(&image, limits()).unwrap();
        assert_eq!(info.format, "image/png");
        assert_eq!((info.width, info.height), (1, 1));
        assert!(super::inspect_image(&image[..20], limits()).is_err());
        assert!(super::inspect_image(&png(32, 1, true), limits()).is_err());
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x00,
            0x00, 0xFF, 0xD9,
        ];
        assert_eq!(
            super::inspect_image(&jpeg, limits()).unwrap().format,
            "image/jpeg"
        );
        assert!(super::inspect_image(&jpeg[..jpeg.len() - 2], limits()).is_err());
        assert!(super::inspect_image(b"%PDF-1.7", limits()).is_err());
    }

    #[tokio::test]
    async fn before_save_hook_rejects_without_writing() {
        let root = std::env::temp_dir().join(format!("ferrioxid-image-{}", Uuid::new_v4()));
        let storage = LocalStorage::open(&root, 1024, &["image/png"])
            .unwrap()
            .with_check(std::sync::Arc::new(super::ImageHeaderCheck {
                limits: limits(),
            }));
        let owner = Uuid::new_v4();
        let rejected = storage
            .save(owner, "broken.png", b"\x89PNG\r\n\x1a\nnope")
            .await;
        assert!(matches!(rejected, Err(super::StorageError::InvalidImage)));
        let entries = std::fs::read_dir(&root).unwrap().count();
        assert_eq!(entries, 0);
        storage
            .save(owner, "ok.png", &png(1, 1, true))
            .await
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
