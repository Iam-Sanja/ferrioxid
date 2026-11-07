//! Reihenfolge von App-Migrationen. Die Anwendung selbst startet keine Migration.

use crate::error::MigrationError;
use crate::runner::{apply_namespaced_directory, namespaced_status};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMigration {
    pub name: String,
    pub directory: PathBuf,
    pub depends_on: Vec<String>,
}

pub fn same_migration_contract(
    registered: &[PackageMigration],
    declared: &[PackageMigration],
    root: &std::path::Path,
) -> Result<(), MigrationError> {
    let mut registered_names = BTreeMap::new();
    for package in registered {
        if registered_names
            .insert(package.name.clone(), package)
            .is_some()
        {
            return Err(MigrationError::new(format!(
                "Die App `{}` ist in der Rust-Registrierung doppelt.",
                package.name
            )));
        }
    }
    let mut declared_names = BTreeMap::new();
    for package in declared {
        if declared_names
            .insert(package.name.clone(), package)
            .is_some()
        {
            return Err(MigrationError::new(format!(
                "Die App `{}` ist in ferrum.toml doppelt.",
                package.name
            )));
        }
    }
    if registered_names.len() != declared_names.len()
        || registered_names
            .keys()
            .any(|name| !declared_names.contains_key(name))
    {
        return Err(MigrationError::new(format!(
            "Rust-Apps mit Migrationen ({}) und [app.<name>] ({}) weichen voneinander ab.",
            joined_names(registered_names.keys()),
            joined_names(declared_names.keys())
        )));
    }
    for (name, registered) in &registered_names {
        let declared = declared_names.get(name).expect("name checked");
        if dependency_set(&registered.depends_on) != dependency_set(&declared.depends_on) {
            return Err(MigrationError::new(format!(
                "depends_on von `{name}` unterscheidet sich zwischen Rust und ferrum.toml."
            )));
        }
        if !same_directory(root, &declared.directory, &registered.directory) {
            return Err(MigrationError::new(format!(
                "Das Migrationsverzeichnis von `{name}` unterscheidet sich zwischen Rust und ferrum.toml."
            )));
        }
    }
    Ok(())
}

fn joined_names<'a>(names: impl Iterator<Item = &'a String>) -> String {
    let mut values = names.cloned().collect::<Vec<_>>();
    if values.is_empty() {
        return "keine".to_owned();
    }
    values.sort();
    values.join(", ")
}

fn dependency_set(names: &[String]) -> BTreeSet<String> {
    names.iter().cloned().collect()
}

fn same_directory(
    root: &std::path::Path,
    declared: &std::path::Path,
    registered: &std::path::Path,
) -> bool {
    let registered = if registered.is_absolute() {
        registered.to_path_buf()
    } else {
        root.join(registered)
    };
    match (declared.canonicalize(), registered.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => declared.components().eq(registered.components()),
    }
}

pub fn runtime_contract_note() -> &'static str {
    "app.migration.runtime Die CLI prüft die Verzeichnisse aus [app.<name>]. Ob registry.migrations dieselben Pfade anmeldet, prüft der Prozessstart. Diese Diagnose bestätigt die Übereinstimmung nicht."
}

pub fn package_order(packages: &[PackageMigration]) -> Result<Vec<usize>, MigrationError> {
    let mut index_of = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        if !crate::snapshot::is_identifier(&package.name) || package.name.len() > 32 {
            return Err(MigrationError::new(format!(
                "Ungültiger App-Name `{}`.",
                package.name
            )));
        }
        if index_of.insert(package.name.clone(), index).is_some() {
            return Err(MigrationError::new(format!(
                "Die App `{}` ist doppelt registriert.",
                package.name
            )));
        }
    }
    for package in packages {
        for dependency in &package.depends_on {
            if !index_of.contains_key(dependency) {
                return Err(MigrationError::new(format!(
                    "Die App `{}` hängt von der fehlenden App `{dependency}` ab.",
                    package.name
                )));
            }
            if dependency == &package.name {
                return Err(MigrationError::new(format!(
                    "Die App `{}` hängt von sich selbst ab.",
                    package.name
                )));
            }
        }
    }
    let mut indegree = vec![0_usize; packages.len()];
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); packages.len()];
    for (index, package) in packages.iter().enumerate() {
        for dependency in &package.depends_on {
            let from = index_of[dependency];
            outgoing[from].push(index);
            indegree[index] += 1;
        }
    }
    let mut ready: BTreeSet<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    let mut order = Vec::new();
    while let Some(index) = ready.pop_first() {
        order.push(index);
        for next in &outgoing[index] {
            indegree[*next] -= 1;
            if indegree[*next] == 0 {
                ready.insert(*next);
            }
        }
    }
    if order.len() != packages.len() {
        return Err(MigrationError::new(
            "Die App-Abhängigkeiten enthalten einen Zyklus.".to_owned(),
        ));
    }
    Ok(order)
}

pub async fn apply_packages(
    db: &sea_orm::DatabaseConnection,
    packages: &[PackageMigration],
    allow_destructive: bool,
) -> Result<Vec<String>, MigrationError> {
    let order = package_order(packages)?;
    let mut applied = Vec::new();
    for index in order {
        let package = &packages[index];
        let report =
            apply_namespaced_directory(db, &package.directory, &package.name, allow_destructive)
                .await?;
        applied.extend(report.applied);
    }
    Ok(applied)
}

