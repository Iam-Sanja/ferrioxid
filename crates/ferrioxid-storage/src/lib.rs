//! Lokaler Dateispeicher. Öffentliche Auslieferung bleibt aus. Ein späteres S3-Backend kann `Storage` implementieren.
//!
//! Malware-Scanning ist nicht enthalten. Die Inhaltsprüfung ersetzt keinen Virenscanner.

mod http;
mod local;

pub use http::{
    multipart_text_field, read_multipart, routes, FormPart, MultipartError, MULTIPART_BODY_LIMIT,
};
pub use local::{
    detect_type, inspect_image, BeforeSave, ImageHeaderCheck, ImageInfo, ImageLimits, LocalStorage,
    StorageError, StoredFile,
};

use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save(
        &self,
        owner: Uuid,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<StoredFile, StorageError>;
    async fn read(&self, owner: Uuid, id: Uuid) -> Result<(StoredFile, Vec<u8>), StorageError>;
    async fn delete(&self, owner: Uuid, id: Uuid) -> Result<(), StorageError>;
    async fn exists(&self, owner: Uuid, id: Uuid) -> Result<bool, StorageError>;
    async fn metadata(&self, owner: Uuid, id: Uuid) -> Result<StoredFile, StorageError>;
}
