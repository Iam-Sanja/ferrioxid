//! Sicherer Projekt-Upgrade-Plan. Unbekannte Dateien werden nicht überschrieben.

use crate::error::MigrationError;
use crate::manifest::{
    load_manifest, manifest_json, ProjectManifest, MANIFEST_FORMAT, MANIFEST_RELATIVE,
    TEMPLATE_VERSION,
};
use crate::store::sha256_hex;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Path,
    Git,
    Registry,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyReport {
    pub kind: DependencyKind,
    pub path: Option<String>,
    pub admin_ui: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanAction {
    Add {
        relative: String,
    },
    Unchanged {
        relative: String,
    },
    Skip {
        relative: String,
        code: &'static str,
        message: String,
    },
    Conflict {
        relative: String,
        code: &'static str,
        message: String,
    },
    Manual {
        code: &'static str,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePlan {
    pub actions: Vec<PlanAction>,
    pub dependency: DependencyReport,
}

impl UpgradePlan {
    pub fn has_conflict(&self) -> bool {
        self.actions
            .iter()
            .any(|action| matches!(action, PlanAction::Conflict { .. }))
    }

    pub fn adds(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action, PlanAction::Add { .. }))
            .count()
    }
}

pub fn ferrioxid_dependency(cargo_toml: &str) -> DependencyReport {
    let Some(start) = cargo_toml.find("ferrioxid") else {
        return DependencyReport {
            kind: DependencyKind::Missing,
            path: None,
            admin_ui: false,
        };
    };
    let window = &cargo_toml[start..];
    let window = &window[..window.len().min(500)];
    if !window.contains("ferrioxid") {
        return DependencyReport {
            kind: DependencyKind::Missing,
            path: None,
            admin_ui: false,
        };
    }
    let assignment = window.contains('=');
    if !assignment {
        return DependencyReport {
            kind: DependencyKind::Missing,
            path: None,
            admin_ui: cargo_toml.contains("admin-ui"),
        };
    }
    let admin_ui = window.contains("admin-ui");
    if window.contains("path") {
        return DependencyReport {
            kind: DependencyKind::Path,
            path: quoted_after(window, "path"),
            admin_ui,
        };
    }
    if window.contains("git") {
        return DependencyReport {
            kind: DependencyKind::Git,
            path: None,
            admin_ui,
        };
    }
    DependencyReport {
        kind: DependencyKind::Registry,
        path: None,
        admin_ui,
    }
}

