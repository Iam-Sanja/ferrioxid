//! Diagnose und Prüfung ohne Schreibzugriff auf die Datenbank.

use ferrioxid_migrations::{
    apply_upgrade, discover_project, inspect_directory, load_schema_from_command, plan_upgrade,
    readonly_database_url, render_plan, ProjectConfig, SchemaSnapshot, TEMPLATE_VERSION,
};
use std::process::{Command, ExitCode};

pub fn doctor() -> ExitCode {
    println!("Ferrioxid Doctor\n");
    let mut errors = 0;
    match command_version("rustc") {
        Some(version) => report("OK", &format!("Rust ist installiert: {version}")),
        None => {
            errors += 1;
            report(
                "ERROR",
                "rustc fehlt. Installiere eine stabile Rust-Toolchain.",
            );
        }
    }
    match command_version("cargo") {
        Some(version) => report("OK", &format!("Cargo ist installiert: {version}")),
        None => {
            errors += 1;
            report("ERROR", "cargo fehlt.");
        }
    }
    report("OK", "Docker ist optional und wird nicht vorausgesetzt.");
    let current = std::env::current_dir().ok();
    let Some(current) = current else {
        report("ERROR", "Das Arbeitsverzeichnis ist nicht lesbar.");
        return ExitCode::from(1);
    };
    match discover_project(&current) {
        Ok(project) => {
            report(
                "OK",
                &format!("ferrum.toml gefunden in {}", project.root.display()),
            );
            report(
                "OK",
                "ferrioxid dev startet keinen Job-Worker. Der Worker heißt ferrioxid worker.",
            );
            if project.migrations_dir.is_dir() {
                report(
                    "OK",
                    &format!("Migrationsverzeichnis {}", project.migrations_dir.display()),
                );
            } else {
                errors += 1;
                report(
                    "ERROR",
                    "Das Migrationsverzeichnis fehlt. Lege es an, bevor du migrate up ausführst.",
                );
            }
            for relative in ["Cargo.toml", "src/main.rs", "src/lib.rs"] {
                let path = project.root.join(relative);
                if path.is_file() {
                    report("OK", &format!("{relative} ist vorhanden."));
                } else {
                    errors += 1;
                    report("ERROR", &format!("{relative} fehlt."));
                }
            }
            if project.host == "0.0.0.0" {
                errors += 1;
                report(
                    "ERROR",
                    "server.host ist 0.0.0.0. Neue Projekte bleiben auf 127.0.0.1.",
                );
            } else {
                report(
                    "OK",
                    &format!("Serverbindung ist {}:{}", project.host, project.port),
                );
            }
            if project.templates_enabled {
                if project.templates_directory().is_dir() {
                    report("OK", "Vorlagenverzeichnis ist vorhanden.");
                } else {
                    errors += 1;
                    report(
                        "ERROR",
                        "templates.enabled ist wahr, aber das Vorlagenverzeichnis fehlt.",
                    );
                }
            }
            if std::env::var("DATABASE_URL").is_ok() {
                report(
                    "OK",
                    "DATABASE_URL überschreibt die URL aus ferrum.toml. Der Wert wird nicht angezeigt.",
                );
            }
            if project.database_url.starts_with("sqlite:") {
                report(
                    "OK",
                    "SQLite ist konfiguriert. Es ist kein Datenbankserver nötig.",
                );
            } else if project.database_url.starts_with("postgres:") {
                report(
                    "OK",
                    "PostgreSQL ist konfiguriert und bleibt optional. Die Diagnose verbindet sich nur lesend.",
                );
            } else {
                errors += 1;
                report(
                    "ERROR",
                    "Die Datenbank-URL muss mit sqlite: oder postgres: beginnen.",
                );
            }
            let counts = migration_lines(&project, false);
            errors += counts.errors;
            let project_counts = project_lines(&project);
            errors += project_counts.errors;
            if project.template.as_deref() == Some("fullstack") || project.embedded_assets {
                report("OK", "Der Admin wird mit dem Rust-Binary ausgeliefert.");
            }
        }
        Err(error) => {
            errors += 1;
            report("ERROR", &error.to_string());
        }
    }
    if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

pub fn check(strict: bool) -> ExitCode {
    let current = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let project = match discover_project(&current) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if let Ok(value) = std::env::var("FERRUM_CSRF_SECRET") {
        let valid =
            value.trim().len() == 64 && value.trim().chars().all(|char| char.is_ascii_hexdigit());
        if !valid {
            eprintln!("FERRUM_CSRF_SECRET muss 64 Hex-Zeichen haben.");
            return ExitCode::from(1);
        }
    }
    if let Ok(value) = std::env::var("DATABASE_URL") {
        if value.is_empty() {
            eprintln!("DATABASE_URL ist leer.");
            return ExitCode::from(1);
        }
    }
    println!("Ferrioxid-Projektprüfung\n");
    report("OK", "project.config Projektkonfiguration gültig.");
    let state = inspect_directory(&project.migrations_dir);
    if !state.conflicts.is_empty() {
        for conflict in state.conflicts {
            eprintln!("migration.order {conflict}");
        }
        return ExitCode::from(1);
    }
    let counts = migration_lines(&project, true);
    let project_counts = project_lines(&project);
    let warnings = counts.warnings + project_counts.warnings;
    if counts.errors > 0 || project_counts.errors > 0 {
        return ExitCode::from(1);
    }
    if std::env::var("FERRUM_CHECK_SKIP_CARGO").ok().as_deref() == Some("1") {
        println!("cargo check wurde für diesen Testlauf übersprungen.");
        return if strict && warnings > 0 {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        };
    }
    println!("Starte cargo check. Es wird keine Migration ausgeführt.");
    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project.root)
        .status();
    match status {
        Ok(status) if status.success() => {
            if strict && warnings > 0 {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("cargo check konnte nicht gestartet werden: {error}");
            ExitCode::from(1)
        }
    }
}

pub fn upgrade(check_only: bool, dry_run: bool, yes: bool) -> ExitCode {
    let current = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let project = match discover_project(&current) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let _ = migration_lines(&project, false);
    let plan = match plan_upgrade(&project.root) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    print!(
        "{}",
        render_plan(&crate::text::locale(), &plan, dry_run || check_only)
    );
    println!("Migrationen werden nicht angewendet. Es gibt kein Datenbank-Reset.");
    let writing = yes && !check_only && !dry_run;
    if !writing {
        println!("Es wurde keine Datei geschrieben.");
        return if plan.has_conflict() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }
    match apply_upgrade(&project.root, &plan) {
        Ok(true) => {
            println!("Manifest geschrieben: .ferrum/project.json");
            println!("Template-Stand für neue Projekte: {TEMPLATE_VERSION}. Bestehende Dateien bleiben unverändert.");
            ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("Keine Dateiänderung.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn tool(program: &str) -> Command {
    if cfg!(windows) && program == "npm" {
        Command::new("npm.cmd")
    } else {
        Command::new(program)
    }
}

fn report(level: &str, message: &str) {
    println!("[{level}] {message}");
}

fn command_version(program: &str) -> Option<String> {
    let output = tool(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().next().unwrap_or(program).trim().to_owned())
}

struct Counts {
    errors: i32,
    warnings: i32,
}

fn migration_lines(project: &ProjectConfig, compare_schema: bool) -> Counts {
    if !project.apps.is_empty() {
        report("OK", ferrioxid_migrations::runtime_contract_note());
    }
    let url = project.resolved_database_url();
    let readonly = match readonly_database_url(&url) {
        Ok(url) => url,
        Err(error) => {
            report("WARN", &error.0);
            return Counts {
                errors: 0,
                warnings: 1,
            };
        }
    };
    let schema = if compare_schema {
        match load_check_schema(project) {
            Ok(schema) => schema,
            Err(error) => {
                report("ERROR", &format!("migration.schema_conflict {error}"));
                return Counts {
                    errors: 1,
                    warnings: 0,
                };
            }
        }
    } else {
        None
    };
    if compare_schema && schema.is_none() {
        report(
            "OK",
            "migration.schema Das Modellschema wurde nicht geladen. ferrioxid migrate status vergleicht es, wenn schema_command oder FERRUM_SCHEMA_FILE gesetzt ist.",
        );
    }
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            report("ERROR", &format!("migration.database.unavailable: {error}"));
            return Counts {
                errors: 1,
                warnings: 0,
            };
        }
    };
    let inspected = runtime.block_on(async {
        let db = ferrioxid_db::connect(&readonly)
            .await
            .map_err(|error| format!("migration.database.unavailable: {error}"))?;
        let packages: Vec<ferrioxid_migrations::PackageMigration> = project
            .apps
            .iter()
            .map(|app| ferrioxid_migrations::PackageMigration {
                name: app.name.clone(),
                directory: app.migrations.clone(),
                depends_on: app.depends_on.clone(),
            })
            .collect();
        ferrioxid_migrations::inspect_migrations_with_packages(
            &db,
            &ferrioxid_auth::migrations_dir(),
            &project.migrations_dir,
            schema.as_ref(),
            &packages,
        )
        .await
        .map_err(|error| error.0)
    });
    match inspected {
        Ok(report_value) => {
            let mut errors = 0;
            let mut warnings = 0;
            let jobs_pending = jobs_pending(&readonly);
            if report_value.pending_auth() == 0
                && report_value.pending_app() == 0
                && report_value.pending_packages() == 0
                && !jobs_pending
                && !report_value.has_error()
            {
                report(
                    "OK",
                    "migration.current Auth-, Job- und App-Migrationen sind angewendet.",
                );
            }
            if jobs_pending {
                warnings += 1;
                report(
                    "WARN",
                    "job.migration.pending 0001_jobs Die Job-Migration ist nicht angewendet. ferrioxid migrate up holt sie nach. Diese Prüfung schreibt nicht.",
                );
            }
            for item in &report_value.diagnostics {
                let level = match item.level {
                    ferrioxid_migrations::Level::Ok => "OK",
                    ferrioxid_migrations::Level::Warn => "WARN",
                    ferrioxid_migrations::Level::Error => "ERROR",
                };
                report(
                    level,
                    &format!("{} {} {}", item.code, item.migration_id, item.description),
                );
                match item.level {
                    ferrioxid_migrations::Level::Error => errors += 1,
                    ferrioxid_migrations::Level::Warn => warnings += 1,
                    ferrioxid_migrations::Level::Ok => {}
                }
            }
            if report_value.pending_auth()
                + report_value.pending_app()
                + report_value.pending_packages()
                > 0
            {
                println!("Empfehlung:\n  ferrioxid migrate up");
            }
            Counts { errors, warnings }
        }
        Err(error) => {
            report("ERROR", &error);
            Counts {
                errors: 1,
                warnings: 0,
            }
        }
    }
}

fn jobs_pending(url: &str) -> bool {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return false;
    };
    runtime
        .block_on(async {
            let db = ferrioxid_db::connect(url).await.ok()?;
            let (_text, current) = ferrioxid_jobs::render_migration_status(&db).await.ok()?;
            Some(!current)
        })
        .unwrap_or(false)
}

fn load_check_schema(project: &ProjectConfig) -> Result<Option<SchemaSnapshot>, String> {
    if std::env::var("FERRUM_CHECK_SKIP_SCHEMA").ok().as_deref() == Some("1") {
        return Ok(None);
    }
    if let Ok(path) = std::env::var("FERRUM_SCHEMA_FILE") {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        return Ok(Some(
            SchemaSnapshot::from_json(&text).map_err(|error| error.0)?,
        ));
    }
    let Some(command) = &project.schema_command else {
        return Ok(None);
    };
    Ok(Some(
        load_schema_from_command(&project.root, command).map_err(|error| error.0)?,
    ))
}

struct ProjectCounts {
    errors: i32,
    warnings: i32,
}

fn project_lines(project: &ProjectConfig) -> ProjectCounts {
    let mut warnings = 0;
    let mut errors = 0;
    let cargo = std::fs::read_to_string(project.root.join("Cargo.toml")).unwrap_or_default();
    let dependency = ferrioxid_migrations::ferrioxid_dependency(&cargo);
    match dependency.kind {
        ferrioxid_migrations::DependencyKind::Path => report(
            "OK",
            &format!(
                "project.dependency.path {}",
                dependency.path.as_deref().unwrap_or("")
            ),
        ),
        ferrioxid_migrations::DependencyKind::Git => {
            report("OK", "project.dependency.git")
        }
        ferrioxid_migrations::DependencyKind::Registry => {
            report("OK", "project.dependency.registry Die Cargo-Version 0.1.0 ist nicht die interne Template-Version.")
        }
        ferrioxid_migrations::DependencyKind::Missing => {
            report("WARN", "project.dependency.unavailable Cargo.toml enthält keine Ferrioxid-Abhängigkeit.");
            warnings += 1;
        }
    }
    if dependency.admin_ui {
        report(
            "OK",
            "project.dependency.admin_ui bleibt aus Kompatibilität gültig.",
        );
    }
    if project.static_enabled {
        if project.static_root().is_dir() {
            report("OK", "Staticfiles-Verzeichnis ist vorhanden.");
        } else {
            report(
                "WARN",
                "project.file.missing Das Staticfiles-Verzeichnis fehlt.",
            );
            warnings += 1;
        }
    }
    match project.enforce_runtime() {
        Ok(()) => report(
            "OK",
            "settings.runtime Datei, Defaults und Umgebungsvariablen sind zusammengeführt. Es wird keine Verbindung geöffnet.",
        ),
        Err(error) => {
            report("ERROR", &format!("settings.runtime {}", error.0));
            errors += 1;
        }
    }
    match project.mail_backend.as_str() {
        "disabled" => report(
            "OK",
            "mail.disabled Es wird keine E-Mail versendet und kein SMTP-Server erwartet.",
        ),
        "console" | "memory" => report(
            "OK",
            "mail.local Das Backend bleibt im Prozess. Es öffnet keine SMTP-Verbindung.",
        ),
        "smtp" => {
            report(
                "OK",
                "mail.smtp Die Form ist gültig. ferrioxid check verbindet sich nicht mit dem Server.",
            );
            if std::env::var("FERRUM_SMTP_PASSWORD")
                .ok()
                .filter(|value| !value.is_empty())
                .is_none()
            {
                report(
                    "WARN",
                    "mail.credentials FERRUM_SMTP_PASSWORD fehlt. Der Wert wird nicht angezeigt.",
                );
                warnings += 1;
            }
        }
        _ => {
            report("WARN", "mail.backend ist nicht bekannt.");
            warnings += 1;
        }
    }
    if project.storage_root().is_dir() {
        report(
            "OK",
            "Storage-Verzeichnis ist vorhanden. Ob die Anwendung Routen einhängt, wird nicht aus dem Quelltext geschlossen.",
        );
    } else {
        report(
            "WARN",
            "project.file.missing Storage-Verzeichnis fehlt. Das ist nur relevant, wenn die Anwendung Dateien speichert.",
        );
        warnings += 1;
    }
    match plan_upgrade(&project.root) {
        Ok(plan) => {
            for action in &plan.actions {
                if let ferrioxid_migrations::PlanAction::Manual { code, message } = action {
                    if *code == "project.template.legacy" || *code == "project.template.outdated" {
                        report("WARN", &format!("{code} {message}"));
                        warnings += 1;
                    }
                }
                if let ferrioxid_migrations::PlanAction::Conflict {
                    code,
                    relative,
                    message,
                } = action
                {
                    report("WARN", &format!("{code} {relative} {message}"));
                    warnings += 1;
                }
            }
        }
        Err(error) => {
            report("WARN", &error.0);
            warnings += 1;
        }
    }
    ProjectCounts { errors, warnings }
}
