//! Der Generator wird außerhalb des Ferrioxid-Workspaces ausgeführt.

use std::path::PathBuf;
use std::process::Command;

fn ferrioxid() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ferrioxid"));
    seal(&mut command);
    command
}

/// Kindprozesse bekommen eine eigene SQLite-Datei. Die geerbte `DATABASE_URL` des Testprozesses wird dabei ersetzt, nicht global verändert.
fn seal(command: &mut Command) -> &mut Command {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let path = std::env::temp_dir().join(format!(
        "ferrioxid-isolated-{}-{}.sqlite",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    command
        .env("DATABASE_URL", url)
        .env("CARGO_BUILD_JOBS", nested_build_jobs())
        .env_remove("FERRUM_E2E_DATABASE")
        .env_remove("FERRUM_PASSWORD")
}

/// Deckt verschachtelte Cargo-Builds ab. rustc 1.98.1 ist hier bei einem unbegrenzten Release-Build mit 0xc0000005 und 0xc0000409 abgestürzt. Die Ursache ist nicht aufgeklärt. Mit höchstens zwei Jobs bestanden der begrenzte und der anschließende uneingeschränkte Workspace-Lauf. Ein bereits kleineres Limit bleibt erhalten.
fn nested_build_jobs() -> String {
    let configured = std::env::var("CARGO_BUILD_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2);
    configured.clamp(1, 2).to_string()
}

fn cleanup_isolated_databases() {
    let prefix = format!("ferrioxid-isolated-{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn rejects_names_and_existing_directories() {
    let root = std::env::temp_dir().join(format!("ferrioxid-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let bad = ferrioxid()
        .current_dir(&root)
        .args(["new", "../escape", "--template", "api"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!root.join("escape").exists());
    let created = ferrioxid()
        .current_dir(&root)
        .args([
            "new",
            "shop",
            "--template",
            "api",
            "--ferrioxid-path",
            repository().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let shop = root.join("shop");
    std::fs::remove_file(shop.join("src/lib.rs")).unwrap();
    let broken = ferrioxid()
        .current_dir(&shop)
        .arg("doctor")
        .output()
        .unwrap();
    assert!(
        !broken.status.success(),
        "{}",
        String::from_utf8_lossy(&broken.stdout)
    );
    let again = ferrioxid()
        .current_dir(&root)
        .args([
            "new",
            "shop",
            "--template",
            "api",
            "--ferrioxid-path",
            repository().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!again.status.success());
    let _ = std::fs::remove_dir_all(&root);
    cleanup_isolated_databases();
}

#[test]
fn generated_api_and_fullstack_projects_compile() {
    let root = std::env::temp_dir().join(format!("ferrioxid-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    // Ein Target nur für diesen Prozess. Die Builds laufen nacheinander, nicht parallel.
    let target = root.join("shared-target");
    let repo = repository();
    assert!(create(&root, "demo_api", "api", &repo).status.success());
    assert!(create(&root, "demo_full", "fullstack", &repo)
        .status
        .success());
    let api = root.join("demo_api");
    assert!(api.join("migrations/0001_initial.json").is_file());
    assert!(api.join("templates/index.html").is_file());
    assert!(api.join("templates/errors/404.html").is_file());
    assert!(api.join("ferrum.toml").is_file());
    assert!(!std::fs::read_to_string(api.join("ferrum.toml"))
        .unwrap()
        .contains("0.0.0.0"));
    let doctor = ferrioxid()
        .current_dir(&api)
        .arg("doctor")
        .output()
        .unwrap();
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stdout)
    );
    let before = std::fs::read(api.join("migrations/0001_initial.json")).unwrap();
    let checked = cargo(&api, &target, &["check"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let project_check = ferrioxid()
        .current_dir(&api)
        .env("CARGO_TARGET_DIR", &target)
        .arg("check")
        .output()
        .unwrap();
    assert!(
        project_check.status.success(),
        "{}",
        String::from_utf8_lossy(&project_check.stderr)
    );
    assert_eq!(
        std::fs::read(api.join("migrations/0001_initial.json")).unwrap(),
        before
    );
    let generated = ferrioxid()
        .current_dir(&api)
        .args(["generate", "model", "Customer"])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let duplicate = ferrioxid()
        .current_dir(&api)
        .args(["generate", "model", "Customer"])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    let rechecked = cargo(&api, &target, &["check"]);
    assert!(
        rechecked.status.success(),
        "{}",
        String::from_utf8_lossy(&rechecked.stderr)
    );
    let full = cargo(&root.join("demo_full"), &target, &["check"]);
    assert!(
        full.status.success(),
        "{}",
        String::from_utf8_lossy(&full.stderr)
    );
    let dev = ferrioxid()
        .current_dir(&api)
        .env("CARGO_TARGET_DIR", &target)
        .env("FERRUM_DEV_EXIT_MS", "800")
        .env("BIND_ADDRESS", "127.0.0.1:18761")
        .arg("dev")
        .output()
        .unwrap();
    let dev_out = format!(
        "{}{}",
        String::from_utf8_lossy(&dev.stdout),
        String::from_utf8_lossy(&dev.stderr)
    );
    assert!(dev.status.success(), "{dev_out}");
    assert!(dev_out.contains("lauscht"), "{dev_out}");
    for (directory, label) in [(&api, "api"), (&root.join("demo_full"), "fullstack")] {
        let built = ferrioxid()
            .current_dir(directory)
            .env("CARGO_TARGET_DIR", &target)
            .arg("build")
            .output()
            .unwrap();
        let build_out = format!(
            "{}{}",
            String::from_utf8_lossy(&built.stdout),
            String::from_utf8_lossy(&built.stderr)
        );
        assert!(built.status.success(), "{label}: {build_out}");
        assert!(build_out.contains("target"), "{label}: {build_out}");
    }
    let _ = std::fs::remove_dir_all(&root);
    cleanup_isolated_databases();
}

fn create(
    root: &std::path::Path,
    name: &str,
    template: &str,
    repo: &std::path::Path,
) -> std::process::Output {
    ferrioxid()
        .current_dir(root)
        .args(["new", name, "--template", template, "--ferrioxid-path"])
        .arg(repo)
        .output()
        .unwrap()
}

fn cargo(
    project: &std::path::Path,
    target: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    let mut command = Command::new("cargo");
    seal(&mut command);
    command
        .args(args)
        .current_dir(project)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap()
}