pub fn plan_upgrade(root: &Path) -> Result<UpgradePlan, MigrationError> {
    let root = root.canonicalize().map_err(|error| {
        MigrationError::new(format!(
            "project.file.missing: Projektwurzel ist nicht lesbar: {error}"
        ))
    })?;
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let dependency = ferrioxid_dependency(&cargo);
    let mut actions = Vec::new();
    if dependency.kind == DependencyKind::Missing {
        actions.push(PlanAction::Manual {
            code: "project.dependency.unavailable",
            message: "Cargo.toml enthält keine Ferrioxid-Abhängigkeit.".to_owned(),
        });
    }
    if dependency.admin_ui {
        actions.push(PlanAction::Manual {
            code: "project.dependency.admin_ui",
            message: "Das Feature admin-ui bleibt aus Kompatibilität gültig und muss nicht entfernt werden.".to_owned(),
        });
    }
    let manifest_path = contained_target(&root, MANIFEST_RELATIVE)?;
    if !manifest_path.is_file() {
        actions.push(PlanAction::Manual {
            code: "project.template.legacy",
            message: "Es gibt kein Upgrade-Manifest. Vorhandene Dateien gelten als unbekannt und werden nicht überschrieben.".to_owned(),
        });
        actions.push(PlanAction::Add {
            relative: MANIFEST_RELATIVE.to_owned(),
        });
        return Ok(UpgradePlan {
            actions,
            dependency,
        });
    }
    let manifest = match load_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            actions.push(PlanAction::Conflict {
                relative: MANIFEST_RELATIVE.to_owned(),
                code: "upgrade.conflict",
                message: error.0,
            });
            return Ok(UpgradePlan {
                actions,
                dependency,
            });
        }
    };
    if manifest.format != MANIFEST_FORMAT {
        actions.push(PlanAction::Conflict {
            relative: MANIFEST_RELATIVE.to_owned(),
            code: "upgrade.conflict",
            message: format!(
                "Manifestformat {} wird nicht automatisch umgeschrieben.",
                manifest.format
            ),
        });
        return Ok(UpgradePlan {
            actions,
            dependency,
        });
    }
    if manifest.origin == "legacy" {
        actions.push(PlanAction::Unchanged {
            relative: MANIFEST_RELATIVE.to_owned(),
        });
        actions.push(PlanAction::Manual {
            code: "project.template.legacy",
            message: "Legacy-Manifest. Bestehende Projektdateien bleiben unverändert.".to_owned(),
        });
        return Ok(UpgradePlan {
            actions,
            dependency,
        });
    }
    if manifest.template_version != TEMPLATE_VERSION {
        actions.push(PlanAction::Manual {
            code: "project.template.outdated",
            message: format!(
                "Template {} ist nicht {}. Dateien werden deshalb nicht ersetzt.",
                manifest.template_version, TEMPLATE_VERSION
            ),
        });
    }
    for tracked in &manifest.files {
        match contained_target(&root, &tracked.path) {
            Err(error) => actions.push(PlanAction::Conflict {
                relative: tracked.path.clone(),
                code: "upgrade.unsafe_path",
                message: error.0,
            }),
            Ok(path) if !path.is_file() => actions.push(PlanAction::Conflict {
                relative: tracked.path.clone(),
                code: "project.file.missing",
                message: format!(
                    "{} fehlt. Ferrioxid legt diese Datei nicht neu an.",
                    tracked.path
                ),
            }),
            Ok(path) => {
                let bytes = std::fs::read(&path).map_err(|error| {
                    MigrationError::new(format!(
                        "project.file.missing: {} ist nicht lesbar: {error}",
                        path.display()
                    ))
                })?;
                if sha256_hex(&bytes) == tracked.sha256 {
                    actions.push(PlanAction::Unchanged {
                        relative: tracked.path.clone(),
                    });
                } else {
                    actions.push(PlanAction::Conflict {
                        relative: tracked.path.clone(),
                        code: "project.file.modified",
                        message: format!(
                            "{} weicht vom Manifest ab und wird nicht überschrieben.",
                            tracked.path
                        ),
                    });
                }
            }
        }
    }
    Ok(UpgradePlan {
        actions,
        dependency,
    })
}

pub fn render_plan(locale: &str, plan: &UpgradePlan, dry_run: bool) -> String {
    let mut text = if locale == "de" {
        "Ferrioxid Upgrade\n\n".to_owned()
    } else {
        "Ferrioxid upgrade\n\n".to_owned()
    };
    text.push_str(&match plan.dependency.kind {
        DependencyKind::Path => format!(
            "project.dependency.path {}\n",
            plan.dependency.path.as_deref().unwrap_or("")
        ),
        DependencyKind::Git => "project.dependency.git\n".to_owned(),
        DependencyKind::Registry => "project.dependency.registry\n".to_owned(),
        DependencyKind::Missing => "project.dependency.unavailable\n".to_owned(),
    });
    if plan.dependency.admin_ui {
        text.push_str("project.dependency.admin_ui optional\n");
    }
    if dry_run {
        text.push_str(if locale == "de" {
            "\nGeplante Dateiänderungen:\n"
        } else {
            "\nPlanned file changes:\n"
        });
    }
    if plan.actions.is_empty() {
        text.push_str(if locale == "de" {
            "Keine Dateiänderung.\n"
        } else {
            "No file change.\n"
        });
    }
    for action in &plan.actions {
        text.push_str(&match action {
            PlanAction::Add { relative } => format!("upgrade.plan.add {relative}\n"),
            PlanAction::Unchanged { relative } => format!("upgrade.unchanged {relative}\n"),
            PlanAction::Skip {
                relative,
                code,
                message,
            } => format!("{code} {relative} {message}\n"),
            PlanAction::Conflict {
                relative,
                code,
                message,
            } => format!("{code} {relative} {message}\n"),
            PlanAction::Manual { code, message } => format!("{code} {message}\n"),
        });
    }
    text
}

