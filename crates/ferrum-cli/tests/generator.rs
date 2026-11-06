//! Der Generator wird außerhalb des Ferrum-Workspaces ausgeführt.

use std::path::PathBuf;
use std::process::Command;

fn ferrum() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ferrum"))
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn rejects_names_and_existing_directories() {
    let root = std::env::temp_dir().join(format!("ferrum-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let bad = ferrum()
        .current_dir(&root)
        .args(["new", "../escape", "--template", "api"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!root.join("escape").exists());
    let created = ferrum()
        .current_dir(&root)
        .args([
            "new",
            "shop",
            "--template",
            "api",
            "--ferrum-path",
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
    let broken = ferrum().current_dir(&shop).arg("doctor").output().unwrap();
    assert!(
        !broken.status.success(),
        "{}",
        String::from_utf8_lossy(&broken.stdout)
    );
    let again = ferrum()
        .current_dir(&root)
        .args([
            "new",
            "shop",
            "--template",
            "api",
            "--ferrum-path",
            repository().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!again.status.success());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn generated_api_and_fullstack_projects_compile() {
    let root = std::env::temp_dir().join(format!("ferrum-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("shared-target");
    let repo = repository();
    assert!(create(&root, "demo_api", "api", &repo).status.success());
    assert!(create(&root, "demo_full", "fullstack", &repo)
        .status
        .success());
    let api = root.join("demo_api");
    assert!(api.join("migrations/0001_initial.json").is_file());
    assert!(api.join("ferrum.toml").is_file());
    assert!(!std::fs::read_to_string(api.join("ferrum.toml"))
        .unwrap()
        .contains("0.0.0.0"));
    let doctor = ferrum().current_dir(&api).arg("doctor").output().unwrap();
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
    let project_check = ferrum()
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
    let generated = ferrum()
        .current_dir(&api)
        .args(["generate", "model", "Customer"])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let duplicate = ferrum()
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
    let dev = ferrum()
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
        let built = ferrum()
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
}

fn create(
    root: &std::path::Path,
    name: &str,
    template: &str,
    repo: &std::path::Path,
) -> std::process::Output {
    ferrum()
        .current_dir(root)
        .args(["new", name, "--template", template, "--ferrum-path"])
        .arg(repo)
        .output()
        .unwrap()
}

fn cargo(
    project: &std::path::Path,
    target: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    Command::new("cargo")
        .args(args)
        .current_dir(project)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap()
}
