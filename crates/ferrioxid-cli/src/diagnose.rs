//! Diagnostics and checks without modifying the database.

use ferrioxid_migrations::{
    apply_upgrade, discover_project, inspect_directory, load_schema_from_command, plan_upgrade,
    readonly_database_url, render_plan, ProjectConfig, SchemaSnapshot, TEMPLATE_VERSION,
};
use std::process::{Command, ExitCode};

pub fn doctor() -> ExitCode {
    println!("Ferrioxid Doctor\n");
    let mut errors = 0;
    match command_version("rustc") {
        Some(version) => report("OK", &format!("Rust is installed: {version}")),
        None => {
            errors += 1;
            report(
                "ERROR",
                "rustc is missing. Install a stable Rust toolchain.",
            );
        }
    }
    match command_version("cargo") {
        Some(version) => report("OK", &format!("Cargo is installed: {version}")),
        None => {
            errors += 1;
            report("ERROR", "cargo is missing.");
        }
    }
    report("OK", "Docker is optional and not required.");
    let current = std::env::current_dir().ok();
    let Some(current) = current else {
        report("ERROR", "Cannot read the working directory.");
        return ExitCode::from(1);
    };
    match discover_project(&current) {
        Ok(project) => {
            report(
                "OK",
                &format!("ferrioxid.toml found in {}", project.root.display()),
            );
            report(
                "OK",
                "ferrioxid dev does not start a job worker. Run ferrioxid worker separately.",
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
                    "The migrations directory is missing. Create it before running migrate up.",
                );
            }
            for relative in ["Cargo.toml", "src/main.rs", "src/lib.rs"] {
                let path = project.root.join(relative);
                if path.is_file() {
                    report("OK", &format!("{relative} exists."));
                } else {
                    errors += 1;
                    report("ERROR", &format!("{relative} is missing."));
                }
            }
            if project.host == "0.0.0.0" {
                errors += 1;
                report(
                    "ERROR",
                    "server.host is 0.0.0.0. New projects should bind to 127.0.0.1.",
                );
            } else {
                report(
                    "OK",
                    &format!("Server is bound to {}:{}", project.host, project.port),
                );
            }
            if project.templates_enabled {
                if project.templates_directory().is_dir() {
                    report("OK", "Vorlagenverzeichnis exists.");
                } else {
                    errors += 1;
                    report(
                        "ERROR",
                        "templates.enabled is true, but the templates directory is missing.",
                    );
                }
            }
            if std::env::var("DATABASE_URL").is_ok() {
                report(
                    "OK",
                    "DATABASE_URL overrides the URL from ferrioxid.toml. Its value is hidden.",
                );
            }
            if project.database_url.starts_with("sqlite:") {
                report(
                    "OK",
                    "SQLite is configured. No database server is required.",
                );
            } else if project.database_url.starts_with("postgres:") {
                report(
                    "OK",
                    "PostgreSQL is configured and remains optional. Diagnostics use a read-only connection.",
                );
            } else {
                errors += 1;
                report(
                    "ERROR",
                    "The database URL must start with sqlite: or postgres:.",
                );
            }
            let counts = migration_lines(&project, false);
            errors += counts.errors;
            let project_counts = project_lines(&project);
            errors += project_counts.errors;
            if project.template.as_deref() == Some("fullstack") || project.embedded_assets {
                report("OK", "The admin UI is embedded in the Rust binary.");
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
    match ferrioxid_core::aliased_env("FERRIOXID_CSRF_SECRET") {
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
        Ok(Some(value)) => {
            let valid = value.trim().len() == 64
                && value.trim().chars().all(|char| char.is_ascii_hexdigit());
            if !valid {
                eprintln!(
                    "FERRIOXID_CSRF_SECRET or FERRUM_CSRF_SECRET must contain 64 hexadecimal characters."
                );
                return ExitCode::from(1);
            }
        }
        Ok(None) => {}
    }
    if let Ok(value) = std::env::var("DATABASE_URL") {
        if value.is_empty() {
            eprintln!("DATABASE_URL is empty.");
            return ExitCode::from(1);
        }
    }
    println!("Ferrioxid project check\n");
    report("OK", "project.config Project configuration is valid.");
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
    if std::env::var("FERRIOXID_CHECK_SKIP_CARGO").ok().as_deref() == Some("1") {
        println!("cargo check was skipped for this test run.");
        return if strict && warnings > 0 {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        };
    }
    println!("Running cargo check. No migrations are applied.");
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
            eprintln!("Could not start cargo check: {error}");
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
    println!("No migrations are applied and the database is not reset.");
    let writing = yes && !check_only && !dry_run;
    if !writing {
        println!("No files were written.");
        return if plan.has_conflict() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }
    match apply_upgrade(&project.root, &plan) {
        Ok(true) => {
            println!("Manifest geschrieben: .ferrioxid/project.json");
            println!("Template version for new projects: {TEMPLATE_VERSION}. Existing files remain unchanged.");
            ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("No file changes.");
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
            "migration.schema The model schema was not loaded. ferrioxid migrate status compares it when schema_command or FERRIOXID_SCHEMA_FILE is set.",
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
        let auth_files =
            ferrioxid_auth::embedded_migrations().map_err(|error| error.to_string())?;
        let jobs_files = ferrioxid_jobs::embedded_migrations()
            .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
        ferrioxid_migrations::inspect_framework(
            &db,
            &auth_files,
            &jobs_files,
            ferrioxid_jobs::JOB_HISTORY,
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
                    "migration.current Authentication, job, and app migrations are applied.",
                );
            }
            if jobs_pending {
                warnings += 1;
                report(
                    "WARN",
                    "job.migration.pending 0001_jobs The job migration has not been applied. Run ferrioxid migrate up. This check is read-only.",
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
    if std::env::var("FERRIOXID_CHECK_SKIP_SCHEMA").ok().as_deref() == Some("1") {
        return Ok(None);
    }
    if let Ok(path) = std::env::var("FERRIOXID_SCHEMA_FILE") {
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
        ferrioxid_migrations::DependencyKind::Git => report("OK", "project.dependency.git"),
        ferrioxid_migrations::DependencyKind::Registry => report(
            "OK",
            "project.dependency.registry Cargo version 0.1.0 is not the internal template version.",
        ),
        ferrioxid_migrations::DependencyKind::Missing => {
            report(
                "WARN",
                "project.dependency.unavailable Cargo.toml has no Ferrioxid dependency.",
            );
            warnings += 1;
        }
    }
    if dependency.admin_ui {
        report(
            "OK",
            "project.dependency.admin_ui remains valid for compatibility.",
        );
    }
    if project.static_enabled {
        if project.static_root().is_dir() {
            report("OK", "Static files directory exists.");
        } else {
            report(
                "WARN",
                "project.file.missing The static files directory is missing.",
            );
            warnings += 1;
        }
    }
    match project.enforce_runtime() {
        Ok(()) => report(
            "OK",
            "settings.runtime File settings, defaults, and environment overrides have been merged. No connection is opened.",
        ),
        Err(error) => {
            report("ERROR", &format!("settings.runtime {}", error.0));
            errors += 1;
        }
    }
    match project.mail_backend.as_str() {
        "disabled" => report(
            "OK",
            "mail.disabled No emails are sent and no SMTP server is required.",
        ),
        "console" | "memory" => report(
            "OK",
            "mail.local The backend runs in-process without opening an SMTP connection.",
        ),
        "smtp" => {
            report(
                "OK",
                "mail.smtp The configuration is valid. ferrioxid check does not connect to the server.",
            );
            match ferrioxid_core::aliased_env("FERRIOXID_SMTP_PASSWORD") {
                Err(error) => {
                    report("ERROR", &format!("mail.smtp {error}"));
                    errors += 1;
                }
                Ok(Some(value)) if !value.is_empty() => {}
                Ok(_) => {
                    report(
                        "WARN",
                        "mail.credentials FERRIOXID_SMTP_PASSWORD or FERRUM_SMTP_PASSWORD is missing. Its value is never displayed.",
                    );
                    warnings += 1;
                }
            }
        }
        _ => {
            report("WARN", "mail.backend is not recognized.");
            warnings += 1;
        }
    }
    if project.storage_root().is_dir() {
        report(
            "OK",
            "Storage directory exists. Route registration is not inferred from source code.",
        );
    } else {
        report(
            "WARN",
            "project.file.missing Storage directory is missing. This matters only if the app stores files.",
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
