//! Explizite Projektintegration. Die CLI liest keine Rust-Quellen ein.

use crate::error::MigrationError;
use crate::snapshot::SchemaSnapshot;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub migrations_dir: PathBuf,
    pub schema_command: Option<String>,
}

pub fn discover_project(start: &Path) -> Result<ProjectConfig, MigrationError> {
    let mut current = if start.is_dir() {
        Some(start)
    } else {
        start.parent()
    };
    while let Some(directory) = current {
        let manifest = directory.join("ferrum.toml");
        if manifest.is_file() {
            return parse_manifest(&manifest);
        }
        current = directory.parent();
    }
    Err(MigrationError::new(
        "Keine ferrum.toml gefunden. Die CLI erkennt Modelle nicht aus dem Quelltext. Lege eine ferrum.toml mit migrations_dir und schema_command an oder übergib --schema-file und --migrations-dir.",
    ))
}

pub fn load_schema_from_command(
    root: &Path,
    command: &str,
) -> Result<SchemaSnapshot, MigrationError> {
    let output = Command::new(command_program())
        .args(command_args(command))
        .current_dir(root)
        .output()
        .map_err(|error| {
            MigrationError::new(format!(
                "Schema-Befehl konnte nicht gestartet werden: {error}"
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MigrationError::new(format!(
            "Schema-Befehl ist fehlgeschlagen.\n{stderr}"
        )));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_| {
        MigrationError::new("Die Modell-Registry hat kein UTF-8-Schema geliefert.".to_owned())
    })?;
    SchemaSnapshot::from_json(stdout.trim())
}

fn parse_manifest(path: &Path) -> Result<ProjectConfig, MigrationError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "{} konnte nicht gelesen werden: {error}",
            path.display()
        ))
    })?;
    let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut migrations_dir = None;
    let mut schema_command = None;
    for (line_number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(MigrationError::new(format!(
                "{}:{line_number}: erwartet wird schluessel = \"wert\".",
                path.display()
            )));
        };
        let key = key.trim();
        let value = unquote(value.trim()).ok_or_else(|| {
            MigrationError::new(format!(
                "{}:{line_number}: der Wert von `{key}` muss in doppelten Anführungszeichen stehen.",
                path.display()
            ))
        })?;
        match key {
            "migrations_dir" => migrations_dir = Some(root.join(value)),
            "schema_command" => schema_command = Some(value.to_owned()),
            _ => {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: unbekannter Schlüssel `{key}`.",
                    path.display()
                )))
            }
        }
    }
    let migrations_dir = migrations_dir.ok_or_else(|| {
        MigrationError::new(format!("{} enthält kein migrations_dir.", path.display()))
    })?;
    Ok(ProjectConfig {
        root,
        migrations_dir,
        schema_command,
    })
}

fn unquote(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn command_program() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "sh"
    }
}

fn command_args(command: &str) -> Vec<String> {
    if cfg!(windows) {
        vec!["/C".to_owned(), command.to_owned()]
    } else {
        vec!["-c".to_owned(), command.to_owned()]
    }
}
