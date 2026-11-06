//! Migrationsdateien, Checksums und Verzeichnisprüfung.

use crate::diff::{plan_change, MigrationOperation, MigrationPlan, MigrationRisk};
use crate::error::MigrationError;
use crate::snapshot::{is_identifier, SchemaSnapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationFile {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub from_snapshot: SchemaSnapshot,
    pub to_snapshot: SchemaSnapshot,
    pub operations: Vec<MigrationOperation>,
    pub risks: Vec<MigrationRisk>,
    pub checksum: String,
}

#[derive(Serialize)]
struct ChecksumBody<'a> {
    format_version: u32,
    id: &'a str,
    name: &'a str,
    from_snapshot: &'a SchemaSnapshot,
    to_snapshot: &'a SchemaSnapshot,
    operations: &'a [MigrationOperation],
    risks: &'a [MigrationRisk],
}

#[derive(Debug)]
pub struct DirectoryState {
    pub files: Vec<MigrationFile>,
    pub conflicts: Vec<String>,
}

#[derive(Debug)]
pub enum MakeOutcome {
    Unchanged,
    Written {
        path: PathBuf,
        id: String,
        risks: Vec<MigrationRisk>,
    },
}

pub fn checksum_of(file: &MigrationFile) -> Result<String, MigrationError> {
    let body = ChecksumBody {
        format_version: file.format_version,
        id: &file.id,
        name: &file.name,
        from_snapshot: &file.from_snapshot,
        to_snapshot: &file.to_snapshot,
        operations: &file.operations,
        risks: &file.risks,
    };
    let canonical = serde_json::to_string(&body).map_err(|error| {
        MigrationError::new(format!(
            "Checksum-Grundlage ist nicht serialisierbar: {error}"
        ))
    })?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn seal(
    id: String,
    name: String,
    from_snapshot: SchemaSnapshot,
    to_snapshot: SchemaSnapshot,
    plan: &MigrationPlan,
) -> Result<MigrationFile, MigrationError> {
    validate_migration_name(&id, &name)?;
    from_snapshot.validate()?;
    to_snapshot.validate()?;
    let mut file = MigrationFile {
        format_version: crate::snapshot::FORMAT_VERSION,
        id,
        name,
        from_snapshot,
        to_snapshot,
        operations: plan.operations.clone(),
        risks: plan.risks.clone(),
        checksum: String::new(),
    };
    file.checksum = checksum_of(&file)?;
    Ok(file)
}

pub fn save_migration(directory: &Path, file: &MigrationFile) -> Result<PathBuf, MigrationError> {
    fs::create_dir_all(directory).map_err(|error| {
        MigrationError::new(format!(
            "Migrationsverzeichnis {} konnte nicht angelegt werden: {error}",
            directory.display()
        ))
    })?;
    let path = directory.join(format!("{}.json", file.id));
    if path.exists() {
        return Err(MigrationError::new(format!(
            "Migrationsdatei {} existiert bereits und wird nicht überschrieben.",
            path.display()
        )));
    }
    let text = serde_json::to_string_pretty(file).map_err(|error| {
        MigrationError::new(format!(
            "Migration konnte nicht serialisiert werden: {error}"
        ))
    })?;
    fs::write(&path, format!("{text}\n")).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} konnte nicht geschrieben werden: {error}",
            path.display()
        ))
    })?;
    Ok(path)
}