/// Schreibt nur das Legacy-Manifest, und nur wenn der Plan genau das vorsieht.
pub fn apply_upgrade(root: &Path, plan: &UpgradePlan) -> Result<bool, MigrationError> {
    if plan.has_conflict() {
        return Err(MigrationError::new(
            "upgrade.conflict: Es gibt Konflikte. Es wurde keine Datei geschrieben.".to_owned(),
        ));
    }
    let adds: Vec<&str> = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            PlanAction::Add { relative } => Some(relative.as_str()),
            _ => None,
        })
        .collect();
    if adds.is_empty() {
        return Ok(false);
    }
    if adds != [MANIFEST_RELATIVE] {
        return Err(MigrationError::new(
            "upgrade.conflict: Nur das neue Manifest ist eine sichere Dateiänderung.".to_owned(),
        ));
    }
    let fresh = plan_upgrade(root)?;
    let still = fresh.actions.iter().any(
        |action| matches!(action, PlanAction::Add { relative } if relative == MANIFEST_RELATIVE),
    );
    if !still || fresh.has_conflict() {
        return Err(MigrationError::new(
            "upgrade.conflict: Das Projekt hat sich seit dem Plan geändert. Der Vorgang wurde abgebrochen.".to_owned(),
        ));
    }
    write_new_file(
        root,
        MANIFEST_RELATIVE,
        &manifest_json(&ProjectManifest::legacy())?,
    )?;
    Ok(true)
}

pub fn contained_target(root: &Path, relative: &str) -> Result<PathBuf, MigrationError> {
    if relative.is_empty()
        || relative.contains('\0')
        || Path::new(relative).is_absolute()
        || relative.starts_with('/')
        || relative.starts_with('\\')
    {
        return Err(MigrationError::new(format!(
            "upgrade.unsafe_path: `{relative}` liegt nicht im Projekt."
        )));
    }
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(MigrationError::new(format!(
                    "upgrade.unsafe_path: `{relative}` verlässt das Projekt."
                )));
            }
        }
    }
    let root = if root.is_absolute() {
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
    } else {
        root.canonicalize()
            .map_err(|error| MigrationError::new(format!("upgrade.unsafe_path: {error}")))?
    };
    let target = root.join(relative);
    if target.exists() {
        let canonical = target
            .canonicalize()
            .map_err(|error| MigrationError::new(format!("upgrade.unsafe_path: {error}")))?;
        if !canonical.starts_with(&root) {
            return Err(MigrationError::new(format!(
                "upgrade.unsafe_path: `{relative}` zeigt aus dem Projekt heraus."
            )));
        }
        return Ok(canonical);
    }
    let mut cursor = target.as_path();
    let mut missing = Vec::new();
    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            return Err(MigrationError::new(format!(
                "upgrade.unsafe_path: `{relative}` ist kein Projektpfad."
            )));
        };
        missing.push(name.to_os_string());
        cursor = cursor.parent().ok_or_else(|| {
            MigrationError::new(format!(
                "upgrade.unsafe_path: `{relative}` hat kein übergeordnetes Verzeichnis."
            ))
        })?;
    }
    let mut rebuilt = cursor
        .canonicalize()
        .map_err(|error| MigrationError::new(format!("upgrade.unsafe_path: {error}")))?;
    if !rebuilt.starts_with(&root) {
        return Err(MigrationError::new(format!(
            "upgrade.unsafe_path: `{relative}` zeigt aus dem Projekt heraus."
        )));
    }
    for name in missing.iter().rev() {
        rebuilt.push(name);
    }
    Ok(rebuilt)
}

