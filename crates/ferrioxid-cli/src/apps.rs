//! Generates an app in the project without changing existing files.

use crate::models::insert_text;
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
    println!("ferrioxid dev does not start this worker. Register app handlers in register_jobs.");
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

pub fn generate_app(name: &str, templates: &str) -> Result<PathBuf, String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    generate_into(&root, name, templates)
}

pub(crate) fn generate_into(root: &Path, name: &str, templates: &str) -> Result<PathBuf, String> {
    validate_app_name(name)?;
    if !matches!(templates, "local" | "shared") {
        return Err("Template placement must be local or shared.".to_owned());
    }
    let project = discover_project(root).map_err(|error| error.0)?;
    if project.layout_version != 2 {
        return Err("new app requires a layout-2 project. Existing projects are not restructured. See docs/project-generator.md for manual migration.".to_owned());
    }
    if project.apps.iter().any(|app| app.name == name) {
        return Err(format!("App {name} is already declared."));
    }
    let relative = if project.apps_dir == "." {
        name.to_owned()
    } else {
        format!("apps/{name}")
    };
    let app_dir = project.root.join(&relative);
    if app_dir.exists() || app_dir.symlink_metadata().is_ok() {
        return Err(format!(
            "App {name} already exists and will not be overwritten."
        ));
    }
    ensure_project_directory(&project.root, app_dir.parent().expect("app parent"))?;
    let template_path = if templates == "shared" {
        project
            .templates_directory()
            .join(format!("{name}/index.html"))
    } else {
        app_dir.join(format!("templates/{name}/index.html"))
    };
    if template_path.exists() || template_path.symlink_metadata().is_ok() {
        return Err(format!(
            "{} already exists and will not be overwritten.",
            template_path.display()
        ));
    }
    ensure_project_directory(
        &project.root,
        template_path.parent().expect("template parent"),
    )?;
    let registry_path = project.root.join("src/apps.rs");
    ensure_project_directory(&project.root, &registry_path)?;
    let mut registry =
        std::fs::read_to_string(&registry_path).map_err(|error| error.to_string())?;
    registry = insert_text(
        &registry,
        "// ferrioxid:app-modules:end",
        &format!("#[path = \"../{relative}/mod.rs\"]\npub mod {name};"),
    )?;
    registry = insert_text(
        &registry,
        "// ferrioxid:apps:end",
        &format!("        .install({name}::{}App)?", pascal(name)),
    )?;
    registry = insert_text(&registry, "// ferrioxid:app-schema:end",
        &format!("        Some(\"{name}\") => Ok(ferrioxid::migrations::SchemaSnapshot::from_registered(&{name}::models::schema())?),"))?;
    let config_path = project.root.join("ferrioxid.toml");
    let config = std::fs::read_to_string(&config_path).map_err(|error| error.to_string())?;
    let config = format!("{config}\n[app.{name}]\nmigrations = \"{relative}/migrations\"\n");
    for directory in ["models", "migrations", "templates", "static"] {
        std::fs::create_dir_all(app_dir.join(directory)).map_err(|error| error.to_string())?;
    }
    write_new(&app_dir.join("mod.rs"), &module_source(name, &relative))?;
    write_new(&app_dir.join("urls.rs"), &urls_source(name))?;
    write_new(&app_dir.join("views.rs"), &views_source(name))?;
    write_new(&app_dir.join("admin.rs"), REGISTER_SOURCE)?;
    write_new(&app_dir.join("models/mod.rs"), MODELS_SOURCE)?;
    write_new(&app_dir.join("migrations/.gitkeep"), "")?;
    write_new(
        &app_dir.join(format!("static/{name}/{name}.css")),
        "/* App styles. */\n",
    )?;
    write_new(
        &template_path,
        &format!(
            "{{% extends \"base.html\" %}}{{% block content %}}<h1>{name}</h1>{{% endblock %}}\n"
        ),
    )?;
    // Only explicit registration markers and the app declaration are updated.
    std::fs::write(registry_path, registry).map_err(|error| error.to_string())?;
    std::fs::write(config_path, config).map_err(|error| error.to_string())?;
    Ok(app_dir)
}

pub(crate) fn ensure_project_directory(root: &Path, directory: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let mut ancestor = directory;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or("Missing parent directory.")?;
    }
    let ancestor = ancestor.canonicalize().map_err(|error| error.to_string())?;
    if !ancestor.starts_with(&root) {
        return Err("Generated files must remain inside the project directory.".to_owned());
    }
    Ok(())
}

fn write_new(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::scaffold::write(path, contents)
}

pub(crate) fn validate_app_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let ok = matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && (1..=32).contains(&name.len());
    if !ok
        || crate::scaffold::reserved_identifier(name)
        || matches!(
            name,
            "admin"
                | "auth"
                | "api"
                | "static"
                | "healthz"
                | "uploads"
                | "apps"
                | "project"
                | "src"
                | "templates"
                | "migrations"
                | "target"
                | "tests"
                | "media"
                | "ferrioxid"
                | "cargo"
        )
    {
        return Err(format!("{name} is not a valid app name."));
    }
    Ok(())
}

