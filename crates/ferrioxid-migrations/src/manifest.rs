//! Upgrade manifest. Cargo remains at version 0.1.0; the template version is stored here.

use crate::error::MigrationError;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MANIFEST_FORMAT: u32 = 1;
pub const TEMPLATE_VERSION: &str = "0.24";
pub const MANIFEST_RELATIVE: &str = ".ferrioxid/project.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub format: u32,
    pub template_version: String,
    pub origin: String,
    pub template: Option<String>,
    pub files: Vec<TrackedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedFile {
    pub path: String,
    pub sha256: String,
}

impl ProjectManifest {
    pub fn generated(template: &str, files: Vec<TrackedFile>) -> Self {
        Self {
            format: MANIFEST_FORMAT,
            template_version: TEMPLATE_VERSION.to_owned(),
            origin: "generated".to_owned(),
            template: Some(template.to_owned()),
            files,
        }
    }

    pub fn legacy() -> Self {
        Self {
            format: MANIFEST_FORMAT,
            template_version: "legacy".to_owned(),
            origin: "legacy".to_owned(),
            template: None,
            files: Vec::new(),
        }
    }
}

pub fn load_manifest(path: &Path) -> Result<ProjectManifest, MigrationError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "project.file.missing: Could not read {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        MigrationError::new(format!(
            "upgrade.conflict: The manifest is not a valid project manifest: {error}"
        ))
    })
}

pub fn manifest_json(manifest: &ProjectManifest) -> Result<String, MigrationError> {
    serde_json::to_string_pretty(manifest)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| MigrationError::new(format!("Could not serialize manifest: {error}")))
}
