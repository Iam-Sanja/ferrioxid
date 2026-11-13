//! The generator runs outside the Fexu workspace.

use std::path::PathBuf;
use std::process::Command;

fn fexu() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fexu"));
    seal(&mut command);
    command
}

/// Child processes use their own SQLite file. Their inherited `DATABASE_URL` is overridden per process, not globally.
fn seal(command: &mut Command) -> &mut Command {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let path = std::env::temp_dir().join(format!(
        "fexu-isolated-{}-{}.sqlite",
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
        .env_remove("FEXU_E2E_DATABASE")
        .env_remove("FEXU_PASSWORD")
}

/// Limits nested Cargo builds. rustc 1.98.1 previously crashed during unrestricted release builds (0xc0000005 and 0xc0000409). The root cause is unresolved. Builds passed with two jobs; preserve any lower caller-specified limit.
fn nested_build_jobs() -> String {
    let configured = std::env::var("CARGO_BUILD_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2);
    configured.clamp(1, 2).to_string()
}

fn cleanup_isolated_databases() {
    let prefix = format!("fexu-isolated-{}-", std::process::id());
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

fn success(output: std::process::Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{text}");
    text
}

#[test]
fn rejects_names_existing_directories_and_legacy_commands() {
    let root = std::env::temp_dir().join(format!("fexu-name-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for args in [
        vec!["new", "project", "../escape"],
        vec!["new", "project", "type"],
        vec!["new", "Shop", "--template", "api"],
        vec!["generate", "app", "shop"],
    ] {
        let output = fexu().current_dir(&root).args(&args).output().unwrap();
        assert!(!output.status.success(), "{args:?}");
        if args[1] == "Shop" || args[1] == "app" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("new app"));
        }
    }
    assert!(!root.join("Shop").exists());
    success(create(&root, "Shop", "api", &repository(), "apps"));
    let shop = root.join("Shop");
    std::fs::write(shop.join("sentinel"), "keep").unwrap();
    assert!(!create(&root, "Shop", "api", &repository(), "apps")
        .status
        .success());
    assert_eq!(
        std::fs::read_to_string(shop.join("sentinel")).unwrap(),
        "keep"
    );
    std::fs::remove_file(shop.join("src/lib.rs")).unwrap();
    assert!(!fexu()
        .current_dir(&shop)
        .arg("doctor")
        .output()
        .unwrap()
        .status
        .success());
    std::fs::remove_dir_all(root).unwrap();
    cleanup_isolated_databases();
}

#[test]
fn generated_api_and_fullstack_projects_compile() {
    let root = std::env::temp_dir().join(format!("fexu-gen-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("generated-projects");
    for (template, placement) in [("api", "apps"), ("fullstack", ".")] {
        let parent = root.join(template);
        std::fs::create_dir_all(&parent).unwrap();
        success(create(&parent, "Shop", template, &repository(), placement));
        let project = parent.join("Shop");
        assert_current_framework_names(&project);
        assert!(!project.join("src/models").exists());
        assert!(!project.join("migrations/0001_initial.json").exists());
        assert!(std::fs::read_to_string(project.join("Cargo.toml"))
            .unwrap()
            .contains("name = \"shop\""));
        success(fexu().current_dir(&project).arg("doctor").output().unwrap());
        let database = format!(
            "sqlite://{}?mode=rwc",
            project
                .join("integration.sqlite")
                .to_string_lossy()
                .replace('\\', "/")
        );
        let cli = |args: &[&str]| {
            fexu()
                .current_dir(&project)
                .env("CARGO_TARGET_DIR", &target)
                .env("DATABASE_URL", &database)
                .args(args)
                .output()
                .unwrap()
        };
        success(cli(&["migrate", "up"]));
        success(cli(&["migrate", "up"]));
        std::fs::write(project.join("tests/database.rs"), DATABASE_TEST).unwrap();
        success(cargo_with_database(
            &project,
            &target,
            &database,
            &["test"],
            false,
        ));
        success(cli(&["check"]));
        success(cli(&["new", "app", "produkte"]));
        success(cli(&["new", "app", "berichte", "--templates", "shared"]));
        let app = project.join(if placement == "." {
            "produkte"
        } else {
            "apps/produkte"
        });
        assert!(app.join("models/mod.rs").is_file());
        assert!(app.join("templates/produkte/index.html").is_file());
        assert!(project.join("templates/berichte/index.html").is_file());
        assert!(!cli(&["new", "app", "produkte"]).status.success());
        assert!(!cli(&["generate", "model", "Customer"]).status.success());
        success(cli(&[
            "generate",
            "model",
            "Customer",
            "--app",
            "produkte",
            "--timestamps",
        ]));
        assert!(
            !cli(&["generate", "model", "Customer", "--app", "produkte"])
                .status
                .success()
        );
        success(cli(&["migrate", "make", "initial", "--app", "produkte"]));
        assert!(app.join("migrations/0001_initial.json").is_file());
        assert!(!project.join("migrations/0001_initial.json").exists());
        assert!(success(cli(&["migrate", "up"])).contains("produkte:0001_initial"));
        success(cli(&["migrate", "up"]));
        success(cargo_with_database(
            &project,
            &target,
            &database,
            &["test"],
            true,
        ));
        success(cargo(
            &project,
            &target,
            &["clippy", "--all-targets", "--", "-D", "warnings"],
        ));
        std::fs::create_dir_all(project.join("templates/produkte")).unwrap();
        std::fs::write(
            project.join("templates/produkte/index.html"),
            "<h1>project override</h1>",
        )
        .unwrap();
        serve_and_check(&project, &target, &database, template == "fullstack");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        drop(listener);
        let dev = fexu()
            .current_dir(&project)
            .env("CARGO_TARGET_DIR", &target)
            .env("DATABASE_URL", &database)
            .env("FEXU_DEV_EXIT_MS", "800")
            .env("BIND_ADDRESS", &address)
            .arg("dev")
            .output()
            .unwrap();
        assert!(success(dev).contains("lauscht"));
        assert!(success(cli(&["build"])).contains("target"));
    }
    std::fs::remove_dir_all(root).unwrap();
    cleanup_isolated_databases();
}

const DATABASE_TEST: &str = r#"use fexu::sea_orm::{ConnectionTrait, DbBackend, Statement};
#[tokio::test]
async fn only_framework_and_explicit_business_tables_exist() {
    let db = fexu::db::connect(&std::env::var("DATABASE_URL").unwrap()).await.unwrap();
    let version = db.query_one(Statement::from_string(DbBackend::Sqlite,
        "SELECT sqlite_version() AS version".to_owned())).await.unwrap().unwrap();
    let version: String = version.try_get("", "version").unwrap();
    let parts: Vec<u32> = version.split('.').map(|part| part.parse().unwrap()).collect();
    assert!(parts.as_slice() >= [3, 54, 1].as_slice(), "Vulnerable SQLite in generated project: {version}");
    let rows = db.query_all(Statement::from_string(DbBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'".to_owned())).await.unwrap();
    let names: Vec<String> = rows.iter().map(|row| row.try_get("", "name").unwrap()).collect();
    assert!(names.iter().any(|name| name == "_fexu_users"));
    assert!(names.iter().any(|name| name == "_fexu_jobs"));
    let domain: Vec<_> = names.iter().filter(|name| !name.starts_with("_fexu_")).map(String::as_str).collect();
    if std::env::var_os("EXPECT_CUSTOMERS").is_some() { assert_eq!(domain, ["customers"]); }
    else { assert!(domain.is_empty(), "{domain:?}"); }
    let count = db.query_one(Statement::from_string(DbBackend::Sqlite,
        "SELECT COUNT(*) AS count FROM _fexu_users".to_owned())).await.unwrap().unwrap();
    assert_eq!(count.try_get::<i64>("", "count").unwrap(), 0);
}
"#;

fn cargo_with_database(
    project: &std::path::Path,
    target: &std::path::Path,
    database: &str,
    args: &[&str],
    customers: bool,
) -> std::process::Output {
    let mut command = Command::new("cargo");
    seal(&mut command);
    command
        .current_dir(project)
        .env("CARGO_TARGET_DIR", target)
        .env("DATABASE_URL", database)
        .env_remove("EXPECT_CUSTOMERS")
        .args(args);
    if customers {
        command.env("EXPECT_CUSTOMERS", "1");
    }
    command.output().unwrap()
}

struct Server(std::process::Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn serve_and_check(
    project: &std::path::Path,
    target: &std::path::Path,
    database: &str,
    admin: bool,
) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut command = Command::new(
        target
            .join("debug")
            .join(format!("shop{}", std::env::consts::EXE_SUFFIX)),
    );
    seal(&mut command);
    let mut server = Server(
        command
            .current_dir(project)
            .env("DATABASE_URL", database)
            .env("BIND_ADDRESS", address.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::net::TcpStream::connect(address).is_err() {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "generated server exited before listening"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "generated server did not listen"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    for (path, status, body) in [
        ("/", "200", "Business apps are added explicitly"),
        ("/api/v1/ping", "200", "\"ok\":true"),
        ("/produkte", "200", "project override"),
        ("/berichte", "200", "berichte"),
        ("/static/produkte/produkte.css", "200", "App styles"),
        ("/api/v1/customers", "401", ""),
        ("/admin", if admin { "303" } else { "404" }, ""),
    ] {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "{path}: {response}"
        );
        assert!(response.contains(body), "{path}: {response}");
    }
}

fn create(
    root: &std::path::Path,
    name: &str,
    template: &str,
    repo: &std::path::Path,
    placement: &str,
) -> std::process::Output {
    fexu()
        .current_dir(root)
        .args([
            "new",
            "project",
            name,
            "--apps-dir",
            placement,
            "--template",
            template,
            "--fexu-path",
        ])
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

// The dependency path is removed before the check. Retired names are matched as
// bytes so a later rename pass does not rewrite the needles.
fn retired_brand(value: &str) -> bool {
    let bytes = value.to_ascii_lowercase().into_bytes();
    let older = [102, 101, 114, 114, 117, 109];
    let previous = [102, 101, 114, 114, 105, 111, 120, 105, 100];
    bytes.windows(older.len()).any(|window| window == older)
        || bytes
            .windows(previous.len())
            .any(|window| window == previous)
}

// Absolute paths point at this checkout. Its directory name is not a generated name.
fn assert_current_framework_names(project: &std::path::Path) {
    let checkout = repository().canonicalize().unwrap();
    let checkout = checkout.to_string_lossy();
    let checkout = checkout
        .strip_prefix(r"\\?\")
        .unwrap_or(&checkout)
        .replace('\\', "/");
    let mut pending = vec![project.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(project).unwrap();
            assert!(!retired_brand(&relative.to_string_lossy()));
            if path.is_dir() {
                pending.push(path);
            } else {
                let content = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace(&checkout, "<framework>");
                assert!(!retired_brand(&content), "{}", relative.display());
            }
        }
    }
    assert!(project.join("fexu.toml").is_file());
    assert!(project.join(".fexu/project.json").is_file());
    assert!(!project.join(".fexu/csrf.key").exists());
    let ignored = std::fs::read_to_string(project.join(".gitignore")).unwrap();
    for pattern in [
        ".env\n",
        ".env.*\n",
        "!.env.example\n",
        ".fexu/csrf.key\n",
        ".fexu/mfa.key\n",
    ] {
        assert!(
            ignored.contains(pattern),
            "missing secret protection: {pattern}"
        );
    }
    let apps = std::fs::read_to_string(project.join("src/apps.rs")).unwrap();
    assert!(apps.contains("// fexu:apps:end"));
}