pub fn inspect_directory(directory: &Path) -> DirectoryState {
    let mut state = DirectoryState {
        files: Vec::new(),
        conflicts: Vec::new(),
    };
    if !directory.exists() {
        return state;
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            state.conflicts.push(format!(
                "Migrationsverzeichnis {} ist nicht lesbar: {error}",
                directory.display()
            ));
            return state;
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    for path in paths {
        match load_migration(&path) {
            Ok(file) => state.files.push(file),
            Err(error) => state.conflicts.push(error.0),
        }
    }
    state
        .files
        .sort_by_key(|file| migration_number(&file.id).unwrap_or(0));
    if let Err(error) = validate_chain(&state.files) {
        state.conflicts.push(error.0);
    }
    state
}

pub fn load_migration(path: &Path) -> Result<MigrationFile, MigrationError> {
    let text = fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} ist nicht lesbar: {error}",
            path.display()
        ))
    })?;
    let file: MigrationFile = serde_json::from_str(&text).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} ist kein gültiges JSON: {error}",
            path.display()
        ))
    })?;
    if file.format_version != crate::snapshot::FORMAT_VERSION {
        return Err(MigrationError::new(format!(
            "Migration {} hat die unbekannte Formatversion {}.",
            file.id, file.format_version
        )));
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    if stem != file.id {
        return Err(MigrationError::new(format!(
            "Dateiname `{stem}` und Migrations-ID `{}` stimmen nicht überein.",
            file.id
        )));
    }
    file.from_snapshot.validate().map_err(|error| {
        MigrationError::new(format!(
            "Ausgangsschema von {} ist ungültig: {}",
            file.id, error.0
        ))
    })?;
    file.to_snapshot.validate().map_err(|error| {
        MigrationError::new(format!(
            "Zielschema von {} ist ungültig: {}",
            file.id, error.0
        ))
    })?;
    validate_migration_name(&file.id, &file.name)?;
    let expected = checksum_of(&file)?;
    if expected != file.checksum {
        return Err(MigrationError::new(format!(
            "Checksum der Migration {} stimmt nicht. Die Datei wurde verändert oder beschädigt.",
            file.id
        )));
    }
    let produced = crate::simulate::apply_operations(&file.from_snapshot, &file.operations)
        .map_err(|error| {
            MigrationError::new(format!(
                "Operationen der Migration {} passen nicht zum Ausgangsschema: {}",
                file.id, error.0
            ))
        })?;
    if !produced.semantically_eq(&file.to_snapshot) {
        return Err(MigrationError::new(format!(
            "Operationen der Migration {} erzeugen nicht das gespeicherte Zielschema.",
            file.id
        )));
    }
    Ok(file)
}

pub fn make_migration(
    directory: &Path,
    name: &str,
    current: &SchemaSnapshot,
) -> Result<MakeOutcome, MigrationError> {
    current.validate()?;
    if !is_migration_slug(name) {
        return Err(MigrationError::new(
            "Der Migrationsname muss mit einem Buchstaben beginnen und darf nur Kleinbuchstaben, Ziffern und Unterstriche enthalten.",
        ));
    }
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    let from = state
        .files
        .last()
        .map(|file| file.to_snapshot.clone())
        .unwrap_or_else(SchemaSnapshot::empty);
    let plan = plan_change(&from, current)?;
    if plan.operations.is_empty() {
        return Ok(MakeOutcome::Unchanged);
    }
    if let Some(reason) = plan.refusal {
        return Err(MigrationError::new(format!(
            "Es wurde keine Migration geschrieben.\n{reason}"
        )));
    }
    let number = state
        .files
        .last()
        .map(|file| migration_number(&file.id).unwrap_or(0) + 1)
        .unwrap_or(1);
    if number > 9999 {
        return Err(MigrationError::new(
            "Es sind höchstens 9999 nummerierte Migrationen vorgesehen.",
        ));
    }
    let id = format!("{number:04}_{name}");
    let file = seal(id, name.to_owned(), from, current.clone(), &plan)?;
    let path = save_migration(directory, &file)?;
    Ok(MakeOutcome::Written {
        path,
        id: file.id,
        risks: file.risks,
    })
}

pub fn validate_chain(files: &[MigrationFile]) -> Result<(), MigrationError> {
    let mut previous_number = 0;
    let mut previous_target: Option<&SchemaSnapshot> = None;
    for file in files {
        let number = migration_number(&file.id).ok_or_else(|| {
            MigrationError::new(format!(
                "Migrations-ID `{}` hat kein vierstelliges Präfix.",
                file.id
            ))
        })?;
        if number <= previous_number {
            return Err(MigrationError::new(format!(
                "Migrationsreihenfolge ist nicht eindeutig bei `{}`.",
                file.id
            )));
        }
        match previous_target {
            None => {
                if !file.from_snapshot.semantically_eq(&SchemaSnapshot::empty()) {
                    return Err(MigrationError::new(format!(
                        "Die erste Migration {} muss von einem leeren Schema ausgehen.",
                        file.id
                    )));
                }
            }
            Some(target) => {
                if file.from_snapshot.to_canonical_json()? != target.to_canonical_json()? {
                    return Err(MigrationError::new(format!(
                        "Migration {} setzt nicht auf dem Zielschema der vorherigen Migration auf.",
                        file.id
                    )));
                }
            }
        }
        previous_number = number;
        previous_target = Some(&file.to_snapshot);
    }
    Ok(())
}

