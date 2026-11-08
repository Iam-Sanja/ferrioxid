//! Generates an app in the project without changing existing files.

use crate::models::insert_line;
use ferrioxid_migrations::discover_project;
use std::path::{Path, PathBuf};

pub fn generate_worker() -> Result<PathBuf, String> {
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    generate_worker_into(&project.root)
}

pub(crate) fn generate_worker_into(root: &Path) -> Result<PathBuf, String> {
    let path = root.join("src/bin/worker.rs");
    write_new(&path, WORKER_SOURCE)?;
    println!(
        "ferrioxid dev does not start this worker. Register app handlers in register_jobs."
    );
    Ok(path)
}

const WORKER_SOURCE: &str = r#"//! Application worker. Does not start a web server or apply migrations.
//! ferrioxid dev does not run this file.

use ferrioxid::jobs::{JobContext, JobError, JobHandler};
use ferrioxid::serde::{Deserialize, Serialize};

fn main() {
    let root = std::env::current_dir().expect("cwd");
    let project = match ferrioxid::migrations::discover_project(&root) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{}", error.0);
            std::process::exit(2);
        }
    };
    let mut host = ferrioxid::jobs::WorkerHost::from_project(&project).with_mail(ferrioxid::mail_for(&project));
    if let Err(error) = host.register(ExampleHandler) {
        eprintln!("{} {}", error.code(), error.detail());
        std::process::exit(1);
    }
    if let Err(error) = host.run_until_ctrl_c() {
        eprintln!("{} {}", error.code(), error.detail());
        let status = if error.code().starts_with("job.migration")
            || error.code() == "job.disabled"
            || error.code() == "job.database"
        {
            2
        } else {
            1
        };
        std::process::exit(status);
    }
}

/// Example handler until the app provides its own handler.
/// Replace the registration with `AppName.register_jobs(host.registry_mut())`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExamplePayload {
    label: String,
}

struct ExampleHandler;

#[ferrioxid::async_trait::async_trait]
impl JobHandler for ExampleHandler {
    const NAME: &'static str = "app_example";
    const VERSION: u32 = 1;
    type Payload = ExamplePayload;

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        if payload.label.is_empty() {
            return Err(JobError::permanent("app.empty"));
        }
        Ok(())
    }
}
"#;

pub fn generate_app(name: &str) -> Result<PathBuf, String> {
    validate_app_name(name)?;
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    generate_into(&project.root, name)
}

pub(crate) fn generate_into(root: &Path, name: &str) -> Result<PathBuf, String> {
    validate_app_name(name)?;
    let app_dir = root.join("src/apps").join(name);
    if app_dir.exists() {
        return Err(format!(
            "App `{name}` already exists and will not be overwritten."
        ));
    }
    std::fs::create_dir_all(app_dir.join("migrations")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(app_dir.join("templates").join(name))
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(app_dir.join("static").join(name))
        .map_err(|error| error.to_string())?;
    write_new(&app_dir.join("mod.rs"), &module_source(name))?;
    write_new(&app_dir.join("urls.rs"), &urls_source(name))?;
    write_new(&app_dir.join("views.rs"), &views_source(name))?;
    write_new(&app_dir.join("admin.rs"), "pub fn attach() {}\n")?;
    write_new(
        &app_dir.join("models.rs"),
        "// Models continue to use the FerrioxidModel derive macro.\n",
    )?;
    write_new(
        &app_dir.join(format!("templates/{name}/index.html")),
        &format!("<p>{name}</p>\n"),
    )?;
    write_new(
        &app_dir.join(format!("static/{name}/{name}.css")),
        "/* app static */\n",
    )?;
    let module = root.join("src/apps/mod.rs");
    if module.exists() {
        let text = std::fs::read_to_string(&module).map_err(|error| error.to_string())?;
        let line = format!("pub mod {name};");
        if !text.contains(&line) {
            std::fs::write(&module, format!("{text}{line}\n"))
                .map_err(|error| error.to_string())?;
        }
    } else {
        write_new(&module, &format!("pub mod {name};\n"))?;
    }
    let main = root.join("src/main.rs");
    if main.is_file() {
        let text = std::fs::read_to_string(&main).map_err(|error| error.to_string())?;
        if text.contains("// ferrioxid:apps:end") {
            insert_line(
                &main,
                "// ferrioxid:apps:end",
                &format!("        .install({name}::{}App)?", pascal(name)),
            )?;
        } else {
            println!(
                "The app is not registered in main.rs yet. Add .install({name}::{pascal}App)? before startup.",
                pascal = pascal(name)
            );
        }
    }
    println!("Declare app migrations under [app.{name}] in ferrum.toml. This entry does not load Rust code.");
    Ok(app_dir)
}

fn write_new(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} already exists and will not be overwritten.",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

pub(crate) fn validate_app_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let ok = matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && (1..=32).contains(&name.len());
    if !ok
        || matches!(
            name,
            "admin" | "auth" | "api" | "static" | "healthz" | "uploads"
        )
    {
        return Err(format!("`{name}` is not a valid app name."));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("The app name cannot be a filesystem path.".to_owned());
    }
    Ok(())
}

fn pascal(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn module_source(name: &str) -> String {
    let type_name = pascal(name);
    format!(
        r#"mod admin;
mod models;
mod urls;
mod views;

use ferrioxid::apps::{{AppConfig, AppError, FerrioxidApplication, Registration, TemplateMode}};

pub struct {type_name}App;

impl FerrioxidApplication for {type_name}App {{
    fn config(&self) -> AppConfig {{
        AppConfig::new("{name}").prefix("/{name}")
    }}

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {{
        registry.routes(urls::routes())?;
        registry.templates(std::path::Path::new("src/apps/{name}/templates"), TemplateMode::Isolated)?;
        registry.static_files(std::path::Path::new("src/apps/{name}/static/{name}"))?;
        admin::attach();
        Ok(())
    }}
}}
"#
    )
}

fn urls_source(name: &str) -> String {
    format!(
        r#"use axum::routing::get;
use ferrioxid::urls::{{UrlError, UrlSet}};

pub fn routes() -> Result<UrlSet, UrlError> {{
    UrlSet::new().route("{name}.index", "/", get(crate::apps::{name}::views::index))
}}
"#
    )
}

fn views_source(name: &str) -> String {
    format!(
        r#"pub async fn index() -> axum::response::Response {{
    ferrioxid::views::json(&ferrioxid::serde_json::json!({{"app": "{name}"}}))
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{generate_into, generate_worker_into, validate_app_name};

    #[test]
    fn generate_writes_once_and_rejects_bad_names() {
        assert!(validate_app_name("Admin").is_err());
        assert!(validate_app_name("..").is_err());
        assert!(validate_app_name("admin").is_err());
        let root = std::env::temp_dir().join(format!(
            "ferrioxid-generate-app-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let created = generate_into(&root, "shop").unwrap();
        assert!(created.join("mod.rs").is_file());
        assert!(created.join("urls.rs").is_file());
        assert!(root.join("src/apps/mod.rs").is_file());
        assert!(generate_into(&root, "shop").is_err());
        std::fs::write(
            root.join("ferrum.toml"),
            "migrations_dir = \"migrations\"\n",
        )
        .unwrap();
        let worker = generate_worker_into(&root).unwrap();
        let source = std::fs::read_to_string(&worker).unwrap();
        assert!(source.contains("WorkerHost"));
        assert!(source.contains("register_jobs"));
        assert!(!source.contains("into_router"));
        assert!(generate_worker_into(&root).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
