//! Migration files, checksums, and directory validation.

use crate::diff::{
    plan_change_with_backfills, ColumnBackfill, MigrationOperation, MigrationPlan, MigrationRisk,
};
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

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
        MigrationError::new(format!("Could not serialize checksum input: {error}"))
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
            "Could not create migrations directory {}: {error}",
            directory.display()
        ))
    })?;
    let path = directory.join(format!("{}.json", file.id));
    if path.exists() {
        return Err(MigrationError::new(format!(
            "Migration file {} already exists and will not be overwritten.",
            path.display()
        )));
    }
    let text = serde_json::to_string_pretty(file)
        .map_err(|error| MigrationError::new(format!("Could not serialize migration: {error}")))?;
    fs::write(&path, format!("{text}\n")).map_err(|error| {
        MigrationError::new(format!(
            "Could not write migration {}: {error}",
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
                "Could not read migrations directory {}: {error}",
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
    let mut seen_numbers = std::collections::BTreeMap::<u32, (PathBuf, String, String)>::new();
    for path in paths {
        match load_migration(&path) {
            Ok(file) => {
                if let Some(number) = migration_number(&file.id) {
                    let operations = operation_summary(&file);
                    if let Some((other_path, other_id, other_operations)) =
                        seen_numbers.get(&number)
                    {
                        state.conflicts.push(format!(
                            "App migration number {number:04} is used by `{other_id}` ({}) operations [{other_operations}] and `{}` ({}) operations [{operations}]. Do not delete or merge the files. Renumber the migration that has not been applied.",
                            other_path.display(),
                            file.id,
                            path.display()
                        ));
                    } else {
                        seen_numbers.insert(number, (path.clone(), file.id.clone(), operations));
                    }
                }
                state.files.push(file);
            }
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
            "Could not read migration {}: {error}",
            path.display()
        ))
    })?;
    let file: MigrationFile = serde_json::from_str(&text).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} is not valid JSON: {error}",
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
            "Filename `{stem}` does not match migration ID `{}`.",
            file.id
        )));
    }
    file.from_snapshot.validate().map_err(|error| {
        MigrationError::new(format!(
            "Original schema of {} is invalid: {}",
            file.id, error.0
        ))
    })?;
    file.to_snapshot.validate().map_err(|error| {
        MigrationError::new(format!(
            "Target schema of {} is invalid: {}",
            file.id, error.0
        ))
    })?;
    validate_migration_name(&file.id, &file.name)?;
    let expected = checksum_of(&file)?;
    if expected != file.checksum {
        return Err(MigrationError::new(format!(
            "Checksum mismatch for migration {}. The file was changed or corrupted.",
            file.id
        )));
    }
    let produced = crate::simulate::apply_operations(&file.from_snapshot, &file.operations)
        .map_err(|error| {
            MigrationError::new(format!(
                "Operations of migration {} do not match the original schema: {}",
                file.id, error.0
            ))
        })?;
    if !produced.semantically_eq(&file.to_snapshot) {
        return Err(MigrationError::new(format!(
            "Operations of migration {} do not reproduce the stored target schema.",
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
    make_migration_with_backfills(directory, name, current, &[])
}

pub fn make_migration_with_backfills(
    directory: &Path,
    name: &str,
    current: &SchemaSnapshot,
    backfills: &[ColumnBackfill],
) -> Result<MakeOutcome, MigrationError> {
    current.validate()?;
    if !is_migration_slug(name) {
        return Err(MigrationError::new(
            "A migration name must start with a letter and contain only lowercase letters, digits, and underscores.",
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
    let plan = plan_change_with_backfills(&from, current, backfills)?;
    if plan.operations.is_empty() {
        return Ok(MakeOutcome::Unchanged);
    }
    if let Some(reason) = plan.refusal {
        return Err(MigrationError::new(format!(
            "No migration was written.\n{reason}"
        )));
    }
    let number = state
        .files
        .last()
        .map(|file| migration_number(&file.id).unwrap_or(0) + 1)
        .unwrap_or(1);
    if number > 9999 {
        return Err(MigrationError::new(
            "At most 9999 numbered migrations are supported.",
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
                "Migration ID `{}` does not have a four-digit prefix.",
                file.id
            ))
        })?;
        if number <= previous_number {
            return Err(MigrationError::new(format!(
                "Migration ordering is ambiguous at `{}`.",
                file.id
            )));
        }
        match previous_target {
            None => {
                if !file.from_snapshot.semantically_eq(&SchemaSnapshot::empty()) {
                    return Err(MigrationError::new(format!(
                        "First migration {} must start from an empty schema.",
                        file.id
                    )));
                }
            }
            Some(target) => {
                if file.from_snapshot.to_canonical_json()? != target.to_canonical_json()? {
                    return Err(MigrationError::new(format!(
                        "Migration {} does not use the previous migration's target schema.",
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
    let number = migration_number(id)
        .ok_or_else(|| MigrationError::new(format!("Migration ID `{id}` must match 0001_name.")))?;
    let expected = format!("{number:04}_{name}");
    if id != expected || !is_migration_slug(name) {
        return Err(MigrationError::new(format!(
            "Migration ID `{id}` does not match name `{name}`."
        )));
    }
    if !is_identifier(name) {
        return Err(MigrationError::new(format!(
            "Invalid migration name `{name}`."
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
            "A migration name must start with a letter and contain only lowercase letters, digits, and underscores.".to_owned(),
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
            "The rename does not account for all differences between the last migration target and registered schema. No file was written.".to_owned(),
        ));
    }
    let number = state
        .files
        .last()
        .map(|file| migration_number(&file.id).unwrap_or(0) + 1)
        .unwrap_or(1);
    if number > 9999 {
        return Err(MigrationError::new(
            "At most 9999 numbered migrations are supported.".to_owned(),
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
            "Could not read migration {}: {error}",
            path.display()
        ))
    })?;
    let mut file: MigrationFile = serde_json::from_str(&text).map_err(|error| {
        MigrationError::new(format!(
            "Migration {} is not valid JSON: {error}",
            path.display()
        ))
    })?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    if stem != file.id {
        return Err(MigrationError::new(format!(
            "Filename `{stem}` does not match migration ID `{}`.",
            file.id
        )));
    }
    file.from_snapshot.validate()?;
    file.to_snapshot.validate()?;
    validate_migration_name(&file.id, &file.name)?;
    let produced = crate::simulate::apply_operations(&file.from_snapshot, &file.operations)?;
    if !produced.semantically_eq(&file.to_snapshot) {
        return Err(MigrationError::new(format!(
            "Migration {} cannot be resealed because its operations do not produce the target schema.",
            file.id
        )));
    }
    file.checksum = checksum_of(&file)?;
    let text = serde_json::to_string_pretty(&file)
        .map_err(|error| MigrationError::new(format!("Could not serialize migration: {error}")))?;
    fs::write(path, format!("{text}\n")).map_err(|error| {
        MigrationError::new(format!(
            "Could not write migration {}: {error}",
            path.display()
        ))
    })?;
    Ok(file)
}

fn operation_summary(file: &MigrationFile) -> String {
    if file.operations.is_empty() {
        return "none".to_owned();
    }
    file.operations
        .iter()
        .map(operation_label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn operation_label(operation: &MigrationOperation) -> &'static str {
    match operation {
        MigrationOperation::CreateTable { .. } => "create_table",
        MigrationOperation::DropTable { .. } => "drop_table",
        MigrationOperation::AddColumn { .. } => "add_column",
        MigrationOperation::DropColumn { .. } => "drop_column",
        MigrationOperation::AlterColumn { .. } => "alter_column",
        MigrationOperation::RenameColumn { .. } => "rename_column",
        MigrationOperation::RenameTable { .. } => "rename_table",
        MigrationOperation::AddUniqueConstraint { .. } => "add_unique_constraint",
        MigrationOperation::DropUniqueConstraint { .. } => "drop_unique_constraint",
        MigrationOperation::AddIndex { .. } => "add_index",
        MigrationOperation::DropIndex { .. } => "drop_index",
        MigrationOperation::AddForeignKey { .. } => "add_foreign_key",
        MigrationOperation::DropForeignKey { .. } => "drop_foreign_key",
    }
}

pub fn migration_number(id: &str) -> Option<u32> {
    let (prefix, suffix) = id.split_once('_')?;
    if prefix.len() != 4 || suffix.is_empty() {
        return None;
    }
    prefix.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{inspect_directory, save_migration, seal};
    use crate::diff::MigrationPlan;
    use crate::snapshot::SchemaSnapshot;

    #[test]
    fn duplicate_numbers_name_both_paths_and_keep_the_files() {
        let directory = std::env::temp_dir().join(format!(
            "ferrioxid-mig-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let plan = MigrationPlan {
            operations: Vec::new(),
            risks: Vec::new(),
            refusal: None,
        };
        let empty = SchemaSnapshot::empty();
        let alpha = seal(
            "0001_alpha".into(),
            "alpha".into(),
            empty.clone(),
            empty.clone(),
            &plan,
        )
        .unwrap();
        let beta = seal(
            "0001_beta".into(),
            "beta".into(),
            empty.clone(),
            empty,
            &plan,
        )
        .unwrap();
        let alpha_path = save_migration(&directory, &alpha).unwrap();
        let beta_path = save_migration(&directory, &beta).unwrap();
        let state = inspect_directory(&directory);
        let message = state.conflicts.join("\n");
        assert!(message.contains("0001_alpha"), "{message}");
        assert!(message.contains("0001_beta"), "{message}");
        assert!(
            message.contains(&alpha_path.display().to_string()),
            "{message}"
        );
        assert!(
            message.contains(&beta_path.display().to_string()),
            "{message}"
        );
        assert!(message.contains("Do not delete"), "{message}");
        assert!(alpha_path.is_file());
        assert!(beta_path.is_file());
        let _ = std::fs::remove_dir_all(directory);
    }
}
