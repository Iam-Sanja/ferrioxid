//! Diagnose und Prüfung ohne Schreibzugriff auf die Datenbank.

use ferrum_migrations::{discover_project, inspect_directory};
use std::path::Path;
use std::process::{Command, ExitCode};

pub fn doctor() -> ExitCode {
    let mut errors = 0;
    match command_version("rustc") {
        Some(version) => report("ok", &format!("Rust ist installiert: {version}")),
        None => {
            errors += 1;
            report(
                "fehler",
                "rustc fehlt. Installiere eine stabile Rust-Toolchain.",
            );
        }
    }
    match command_version("cargo") {
        Some(version) => report("ok", &format!("Cargo ist installiert: {version}")),
        None => {
            errors += 1;
            report("fehler", "cargo fehlt.");
        }
    }
    report("ok", "Docker ist optional und wird nicht vorausgesetzt.");
    let current = std::env::current_dir().ok();
    let Some(current) = current else {
        report("fehler", "Das Arbeitsverzeichnis ist nicht lesbar.");
        return ExitCode::from(1);
    };
    match discover_project(&current) {
        Ok(project) => {
            report(
                "ok",
                &format!("ferrum.toml gefunden in {}", project.root.display()),
            );
            if project.migrations_dir.is_dir() {
                report(
                    "ok",
                    &format!("Migrationsverzeichnis {}", project.migrations_dir.display()),
                );
            } else {
                errors += 1;
                report(
                    "fehler",
                    "Das Migrationsverzeichnis fehlt. Lege es an, bevor du migrate up ausführst.",
                );
            }
            for relative in ["Cargo.toml", "src/main.rs", "src/lib.rs"] {
                let path = project.root.join(relative);
                if path.is_file() {
                    report("ok", &format!("{relative} ist vorhanden."));
                } else {
                    errors += 1;
                    report("fehler", &format!("{relative} fehlt."));
                }
            }
            if project.host == "0.0.0.0" {
                errors += 1;
                report(
                    "fehler",
                    "server.host ist 0.0.0.0. Neue Projekte bleiben auf 127.0.0.1.",
                );
            } else {
                report(
                    "ok",
                    &format!("Serverbindung ist {}:{}", project.host, project.port),
                );
            }
            if project.database_url.starts_with("sqlite:") {
                report(
                    "ok",
                    "SQLite ist konfiguriert. Es ist kein Datenbankserver nötig.",
                );
                if !sqlite_file_exists(&project.database_url) {
                    report(
                        "warnung",
                        "Die SQLite-Datei existiert noch nicht. Sie entsteht beim ersten Verbinden.",
                    );
                }
            } else if project.database_url.starts_with("postgres:") {
                report(
                    "warnung",
                    "PostgreSQL ist optional. ferrum doctor verbindet sich nicht automatisch damit.",
                );
            } else {
                errors += 1;
                report(
                    "fehler",
                    "Die Datenbank-URL muss mit sqlite: oder postgres: beginnen.",
                );
            }
            if project.template.as_deref() == Some("fullstack") || project.embedded_assets {
                match command_version("node") {
                    Some(version) => report("ok", &format!("Node.js für den Frontend-Build: {version}")),
                    None => report(
                        "warnung",
                        "Node.js fehlt. Es wird nur zum Bauen des Admin-Frontends gebraucht, nicht zum Starten.",
                    ),
                }
                match command_version("npm") {
                    Some(_) => report("ok", "npm ist verfügbar."),
                    None => report("warnung", "npm fehlt für einen neuen Admin-Build."),
                }
            }
        }
        Err(error) => {
            errors += 1;
            report("fehler", &error.to_string());
        }
    }
    if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

pub fn check() -> ExitCode {
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
    let state = inspect_directory(&project.migrations_dir);
    if state.conflicts.is_empty() {
        println!("{} Migrationsdateien, keine Konflikte.", state.files.len());
    } else {
        for conflict in state.conflicts {
            eprintln!("{conflict}");
        }
        return ExitCode::from(1);
    }
    println!("Starte cargo check. Es wird keine Migration ausgeführt.");
    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project.root)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("cargo check konnte nicht gestartet werden: {error}");
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

fn sqlite_file_exists(url: &str) -> bool {
    let Some(path) = url.strip_prefix("sqlite://") else {
        return false;
    };
    let path = path.split('?').next().unwrap_or(path);
    if path == ":memory:" {
        return true;
    }
    Path::new(path).is_file()
}