fn write_new_file(root: &Path, relative: &str, contents: &str) -> Result<(), MigrationError> {
    let destination = contained_target(root, relative)?;
    if destination.exists() {
        return Err(MigrationError::new(
            "upgrade.conflict: Die Zieldatei existiert bereits und wird nicht ersetzt.".to_owned(),
        ));
    }
    let partial_relative = format!("{relative}.partial");
    let partial = contained_target(root, &partial_relative)?;
    if let Some(parent) = partial.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            MigrationError::new(format!("upgrade.conflict: Verzeichnis fehlt: {error}"))
        })?;
    }
    if partial.exists() {
        backup_file(root, &partial)?;
    }
    std::fs::write(&partial, contents).map_err(|error| {
        MigrationError::new(format!("upgrade.conflict: Schreiben abgebrochen: {error}"))
    })?;
    if destination.exists() {
        let _ = std::fs::remove_file(&partial);
        return Err(MigrationError::new(
            "upgrade.conflict: Die Zieldatei wurde während des Schreibens angelegt. Abbruch ohne Ersatz.".to_owned(),
        ));
    }
    std::fs::rename(&partial, &destination).map_err(|error| {
        MigrationError::new(format!(
            "upgrade.conflict: Die Datei wurde nicht eingewechselt: {error}"
        ))
    })?;
    Ok(())
}

fn backup_file(root: &Path, source: &Path) -> Result<(), MigrationError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let relative = format!(".ferrum/backup/{stamp}/{name}");
    let target = contained_target(root, &relative)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            MigrationError::new(format!("upgrade.conflict: Backup-Verzeichnis: {error}"))
        })?;
    }
    std::fs::rename(source, &target).map_err(|error| {
        MigrationError::new(format!(
            "upgrade.conflict: Backup ist fehlgeschlagen: {error}"
        ))
    })?;
    Ok(())
}

