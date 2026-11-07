//! Entwicklungsserver und Release-Build. Der Admin steckt im Rust-Binary.

use ferrum_migrations::discover_project;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

pub fn dev() -> ExitCode {
    let project = match current_project() {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let notice = migration_notice(&project);
    if !notice.is_empty() {
        println!("{notice}");
    }
    let package = match package_name(&project.root) {
        Ok(name) => name,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let (control_tx, control_rx) = std::sync::mpsc::channel();
    let watch_tx = control_tx.clone();
    let mut watcher = match RecommendedWatcher::new(
        move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                if event.paths.iter().any(|path| relevant(path)) {
                    let _ = watch_tx.send(Signal::Changed);
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(error) => {
            eprintln!("Dateiwächter konnte nicht starten: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = watcher.watch(&project.root, RecursiveMode::Recursive) {
        eprintln!("Projektverzeichnis kann nicht beobachtet werden: {error}");
        return ExitCode::from(1);
    }
    let stop_tx = control_tx.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        let _ = stop_tx.send(Signal::Stop);
    }) {
        eprintln!("Abbruchsignal konnte nicht gesetzt werden: {error}");
        return ExitCode::from(1);
    }
    println!(
        "{} {package}. {}",
        crate::text::line("dev_started"),
        crate::text::line("dev_stop_hint")
    );
    let exit_after = std::env::var("FERRUM_DEV_EXIT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis);
    let mut child = None;
    let mut dirty = true;
    let mut last_restart = Instant::now() - Duration::from_secs(2);
    loop {
        if dirty && last_restart.elapsed() >= Duration::from_millis(400) {
            stop_child(&mut child);
            match start_server(&project.root, &package) {
                Ok(running) => child = Some(running),
                Err(error) => eprintln!("{error}"),
            }
            dirty = false;
            last_restart = Instant::now();
            if let Some(limit) = exit_after {
                if child.is_none() {
                    return ExitCode::from(1);
                }
                std::thread::sleep(limit);
                stop_child(&mut child);
                println!("{}", crate::text::line("dev_test_stopped"));
                return ExitCode::SUCCESS;
            }
        }
        if let Some(running) = child.as_mut() {
            if let Ok(Some(status)) = running.try_wait() {
                eprintln!("Der Server ist mit Status {status} beendet. Ferrum startet erst nach einer Dateiänderung neu.");
                child = None;
            }
        }
        match control_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Signal::Stop) => {
                stop_child(&mut child);
                println!("{}", crate::text::line("dev_stopped"));
                return ExitCode::SUCCESS;
            }
            Ok(Signal::Changed) => dirty = true,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                stop_child(&mut child);
                return ExitCode::from(1);
            }
        }
    }
}

pub fn build() -> ExitCode {
    let project = match current_project() {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&project.root)
        .status();
    match status {
        Ok(status) if status.success() => {
            let output = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| project.root.join("target"));
            println!(
                "Release-Build liegt in {}",
                output.join("release").display()
            );
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("cargo build konnte nicht gestartet werden: {error}");
            ExitCode::from(1)
        }
    }
}

enum Signal {
    Changed,
    Stop,
}

fn migration_notice(project: &ferrum_migrations::ProjectConfig) -> String {
    let locale = crate::text::locale();
    let url = project.resolved_database_url();
    let readonly = match ferrum_migrations::readonly_database_url(&url) {
        Ok(url) => url,
        Err(error) => return ferrum_migrations::dev_notice(&locale, &Err(error)),
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ferrum_migrations::dev_notice(
                &locale,
                &Err(ferrum_migrations::MigrationError::new(format!(
                    "migration.database.unavailable: {error}"
                ))),
            );
        }
    };
    let result = runtime.block_on(async {
        let db = ferrum_db::connect(&readonly).await.map_err(|error| {
            ferrum_migrations::MigrationError::new(format!(
                "migration.database.unavailable: {error}"
            ))
        })?;
        ferrum_migrations::inspect_migrations(
            &db,
            &ferrum_auth::migrations_dir(),
            &project.migrations_dir,
            None,
        )
        .await
    });
    ferrum_migrations::dev_notice(&locale, &result)
}

fn current_project() -> Result<ferrum_migrations::ProjectConfig, String> {
    let current = std::env::current_dir().map_err(|error| error.to_string())?;
    discover_project(&current).map_err(|error| error.to_string())
}

fn start_server(root: &Path, package: &str) -> Result<Child, String> {
    let status = Command::new("cargo")
        .args(["build", "--bin", package])
        .current_dir(root)
        .status()
        .map_err(|error| format!("cargo build konnte nicht gestartet werden: {error}"))?;
    if !status.success() {
        return Err("Der Build ist fehlgeschlagen. Der alte Server bleibt beendet.".to_owned());
    }
    let binary = binary_path(root, package);
    Command::new(&binary)
        .current_dir(root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("{} konnte nicht starten: {error}", binary.display()))
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(running) = child.as_mut() {
        let _ = running.kill();
        let _ = running.wait();
    }
    *child = None;
}

fn binary_path(root: &Path, package: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{package}.exe")
    } else {
        package.to_owned()
    };
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    base.join("debug").join(file)
}

fn package_name(root: &Path) -> Result<String, String> {
    let text =
        std::fs::read_to_string(root.join("Cargo.toml")).map_err(|error| error.to_string())?;
    for line in text.lines() {
        if let Some(value) = line.trim().strip_prefix("name = ") {
            return Ok(value.trim().trim_matches('"').to_owned());
        }
    }
    Err("Cargo.toml enthält keinen Paketnamen.".to_owned())
}

pub fn relevant(path: &Path) -> bool {
    if path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("target" | "node_modules" | ".git" | "dist" | "embedded-admin")
        )
    }) {
        return false;
    }
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs" | "toml" | "json")
    )
}

#[cfg(test)]
mod tests {
    use super::relevant;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn watch_ignores_build_directories() {
        assert!(!relevant(Path::new("target/debug/app")));
        assert!(!relevant(Path::new("node_modules/react/index.js")));
        assert!(relevant(Path::new("src/main.rs")));
    }

    #[test]
    fn child_process_can_be_stopped() {
        let mut child = if cfg!(windows) {
            Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .spawn()
                .expect("ping")
        } else {
            Command::new("sleep").arg("30").spawn().expect("sleep")
        };
        child.kill().expect("kill");
        let status = child.wait().expect("wait");
        let _ = status;
    }
}
