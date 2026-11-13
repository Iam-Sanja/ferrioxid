//! Dateien liegen unter `{root}/{owner}/{id}`. Der Client-Dateiname wird nie zum Pfad.

use crate::Storage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredFile {
    pub id: Uuid,
    pub owner: Uuid,
    pub original_name: String,
    pub content_type: String,
    pub size: u64,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed_partials: u64,
    pub removed_orphans: u64,
    pub removed_expired: u64,
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
    #[error("file access was denied")]
    Forbidden,
    #[error("storage quota was reached")]
    QuotaExceeded,
    #[error("file has expired")]
    Expired,
}

#[derive(Clone)]
pub struct LocalStorage {
    root: PathBuf,
    max_bytes: u64,
    allow: Vec<String>,
    check: Option<std::sync::Arc<dyn BeforeSave>>,
    max_files: u32,
    max_owner_bytes: u64,
    gate: std::sync::Arc<Mutex<()>>,
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
            max_files: 0,
            max_owner_bytes: 0,
            gate: std::sync::Arc::new(Mutex::new(())),
        })
    }

    /// `max_files` oder `max_owner_bytes` mit 0 hebt diese Grenze auf. Die Dateigrenze bleibt.
    pub fn with_quota(mut self, max_files: u32, max_owner_bytes: u64) -> Self {
        self.max_files = max_files;
        self.max_owner_bytes = max_owner_bytes;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Runs `check` before a file is created. A rejection leaves no file behind.
    pub fn with_check(mut self, check: std::sync::Arc<dyn BeforeSave>) -> Self {
        self.check = Some(check);
        self
    }

    /// Resolves `{root}/{owner}` and refuses a symlink or a path outside the root.
    fn rooted_owner(&self, owner: Uuid, create: bool) -> Result<PathBuf, StorageError> {
        let root = canonicalize_existing(&self.root)?;
        let directory = root.join(owner.to_string());
        match std::fs::symlink_metadata(&directory) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(if create {
                    StorageError::Failed
                } else {
                    StorageError::NotFound
                });
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(StorageError::Failed),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Err(StorageError::NotFound);
                }
                std::fs::create_dir(&directory).map_err(|_| StorageError::Failed)?;
            }
            Err(_) => return Err(StorageError::Failed),
        }
        if is_symlink(&directory) {
            return Err(if create {
                StorageError::Failed
            } else {
                StorageError::NotFound
            });
        }
        let canonical = canonicalize_existing(&directory)?;
        if canonical.starts_with(&root) {
            Ok(canonical)
        } else if create {
            Err(StorageError::Failed)
        } else {
            Err(StorageError::NotFound)
        }
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

    pub async fn save_until(
        &self,
        owner: Uuid,
        original_name: &str,
        bytes: &[u8],
        expires_at: Option<i64>,
    ) -> Result<StoredFile, StorageError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        if !extension_matches(original_name, content_type) {
            return Err(StorageError::TypeRejected);
        }
        let directory = self.rooted_owner(owner, true)?;
        self.enforce_quota(&directory, bytes.len() as u64)?;
        let id = Uuid::new_v4();
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
            created_at: unix_now(),
            expires_at,
            sha256: hex(Sha256::digest(bytes)),
        };
        let json = serde_json::to_vec(&stored).map_err(|_| StorageError::Failed)?;
        if write_new(&meta_path, &json).is_err() {
            let _ = std::fs::remove_file(&target);
            let _ = std::fs::remove_file(&meta_path);
            return Err(StorageError::Failed);
        }
        Ok(stored)
    }

    pub fn cleanup_orphans(&self, now: i64) -> Result<CleanupReport, StorageError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut report = CleanupReport::default();
        let root = std::fs::read_dir(&self.root).map_err(|_| StorageError::Failed)?;
        for owner in root.flatten() {
            let kind = match owner.file_type() {
                Ok(kind) => kind,
                Err(_) => continue,
            };
            // Directory links are not walked. Their targets stay outside this cleanup.
            if kind.is_symlink() || !kind.is_dir() {
                continue;
            }
            let directory = owner.path();
            let entries = std::fs::read_dir(&directory).map_err(|_| StorageError::Failed)?;
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if is_symlink(&entry.path()) {
                    if let Some(id) = managed_id(name) {
                        let _ = std::fs::remove_file(entry.path());
                        if !name.ends_with(".json") && !name.ends_with(".partial") {
                            let sidecar = directory.join(format!("{id}.json"));
                            if is_symlink(&sidecar) || regular_file(&sidecar) {
                                let _ = std::fs::remove_file(sidecar);
                            }
                        }
                        report.removed_orphans += 1;
                    }
                    continue;
                }
                if !regular_file(&entry.path()) {
                    continue;
                }
                if let Some(id) = name.strip_suffix(".partial") {
                    if Uuid::parse_str(id).is_ok() {
                        let _ = std::fs::remove_file(entry.path());
                        report.removed_partials += 1;
                    }
                    continue;
                }
                if let Some(id) = name.strip_suffix(".json") {
                    if Uuid::parse_str(id).is_err() {
                        continue;
                    }
                    let data = directory.join(id);
                    if !regular_file(&data) {
                        let _ = std::fs::remove_file(entry.path());
                        report.removed_orphans += 1;
                        continue;
                    }
                    if let Ok(meta) = read_meta(&entry.path()) {
                        if expired(&meta, now) {
                            let _ = std::fs::remove_file(&data);
                            let _ = std::fs::remove_file(entry.path());
                            report.removed_expired += 1;
                        }
                    }
                    continue;
                }
                if Uuid::parse_str(name).is_ok()
                    && !regular_file(&directory.join(format!("{name}.json")))
                {
                    let _ = std::fs::remove_file(entry.path());
                    report.removed_orphans += 1;
                }
            }
        }
        Ok(report)
    }

    pub fn purge_owner(&self, owner: Uuid) -> Result<u64, StorageError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = canonicalize_existing(&self.root)?;
        let directory = root.join(owner.to_string());
        let meta = match std::fs::symlink_metadata(&directory) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(StorageError::Failed),
        };
        if meta.file_type().is_symlink() {
            std::fs::remove_file(&directory)
                .or_else(|_| std::fs::remove_dir(&directory))
                .map_err(|_| StorageError::Failed)?;
            return Ok(1);
        }
        if !meta.is_dir() {
            return Err(StorageError::Failed);
        }
        let canonical = canonicalize_existing(&directory)?;
        if !canonical.starts_with(&root) {
            return Err(StorageError::Failed);
        }
        let mut removed = 0u64;
        let entries = std::fs::read_dir(&canonical).map_err(|_| StorageError::Failed)?;
        for entry in entries.flatten() {
            if regular_file(&entry.path()) {
                std::fs::remove_file(entry.path()).map_err(|_| StorageError::Failed)?;
                removed += 1;
            }
        }
        let _ = std::fs::remove_dir(&canonical);
        Ok(removed)
    }

    fn enforce_quota(&self, directory: &Path, incoming: u64) -> Result<(), StorageError> {
        if self.max_files == 0 && self.max_owner_bytes == 0 {
            return Ok(());
        }
        let mut files = 0u32;
        let mut bytes = 0u64;
        if directory.exists() {
            for entry in std::fs::read_dir(directory).map_err(|_| StorageError::Failed)? {
                let entry = entry.map_err(|_| StorageError::Failed)?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                let Some(id) = name.strip_suffix(".json") else {
                    continue;
                };
                if Uuid::parse_str(id).is_err()
                    || !regular_file(&entry.path())
                    || !regular_file(&directory.join(id))
                {
                    continue;
                }
                if let Ok(meta) = read_meta(&entry.path()) {
                    files += 1;
                    bytes = bytes.saturating_add(meta.size);
                }
            }
        }
        if self.max_files > 0 && files >= self.max_files {
            return Err(StorageError::QuotaExceeded);
        }
        if self.max_owner_bytes > 0 && bytes.saturating_add(incoming) > self.max_owner_bytes {
            return Err(StorageError::QuotaExceeded);
        }
        Ok(())
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
        self.save_until(owner, original_name, bytes, None).await
    }

    async fn read(&self, owner: Uuid, id: Uuid) -> Result<(StoredFile, Vec<u8>), StorageError> {
        let meta = self.metadata(owner, id).await?;
        if expired(&meta, unix_now()) {
            return Err(StorageError::Expired);
        }
        let directory = self.rooted_owner(owner, false)?;
        let path = directory.join(id.to_string());
        if !regular_file(&path) {
            return Err(StorageError::NotFound);
        }
        let path = Self::contained(&directory, &path)?;
        let bytes = std::fs::read(path).map_err(|_| StorageError::NotFound)?;
        Ok((meta, bytes))
    }

    async fn delete(&self, owner: Uuid, id: Uuid) -> Result<(), StorageError> {
        let directory = self.rooted_owner(owner, false)?;
        let file = directory.join(id.to_string());
        if !regular_file(&file) {
            return Err(StorageError::NotFound);
        }
        let file = Self::contained(&directory, &file)?;
        let meta = directory.join(format!("{id}.json"));
        std::fs::remove_file(file).map_err(|_| StorageError::NotFound)?;
        if regular_file(&meta) {
            if let Ok(meta) = Self::contained(&directory, &meta) {
                let _ = std::fs::remove_file(meta);
            }
        }
        Ok(())
    }

    async fn exists(&self, owner: Uuid, id: Uuid) -> Result<bool, StorageError> {
        match self.metadata(owner, id).await {
            Ok(meta) if !expired(&meta, unix_now()) => Ok(true),
            Ok(_) => Ok(false),
            Err(StorageError::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn metadata(&self, owner: Uuid, id: Uuid) -> Result<StoredFile, StorageError> {
        let directory = self.rooted_owner(owner, false)?;
        let path = directory.join(format!("{id}.json"));
        if !regular_file(&path) {
            return Err(StorageError::NotFound);
        }
        let path = Self::contained(&directory, &path)?;
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
    if cleaned.is_empty() || cleaned.chars().all(|char| char == '.') {
        "file".to_owned()
    } else {
        cleaned
    }
}

fn extension_matches(name: &str, content_type: &str) -> bool {
    let Some(extension) = std::path::Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
    else {
        return true;
    };
    let expected = match extension.to_ascii_lowercase().as_str() {
        "txt" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => return false,
    };
    expected == content_type
}

fn expired(file: &StoredFile, now: i64) -> bool {
    file.expires_at.is_some_and(|expires| expires <= now)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_file())
        .unwrap_or(false)
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn managed_id(name: &str) -> Option<&str> {
    let stem = name
        .strip_suffix(".partial")
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(name);
    Uuid::parse_str(stem).is_ok().then_some(stem)
}

fn read_meta(path: &Path) -> Result<StoredFile, StorageError> {
    let bytes = std::fs::read(path).map_err(|_| StorageError::NotFound)?;
    serde_json::from_slice(&bytes).map_err(|_| StorageError::Failed)
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

    #[tokio::test]
    async fn lifecycle_quota_names_and_reopen_stay_inside_the_root() {
        let root = std::env::temp_dir().join(format!("ferrioxid-life-{}", Uuid::new_v4()));
        let storage = LocalStorage::open(&root, 64, &["text/plain", "image/png", "image/jpeg"])
            .unwrap()
            .with_quota(1, 0);
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let stored = storage
            .save_until(owner, "..\\notes.txt", b"hello", Some(50))
            .await
            .unwrap();
        assert_eq!(stored.original_name, "notes.txt");
        assert!(!stored.sha256.is_empty());
        assert!(storage.save(owner, "second.txt", b"more").await.is_err());
        assert!(storage
            .save(owner, "photo.png", &[0xFF, 0xD8, 0xFF, b'x'])
            .await
            .is_err());
        assert_eq!(sanitize_name(".."), "file");
        assert_eq!(sanitize_name("a\0b.txt"), "ab.txt");
        assert!(storage.read(owner, stored.id).await.is_err());
        let outside = root
            .join("..")
            .join(format!("ferrioxid-outside-{}", Uuid::new_v4()));
        std::fs::write(&outside, b"secret").unwrap();
        assert!(LocalStorage::contained(&root, &outside).is_err());
        let _ = std::fs::remove_file(&outside);
        let directory = root.join(owner.to_string());
        std::fs::write(
            directory.join(format!("{}.partial", Uuid::new_v4())),
            b"partial",
        )
        .unwrap();
        std::fs::write(directory.join(format!("{}.json", Uuid::new_v4())), b"{}").unwrap();
        let report = storage.cleanup_orphans(60).unwrap();
        assert!(report.removed_partials >= 1);
        assert!(report.removed_orphans >= 1);
        assert!(report.removed_expired >= 1);
        assert!(storage.read(owner, stored.id).await.is_err());
        let kept = LocalStorage::open(&root, 64, &["text/plain"])
            .unwrap()
            .save(owner, "again.txt", b"kept")
            .await
            .unwrap();
        drop(storage);
        let reopened = LocalStorage::open(&root, 64, &["text/plain"]).unwrap();
        let (_, bytes) = reopened.read(owner, kept.id).await.unwrap();
        assert_eq!(bytes, b"kept");
        assert!(matches!(
            reopened.read_for(other, owner, kept.id).await,
            Err(super::StorageError::Forbidden)
        ));
        assert!(reopened.purge_owner(owner).unwrap() >= 2);
        let old = serde_json::from_str::<super::StoredFile>(
            &format!(
                "{{\"id\":\"{}\",\"owner\":\"{}\",\"original_name\":\"old.txt\",\"content_type\":\"text/plain\",\"size\":1}}",
                Uuid::nil(),
                owner
            ),
        )
        .unwrap();
        assert_eq!(old.sha256, "");
        assert!(old.expires_at.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn concurrent_uploads_keep_distinct_ids() {
        let root = std::env::temp_dir().join(format!("ferrioxid-race-{}", Uuid::new_v4()));
        let storage = std::sync::Arc::new(
            LocalStorage::open(&root, 64, &["text/plain"])
                .unwrap()
                .with_quota(8, 0),
        );
        let owner = Uuid::new_v4();
        let left = storage.clone();
        let right = storage.clone();
        let (first, second) = tokio::join!(
            left.save(owner, "a.txt", b"one"),
            right.save(owner, "b.txt", b"two"),
        );
        let first = first.expect("left");
        let second = second.expect("right");
        assert_ne!(first.id, second.id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn parallel_uploads_stop_at_the_quota() {
        let root = std::env::temp_dir().join(format!("ferrioxid-quota-{}", Uuid::new_v4()));
        let storage = std::sync::Arc::new(
            LocalStorage::open(&root, 64, &["text/plain"])
                .unwrap()
                .with_quota(1, 32),
        );
        let owner = Uuid::new_v4();
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let storage = storage.clone();
            tasks.push(tokio::spawn(async move {
                storage.save(owner, "notes.txt", b"hello").await
            }));
        }
        let mut stored = 0usize;
        for task in tasks {
            if task.await.expect("join").is_ok() {
                stored += 1;
            }
        }
        assert_eq!(stored, 1);
        let data_files = std::fs::read_dir(root.join(owner.to_string()))
            .expect("owner")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| Uuid::parse_str(name).is_ok())
            })
            .count();
        assert_eq!(data_files, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn traversal_mime_partials_and_symlinks_do_not_escape() {
        let root = std::env::temp_dir().join(format!("ferrioxid-escape-{}", Uuid::new_v4()));
        let storage = LocalStorage::open(&root, 64, &["text/plain"]).unwrap();
        let owner = Uuid::new_v4();
        let stored = storage
            .save(owner, r"C:\Windows\system32\notes.txt", b"hello")
            .await
            .unwrap();
        assert_eq!(stored.original_name, "notes.txt");
        assert!(root
            .join(owner.to_string())
            .join(stored.id.to_string())
            .is_file());
        assert!(!root.join("Windows").exists());
        assert!(storage.save(owner, "notes.png", b"hello").await.is_err());
        assert!(storage.save(owner, "notes.exe", b"hello").await.is_err());
        let orphan = Uuid::new_v4();
        std::fs::write(
            root.join(owner.to_string()).join(orphan.to_string()),
            b"cut",
        )
        .unwrap();
        let report = storage.cleanup_orphans(0).unwrap();
        assert!(report.removed_orphans >= 1);
        assert!(!root
            .join(owner.to_string())
            .join(orphan.to_string())
            .exists());
        let (_, bytes) = storage.read(owner, stored.id).await.unwrap();
        assert_eq!(bytes, b"hello");

        let outside =
            std::env::temp_dir().join(format!("ferrioxid-link-target-{}", Uuid::new_v4()));
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
        let linked_owner = Uuid::new_v4();
        link_dir(&outside, &root.join(linked_owner.to_string()));
        assert!(storage
            .save(linked_owner, "notes.txt", b"leaked")
            .await
            .is_err());
        assert_eq!(
            std::fs::read(outside.join("secret.txt")).unwrap(),
            b"secret"
        );
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);

        let data = root.join(owner.to_string()).join(stored.id.to_string());
        std::fs::remove_file(&data).unwrap();
        link_file(&outside.join("secret.txt"), &data);
        assert!(storage.read(owner, stored.id).await.is_err());
        storage.cleanup_orphans(0).unwrap();
        assert_eq!(
            std::fs::read(outside.join("secret.txt")).unwrap(),
            b"secret"
        );
        assert_eq!(storage.purge_owner(linked_owner).unwrap(), 1);
        assert!(outside.join("secret.txt").is_file());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    fn link_dir(target: &std::path::Path, link: &std::path::Path) {
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target, link).expect("directory symlink");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).expect("directory symlink");
    }

    fn link_file(target: &std::path::Path, link: &std::path::Path) {
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(target, link).expect("file symlink");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).expect("file symlink");
    }
}
