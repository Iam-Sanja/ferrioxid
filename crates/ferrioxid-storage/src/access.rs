//! Wer eine Datei schreiben oder lesen darf. Der Besitzer ist nicht automatisch eine andere Person.

use crate::StorageError;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageAction {
    Upload,
    Download,
    Delete,
}

pub trait StorageAccess: Send + Sync {
    fn permit(&self, actor: Uuid, owner: Uuid, action: StorageAction) -> Result<(), StorageError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct OwnerAccess;

impl StorageAccess for OwnerAccess {
    fn permit(&self, actor: Uuid, owner: Uuid, _action: StorageAction) -> Result<(), StorageError> {
        if actor == owner {
            Ok(())
        } else {
            Err(StorageError::Forbidden)
        }
    }
}