fn quoted_after(window: &str, key: &str) -> Option<String> {
    let start = window.find(key)?;
    let after = &window[start + key.len()..];
    let quote = after.find('"')?;
    let rest = &after[quote + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ProjectManifest, TrackedFile, MANIFEST_RELATIVE};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ferrioxid-upgrade-{nanos}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn cargo(path_value: &str) -> String {
        format!(
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nferrioxid = {{ path = \"{path_value}\", features = [\"admin-ui\"] }}\n"
        )
    }

    #[test]
    fn windows_path_dependency_is_not_a_version_error() {
        let report = ferrioxid_dependency(&cargo(
            r"D:\04_Projekte_Arbeit\Projekte\ferrioxid\crates\ferrioxid",
        ));
        assert_eq!(report.kind, DependencyKind::Path);
        assert!(report.path.unwrap().contains("ferrioxid"));
        assert!(report.admin_ui);
        let git =
            ferrioxid_dependency("ferrioxid = { git = \"https://example.test/ferrioxid.git\" }\n");
        assert_eq!(git.kind, DependencyKind::Git);
        let registry = ferrioxid_dependency("ferrioxid = \"0.1.0\"\n");
        assert_eq!(registry.kind, DependencyKind::Registry);
    }

    #[test]
    fn path_traversal_and_legacy_plan_do_not_write() {
        let root = scratch();
        std::fs::write(
            root.join("Cargo.toml"),
            cargo("..\\ferrioxid\\crates\\ferrioxid"),
        )
        .unwrap();
        std::fs::write(root.join("secret.txt"), "behalten").unwrap();
        let error = contained_target(&root, "../secret.txt").unwrap_err();
        assert!(error.0.contains("upgrade.unsafe_path"));
        let plan = plan_upgrade(&root).unwrap();
        assert!(plan.adds() == 1);
        assert!(!root.join(".ferrum").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("secret.txt")).unwrap(),
            "behalten"
        );
        apply_upgrade(&root, &plan).unwrap();
        assert!(root.join(".ferrum").join("project.json").is_file());
        assert_eq!(
            std::fs::read_to_string(root.join("secret.txt")).unwrap(),
            "behalten"
        );
        let again = plan_upgrade(&root).unwrap();
        assert_eq!(again.adds(), 0);
        assert!(!apply_upgrade(&root, &again).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn modified_tracked_file_is_not_replaced() {
        let root = scratch();
        std::fs::write(
            root.join("Cargo.toml"),
            cargo("C:/ferrioxid/crates/ferrioxid"),
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "eigen").unwrap();
        let manifest = ProjectManifest::generated(
            "api",
            vec![TrackedFile {
                path: "README.md".to_owned(),
                sha256: "deadbeef".to_owned(),
            }],
        );
        std::fs::create_dir_all(root.join(".ferrum")).unwrap();
        std::fs::write(
            root.join(MANIFEST_RELATIVE),
            manifest_json(&manifest).unwrap(),
        )
        .unwrap();
        let plan = plan_upgrade(&root).unwrap();
        assert!(plan.has_conflict());
        assert!(plan
            .actions
            .iter()
            .any(|action| matches!(action, PlanAction::Conflict { code, .. } if *code == "project.file.modified")));
        let error = apply_upgrade(&root, &plan).unwrap_err();
        assert!(error.0.contains("upgrade.conflict"));
        assert_eq!(
            std::fs::read_to_string(root.join("README.md")).unwrap(),
            "eigen"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn partial_manifest_is_backed_up_before_a_new_write() {
        let root = scratch();
        std::fs::write(
            root.join("Cargo.toml"),
            cargo("C:/ferrioxid/crates/ferrioxid"),
        )
        .unwrap();
        let partial_dir = root.join(".ferrum");
        std::fs::create_dir_all(&partial_dir).unwrap();
        std::fs::write(partial_dir.join("project.json.partial"), "abbruch").unwrap();
        let plan = plan_upgrade(&root).unwrap();
        apply_upgrade(&root, &plan).unwrap();
        assert!(std::fs::read_to_string(root.join(".ferrum/project.json"))
            .unwrap()
            .contains("legacy"));
        assert!(!partial_dir.join("project.json.partial").exists());
        let backup = partial_dir.join("backup");
        assert!(backup.is_dir());
        let mut found = false;
        for entry in walk(&backup) {
            if std::fs::read_to_string(&entry).ok().as_deref() == Some("abbruch") {
                found = true;
            }
        }
        assert!(found);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_aborts_when_the_manifest_appears_after_the_plan() {
        let root = scratch();
        std::fs::write(
            root.join("Cargo.toml"),
            cargo("C:/ferrioxid/crates/ferrioxid"),
        )
        .unwrap();
        let plan = plan_upgrade(&root).unwrap();
        std::fs::create_dir_all(root.join(".ferrum")).unwrap();
        std::fs::write(root.join(MANIFEST_RELATIVE), "{\"format\":1}\n").unwrap();
        let error = apply_upgrade(&root, &plan).unwrap_err();
        assert!(error.0.contains("seit dem Plan"));
        assert_eq!(
            std::fs::read_to_string(root.join(MANIFEST_RELATIVE)).unwrap(),
            "{\"format\":1}\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn junction_outside_the_project_is_refused() {
        let root = scratch();
        let outside = scratch();
        std::fs::write(
            root.join("Cargo.toml"),
            cargo("C:/ferrioxid/crates/ferrioxid"),
        )
        .unwrap();
        let link = root.join(".ferrum");
        let status = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &link.display().to_string(),
                &outside.display().to_string(),
            ])
            .status();
        if status.map(|status| status.success()).unwrap_or(false) {
            let error = contained_target(&root, MANIFEST_RELATIVE).unwrap_err();
            assert!(error.0.contains("upgrade.unsafe_path"));
            assert!(!outside.join("project.json").exists());
        }
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else {
                files.push(path);
            }
        }
        files
    }
}