pub async fn package_reports(
    db: &sea_orm::DatabaseConnection,
    packages: &[PackageMigration],
) -> Result<Vec<(String, crate::runner::StatusReport)>, MigrationError> {
    let order = package_order(packages)?;
    let mut reports = Vec::new();
    for index in order {
        let package = &packages[index];
        if !package.directory.is_dir() {
            return Err(MigrationError::new(format!(
                "Das Migrationsverzeichnis der App `{}` fehlt.",
                package.name
            )));
        }
        let status = namespaced_status(db, &package.directory, &package.name).await?;
        reports.push((package.name.clone(), status));
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::{apply_packages, package_order, same_migration_contract, PackageMigration};
    use crate::diff::plan_change;
    use crate::error::MigrationError;
    use crate::snapshot::{ColumnSnapshot, LogicalType, SchemaSnapshot, TableSnapshot};
    use crate::store::{save_migration, seal};
    use std::path::Path;

    fn write_initial(directory: &Path, table: &str) -> Result<(), MigrationError> {
        let from = SchemaSnapshot::empty();
        let to = SchemaSnapshot {
            format_version: 1,
            tables: vec![TableSnapshot {
                name: table.to_owned(),
                columns: vec![ColumnSnapshot {
                    name: "id".to_owned(),
                    logical_type: LogicalType::String,
                    primary_key: true,
                    nullable: false,
                    default: None,
                }],
                unique_constraints: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            }],
        };
        let plan = plan_change(&from, &to)?;
        let file = seal(
            "0001_initial".to_owned(),
            "initial".to_owned(),
            from,
            to,
            &plan,
        )?;
        save_migration(directory, &file)?;
        Ok(())
    }
    use crate::runner::{apply_directory, migration_status};
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, Value};

    fn node(name: &str, directory: &std::path::Path, depends_on: &[&str]) -> PackageMigration {
        PackageMigration {
            name: name.to_owned(),
            directory: directory.to_path_buf(),
            depends_on: depends_on.iter().map(|item| (*item).to_owned()).collect(),
        }
    }

    #[test]
    fn rust_and_toml_directories_must_name_the_same_apps() {
        let root = std::path::Path::new("project");
        let registered = [node(
            "shop",
            std::path::Path::new("apps/shop"),
            &["accounts"],
        )];
        let declared = [node("shop", &root.join("apps/shop"), &["accounts"])];
        assert!(same_migration_contract(&registered, &declared, root).is_ok());
        let other = [node("blog", &root.join("apps/shop"), &[])];
        assert!(same_migration_contract(&registered, &other, root).is_err());
        let dependency = [node("shop", &root.join("apps/shop"), &[])];
        assert!(same_migration_contract(&registered, &dependency, root).is_err());
    }

    #[test]
    fn cycles_missing_and_duplicates_are_rejected() {
        let missing = package_order(&[node(
            "billing",
            std::path::Path::new("billing"),
            &["accounts"],
        )]);
        assert!(missing.is_err());
        let cycle = package_order(&[
            node("accounts", std::path::Path::new("accounts"), &["billing"]),
            node("billing", std::path::Path::new("billing"), &["accounts"]),
        ]);
        assert!(cycle.is_err());
        let duplicate = package_order(&[
            node("shop", std::path::Path::new("a"), &[]),
            node("shop", std::path::Path::new("b"), &[]),
        ]);
        assert!(duplicate.is_err());
        let order = package_order(&[
            node("billing", std::path::Path::new("billing"), &["accounts"]),
            node("accounts", std::path::Path::new("accounts"), &[]),
        ])
        .unwrap();
        assert_eq!(order, vec![1, 0]);
    }

    #[tokio::test]
    async fn namespaced_apps_keep_legacy_rows_and_repeat_safely() {
        let db = ferrum_db::connect("sqlite::memory:").await.unwrap();
        let root = std::env::temp_dir().join(format!(
            "ferrum-apps-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy = root.join("legacy");
        let shop = root.join("shop");
        let blog = root.join("blog");
        write_initial(&legacy, "legacy_notes").unwrap();
        write_initial(&shop, "shop_notes").unwrap();
        write_initial(&blog, "blog_notes").unwrap();
        let first = apply_directory(&db, &legacy, false).await.unwrap();
        assert_eq!(first.applied, vec!["0001_initial".to_owned()]);
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO legacy_notes (id) VALUES (?)",
            [Value::from("kept")],
        ))
        .await
        .unwrap();
        let packages = vec![node("blog", &blog, &["shop"]), node("shop", &shop, &[])];
        let applied = apply_packages(&db, &packages, false).await.unwrap();
        assert_eq!(
            applied,
            vec![
                "shop:0001_initial".to_owned(),
                "blog:0001_initial".to_owned()
            ]
        );
        let again = apply_packages(&db, &packages, false).await.unwrap();
        assert!(again.is_empty());
        let legacy_again = apply_directory(&db, &legacy, false).await.unwrap();
        assert!(legacy_again.applied.is_empty());
        let status = migration_status(&db, &legacy, None).await.unwrap();
        assert!(status.conflicts.is_empty());
        assert_eq!(status.lines[0].state, "angewendet");
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM legacy_notes WHERE id = ?",
                [Value::from("kept")],
            ))
            .await
            .unwrap();
        assert!(row.is_some());
        let path = shop.join("0001_initial.json");
        let file = crate::store::load_migration(&path).unwrap();
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace(&file.checksum, &"0".repeat(file.checksum.len()));
        std::fs::write(&path, text).unwrap();
        assert!(namespaced_conflict(&db, &shop).await);
        let _ = std::fs::remove_dir_all(root);
    }

    async fn namespaced_conflict(
        db: &sea_orm::DatabaseConnection,
        directory: &std::path::Path,
    ) -> bool {
        crate::runner::namespaced_status(db, directory, "shop")
            .await
            .unwrap()
            .conflicts
            .iter()
            .any(|conflict| conflict.contains("Checksum"))
    }
}
