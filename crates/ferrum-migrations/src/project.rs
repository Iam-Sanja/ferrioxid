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
    pub name: Option<String>,
    pub template: Option<String>,
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub admin_enabled: bool,
    pub embedded_assets: bool,
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
    let mut section = String::new();
    let mut migrations_dir = None;
    let mut schema_command = None;
    let mut name = None;
    let mut template = None;
    let mut database_url = ferrum_db::DEFAULT_SQLITE_URL.to_owned();
    let mut host = "127.0.0.1".to_owned();
    let mut port = 3000_u16;
    let mut admin_enabled = false;
    let mut embedded_assets = false;
    for (line_number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            if !matches!(
                section.as_str(),
                "project" | "database" | "server" | "admin"
            ) {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: unbekannter Abschnitt `[{section}]`.",
                    path.display()
                )));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(MigrationError::new(format!(
                "{}:{line_number}: erwartet wird schluessel = \"wert\".",
                path.display()
            )));
        };
        let key = key.trim();
        let value = parse_value(value.trim()).ok_or_else(|| {
            MigrationError::new(format!(
                "{}:{line_number}: der Wert von `{key}` ist ungültig.",
                path.display()
            ))
        })?;
        let qualified = if section.is_empty() {
            key.to_owned()
        } else {
            format!("{section}.{key}")
        };
        match qualified.as_str() {
            "migrations_dir" | "project.migrations_dir" => {
                migrations_dir = Some(root.join(value));
            }
            "schema_command" | "project.schema_command" => schema_command = Some(value),
            "project.name" => name = Some(value),
            "project.template" => template = Some(value),
            "database.url" => database_url = value,
            "server.host" => host = value,
            "server.port" => {
                port = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: server.port muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "admin.enabled" => admin_enabled = parse_bool(&value)?,
            "admin.embedded_assets" => embedded_assets = parse_bool(&value)?,
            _ => {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: unbekannter Schlüssel `{qualified}`.",
                    path.display()
                )))
            }
        }
    }
    let migrations_dir = migrations_dir.ok_or_else(|| {
        MigrationError::new(format!("{} enthält kein migrations_dir.", path.display()))
    })?;
    if let Some(template) = &template {
        if template != "api" && template != "fullstack" {
            return Err(MigrationError::new(format!(
                "project.template `{template}` ist unbekannt. Erlaubt sind api und fullstack."
            )));
        }
    }
    Ok(ProjectConfig {
        root,
        migrations_dir,
        schema_command,
        name,
        template,
        database_url,
        host,
        port,
        admin_enabled,
        embedded_assets,
    })
}

fn parse_value(value: &str) -> Option<String> {
    if let Some(quoted) = unquote(value) {
        return Some(quoted.to_owned());
    }
    if value == "true" || value == "false" || value.chars().all(|char| char.is_ascii_digit()) {
        return Some(value.to_owned());
    }
    None
}

fn parse_bool(value: &str) -> Result<bool, MigrationError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MigrationError::new(format!(
            "`{value}` ist kein Wahrheitswert. Erlaubt sind true und false."
        ))),
    }
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