fn validate_migration_name(id: &str, name: &str) -> Result<(), MigrationError> {
    let number = migration_number(id).ok_or_else(|| {
        MigrationError::new(format!(
            "Migrations-ID `{id}` muss die Form 0001_name haben."
        ))
    })?;
    let expected = format!("{number:04}_{name}");
    if id != expected || !is_migration_slug(name) {
        return Err(MigrationError::new(format!(
            "Migrations-ID `{id}` passt nicht zum Namen `{name}`."
        )));
    }
    if !is_identifier(name) {
        return Err(MigrationError::new(format!(
            "Ungültiger Migrationsname `{name}`."
        )));
    }
    Ok(())
}

fn is_migration_slug(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

pub fn write_explicit_migration(
    directory: &Path,
    name: &str,
    current: &SchemaSnapshot,
    operation: MigrationOperation,
) -> Result<MakeOutcome, MigrationError> {
    if !matches!(
        operation,
        MigrationOperation::RenameColumn { .. } | MigrationOperation::RenameTable { .. }
    ) {
        return Err(MigrationError::new(
            "Eine explizite Migrationsdatei akzeptiert nur RenameColumn oder RenameTable."
                .to_owned(),
        ));
    }
    current.validate()?;
    if !is_migration_slug(name) {
        return Err(MigrationError::new(
            "Der Migrationsname muss mit einem Buchstaben beginnen und darf nur Kleinbuchstaben, Ziffern und Unterstriche enthalten.".to_owned(),
        ));
    }
    let state = inspect_directory(directory);
    if !state.conflicts.is_empty() {
        return Err(MigrationError::new(state.conflicts.join("\n")));
    }
    let from = state
        .files
        .last()
        .map(|file| file.to_snapshot.clone())
        .unwrap_or_else(SchemaSnapshot::empty);
    let produced = crate::simulate::apply_operations(&from, std::slice::from_ref(&operation))?;
    if !produced.semantically_eq(current) {
        return Err(MigrationError::new(
            "Die Umbenennung erklärt nicht die gesamte Abweichung zwischen dem letzten Migrationsziel und dem registrierten Schema. Es wurde keine Datei geschrieben.".to_owned(),
        ));
    }
    let number = state
        .files
        .last()
        .map(|file| migration_number(&file.id).unwrap_or(0) + 1)
        .unwrap_or(1);
    if number > 9999 {
        return Err(MigrationError::new(
            "Es sind höchstens 9999 nummerierte Migrationen vorgesehen.".to_owned(),
        ));
    }
    let id = format!("{number:04}_{name}");
    let plan = MigrationPlan {
        operations: vec![operation],
        risks: Vec::new(),
        refusal: None,
    };
    let file = seal(id, name.to_owned(), from, current.clone(), &plan)?;
    let path = save_migration(directory, &file)?;
    Ok(MakeOutcome::Written {
        path,
        id: file.id,
        risks: file.risks,
    })
}

pub fn reseal_migration(path: &Path) -> Result<MigrationFile, MigrationError> {
    let text = fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} ist nicht lesbar: {error}",
            path.display()
        ))
    })?;
    let mut file: MigrationFile = serde_json::from_str(&text).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} ist kein gültiges JSON: {error}",
            path.display()
        ))
    })?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    if stem != file.id {
        return Err(MigrationError::new(format!(
            "Dateiname `{stem}` und Migrations-ID `{}` stimmen nicht überein.",
            file.id
        )));
    }
    file.from_snapshot.validate()?;
    file.to_snapshot.validate()?;
    validate_migration_name(&file.id, &file.name)?;
    let produced = crate::simulate::apply_operations(&file.from_snapshot, &file.operations)?;
    if !produced.semantically_eq(&file.to_snapshot) {
        return Err(MigrationError::new(format!(
            "Migration {} wird nicht neu versiegelt, weil die Operationen das Zielschema nicht erzeugen.",
            file.id
        )));
    }
    file.checksum = checksum_of(&file)?;
    let text = serde_json::to_string_pretty(&file).map_err(|error| {
        MigrationError::new(format!(
            "Migration konnte nicht serialisiert werden: {error}"
        ))
    })?;
    fs::write(path, format!("{text}\n")).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} konnte nicht geschrieben werden: {error}",
            path.display()
        ))
    })?;
    Ok(file)
}

pub fn migration_number(id: &str) -> Option<u32> {
    let (prefix, suffix) = id.split_once('_')?;
    if prefix.len() != 4 || suffix.is_empty() {
        return None;
    }
    prefix.parse().ok()
}