fn pascal(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part[..1].to_ascii_uppercase() + &part[1..])
        .collect()
}

fn module_source(name: &str, relative: &str) -> String {
    let type_name = pascal(name);
    format!(
        r#"mod admin;
pub mod models;
mod urls;
mod views;

use ferrioxid::apps::{{AppConfig, AppError, FerrioxidApplication, Registration, TemplateMode}};

pub struct {type_name}App;

impl FerrioxidApplication for {type_name}App {{
    fn config(&self) -> AppConfig {{
        AppConfig::new("{name}").prefix("/{name}")
    }}

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {{
        let root = registry.project()?.root.clone();
        models::register(registry)?;
        admin::register(registry)?;
        registry.routes(urls::routes()?)?;
        registry.migrations("{relative}/migrations")?;
        registry.templates(root.join("{relative}/templates"), TemplateMode::ProjectOverride)?;
        registry.static_files(root.join("{relative}/static/{name}"))?;
        Ok(())
    }}
}}
"#
    )
}

const REGISTER_SOURCE: &str = r#"pub fn register(_registry: &mut ferrioxid::apps::Registration<'_>) -> Result<(), ferrioxid::apps::AppError> {
    // ferrioxid:admin:end
    Ok(())
}
"#;

const MODELS_SOURCE: &str = r#"// ferrioxid:models:end

pub fn register(_registry: &mut ferrioxid::apps::Registration<'_>) -> Result<(), ferrioxid::apps::AppError> {
    // ferrioxid:registry:end
    Ok(())
}

pub fn schema() -> Vec<ferrioxid::migrations::RegisteredModel> {
    vec![
        // ferrioxid:schema:end
    ]
}
"#;

fn urls_source(name: &str) -> String {
    format!(
        r#"use axum::routing::get;
use ferrioxid::urls::{{UrlError, UrlSet}};

pub fn routes() -> Result<UrlSet, UrlError> {{
    UrlSet::new().route("{name}.index", "/", get(super::views::index))
}}
"#
    )
}

fn views_source(name: &str) -> String {
    format!(
        r#"use axum::{{Extension, http::HeaderMap, response::{{Html, IntoResponse, Response}}}};
use std::sync::Arc;

pub async fn index(
    Extension(catalog): Extension<Arc<ferrioxid::apps::TemplateCatalog>>,
    Extension(settings): Extension<Arc<ferrioxid::migrations::ProjectConfig>>,
    headers: HeaderMap,
) -> Response {{
    let context = ferrioxid::pages::home_context(&settings);
    match catalog.render("{name}/index.html", &context) {{
        Ok(html) => Html(html).into_response(),
        Err(error) => ferrioxid::views::fail(&headers, None, axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "page.render", &error.to_string(), settings.resolved_debug().unwrap_or(false)),
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ferrioxid-app-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/apps.rs"), crate::scaffold::APPS_REGISTRY).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n[project]\nlayout_version = 2\napps_dir = \"apps\"\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn app_registration_is_explicit_and_preflighted() {
        for invalid in ["Admin", "..", "admin", "type", "self", "con", "apps", "src"] {
            assert!(validate_app_name(invalid).is_err());
        }
        assert_eq!(pascal("order_items"), "OrderItems");
        let root = fixture();
        let created = generate_into(&root, "shop", "local").unwrap();
        assert!(created.join("models/mod.rs").is_file());
        assert!(created.join("migrations/.gitkeep").is_file());
        let config = discover_project(&root).unwrap();
        assert_eq!(config.apps[0].name, "shop");
        assert!(generate_into(&root, "shop", "local").is_err());
        let before = std::fs::read(root.join("ferrioxid.toml")).unwrap();
        std::fs::write(root.join("src/apps.rs"), "// no markers\n").unwrap();
        assert!(generate_into(&root, "other", "local").is_err());
        assert!(!root.join("apps/other").exists());
        assert_eq!(std::fs::read(root.join("ferrioxid.toml")).unwrap(), before);
        let worker = generate_worker_into(&root).unwrap();
        assert!(std::fs::read_to_string(worker)
            .unwrap()
            .contains("WorkerHost"));
        assert!(generate_worker_into(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn root_apps_and_shared_templates_preserve_existing_files() {
        let root = fixture();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n[project]\nlayout_version = 2\napps_dir = \".\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("templates/shop")).unwrap();
        std::fs::write(root.join("templates/shop/index.html"), "keep").unwrap();
        assert!(generate_into(&root, "shop", "shared").is_err());
        assert!(!root.join("shop").exists());
        std::fs::remove_file(root.join("templates/shop/index.html")).unwrap();
        assert_eq!(
            generate_into(&root, "shop", "shared").unwrap(),
            root.join("shop")
        );
        assert!(root.join("templates/shop/index.html").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}
