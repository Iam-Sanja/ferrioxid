//! Generates empty standalone projects. Business models belong to explicitly created apps.

use ferrioxid_migrations::{manifest_json, sha256_hex, ProjectManifest, TrackedFile};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn new_project(
    name: &str,
    template: &str,
    apps_dir: &str,
    ferrioxid_path: Option<&Path>,
) -> Result<PathBuf, String> {
    validate_project_name(name)?;
    if !matches!(apps_dir, "." | "apps") {
        return Err("App placement must be . or apps.".to_owned());
    }
    let template = parse_template(template)?;
    let ferrioxid = resolve_ferrioxid_path(ferrioxid_path)?;
    // Cargo only honors patches in the consuming project/workspace root.
    let sqlite_driver = ferrioxid
        .join("../../vendor/sqlx-sqlite")
        .canonicalize()
        .map_err(|error| format!("The Ferrioxid SQLite security patch is missing: {error}"))?;
    let sqlite_driver =
        serde_json::to_string(&cargo_path(&sqlite_driver)).map_err(|error| error.to_string())?;
    let destination = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(name);
    std::fs::create_dir(&destination).map_err(|error| {
        format!(
            "{} cannot be created: {error}. Existing directories are never overwritten.",
            destination.display()
        )
    })?;
    for directory in [
        "src/project",
        "src/bin",
        "migrations",
        "tests",
        "templates/errors",
        "static/css",
        "static/js",
        "media",
    ] {
        std::fs::create_dir_all(destination.join(directory)).map_err(|error| error.to_string())?;
    }
    let package = crate_name(name);
    let dependency =
        serde_json::to_string(&cargo_path(&ferrioxid)).map_err(|error| error.to_string())?;
    let features = if matches!(template, Template::Fullstack) {
        ", features = [\"admin-ui\"]"
    } else {
        ""
    };
    write(
        &destination.join("Cargo.toml"),
        &cargo_toml(&package, &dependency, features, &sqlite_driver),
    )?;
    write(
        &destination.join("ferrioxid.toml"),
        &manifest(name, template, apps_dir),
    )?;
    write(&destination.join(".gitignore"), "target/\n.env\n.env.*\n!.env.example\n.ferrioxid/csrf.key\n*.sqlite3\n*.sqlite3-*\nmedia/*\n!media/.gitkeep\n")?;
    write(&destination.join(".env.example"), env_example())?;
    write(&destination.join("README.md"), &readme(name))?;
    write(
        &destination.join("src/lib.rs"),
        "pub mod apps;\npub mod project;\n",
    )?;
    write(&destination.join("src/main.rs"), &format!("#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {{\n    {package}::project::run().await\n}}\n"))?;
    write(
        &destination.join("src/bin/ferrioxid-manage.rs"),
        &format!("#[tokio::main]\nasync fn main() {{\n    let code = {package}::project::management::execute().await;\n    std::process::exit(i32::from(code));\n}}\n"),
    )?;
    write(
        &destination.join("src/project/management.rs"),
        MANAGEMENT_SOURCE,
    )?;
    write(&destination.join("src/apps.rs"), APPS_REGISTRY)?;
    write(
        &destination.join("src/project/mod.rs"),
        &project_source(template),
    )?;
    write(&destination.join("src/project/settings.rs"), settings_rs())?;
    write(&destination.join("src/project/urls.rs"), URLS_SOURCE)?;
    write(&destination.join("migrations/.gitkeep"), "")?;
    write(
        &destination.join("static/css/app.css"),
        "body { font-family: sans-serif; max-width: 60rem; margin: 2rem auto; padding: 1rem; }\n",
    )?;
    write(
        &destination.join("static/js/app.js"),
        "document.documentElement.dataset.ferrioxidStatic = \"ready\";\n",
    )?;
    write(&destination.join("media/.gitkeep"), "")?;
    write(&destination.join("templates/base.html"), "<!doctype html>\n<html lang=\"{{ locale }}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{{ app_name }}</title><link rel=\"stylesheet\" href=\"/static/css/app.css\"></head><body><header>{{ app_name }}</header><main>{% block content %}{% endblock %}</main></body></html>\n")?;
    write(&destination.join("templates/index.html"), "{% extends \"base.html\" %}{% block content %}<h1>{{ project_name }}</h1><p>Your Ferrioxid project is running. Business apps are added explicitly.</p><p>Create an app with <code>ferrioxid new app &lt;name&gt;</code>.</p>{% if admin %}<p><a href=\"/admin\">Admin</a></p>{% endif %}{% endblock %}\n")?;
    for status in [404, 500] {
        write(&destination.join(format!("templates/errors/{status}.html")), &format!("{{% extends \"base.html\" %}}{{% block content %}}<h1>{status}</h1>{{% endblock %}}\n"))?;
    }
    write(&destination.join("tests/project.rs"), &format!("#[test]\nfn no_business_models_are_installed() {{\n    assert!({package}::apps::schema(None).unwrap().tables.is_empty());\n}}\n"))?;
    write_upgrade_manifest(&destination, template)?;
    Ok(destination)
}

pub(crate) fn crate_name(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut output = String::new();
    for (index, &character) in characters.iter().enumerate() {
        if character.is_ascii_uppercase()
            && index > 0
            && (characters[index - 1].is_ascii_lowercase()
                || characters[index - 1].is_ascii_digit()
                || characters
                    .get(index + 1)
                    .is_some_and(char::is_ascii_lowercase)
                    && characters[index - 1].is_ascii_uppercase())
        {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}

pub(crate) fn reserved_identifier(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "con"
            | "prn"
            | "aux"
            | "nul"
    ) || name
        .strip_prefix("com")
        .or_else(|| name.strip_prefix("lpt"))
        .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

#[derive(Clone, Copy)]
enum Template {
    Api,
    Fullstack,
}

fn parse_template(value: &str) -> Result<Template, String> {
    match value {
        "api" => Ok(Template::Api),
        "fullstack" => Ok(Template::Fullstack),
        _ => Err("Unknown template. Choose api or fullstack.".to_owned()),
    }
}

pub fn validate_project_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("The project name cannot be empty.".to_owned());
    };
    if !first.is_ascii_alphabetic()
        || !chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        || name.len() > 48
    {
        return Err(
            "The project name must start with a letter and contain only letters, digits, and underscores."
                .to_owned(),
        );
    }
    let reserved = reserved_identifier(&crate_name(name))
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "ferrioxid"
                | "test"
                | "main"
                | "src"
                | "project"
                | "apps"
                | "templates"
                | "static"
                | "media"
                | "migrations"
                | "cargo"
                | "target"
                | "tests"
        );
    if reserved {
        return Err(format!("`{name}` is a reserved project name."));
    }
    Ok(())
}

fn resolve_ferrioxid_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let raw = if let Some(path) = explicit {
        path.to_path_buf()
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    };
    let path = raw.canonicalize().map_err(|_| {
        "The Ferrioxid path does not exist. Use --ferrioxid-path to specify the repository."
            .to_owned()
    })?;
    let crate_manifest = path.join("crates/ferrioxid/Cargo.toml");
    if !crate_manifest.is_file() {
        return Err(
            "The Ferrioxid path does not contain crates/ferrioxid/Cargo.toml. The crate is not published yet."
                .to_owned(),
        );
    }
    Ok(path.join("crates/ferrioxid"))
}

fn cargo_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.replace('\\', "/")
}

fn write_upgrade_manifest(destination: &Path, template: Template) -> Result<(), String> {
    let mut pending = vec![destination.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(destination)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
                files.push(TrackedFile {
                    path: relative,
                    sha256: sha256_hex(&bytes),
                });
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let json = manifest_json(&ProjectManifest::generated(
        if matches!(template, Template::Api) {
            "api"
        } else {
            "fullstack"
        },
        files,
    ))
    .map_err(|error| error.0)?;
    std::fs::create_dir(destination.join(".ferrioxid")).map_err(|error| error.to_string())?;
    write(&destination.join(".ferrioxid/project.json"), &json)
}

pub(crate) fn write(path: &Path, contents: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{} will not be overwritten: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| error.to_string())
}

fn cargo_toml(name: &str, ferrioxid: &str, features: &str, sqlite_driver: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
default-run = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrioxid = {{ path = {ferrioxid}{features} }}
axum = "0.8"
serde = {{ version = "1", features = ["derive"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

# Keep the framework's SQLite security backport in this Cargo root.
[patch.crates-io]
sqlx-sqlite = {{ path = {sqlite_driver} }}
"#
    )
}

fn manifest(name: &str, template: Template, apps_dir: &str) -> String {
    let (template_name, admin, embedded, auth) = match template {
        Template::Api => ("api", "false", "false", "true"),
        Template::Fullstack => ("fullstack", "true", "true", "true"),
    };
    format!(
        r##"# Ferrioxid project. This file does not contain secrets.
migrations_dir = "migrations"
schema_command = "cargo run --quiet -- --print-schema"

[project]
name = "{name}"
template = "{template_name}"
layout_version = 2
apps_dir = "{apps_dir}"
default_locale = "de"
fallback_locale = "en"

[database]
url = "sqlite://db.sqlite3?mode=rwc"

[server]
host = "127.0.0.1"
port = 3000
debug = true

[admin]
enabled = {admin}
embedded_assets = {embedded}
site_title = "Ferrioxid Admin"
site_header = "Ferrioxid Verwaltung"

[auth]
enabled = {auth}
session_hours = 12
idle_minutes = 30

[templates]
enabled = true
engine = "tera"
templates_dir = "templates"

[branding]
app_name = "{name}"
primary_color = "#0f172a"
currency = "EUR"

[static]
enabled = true
url_prefix = "/static"
directory = "static"
cache_seconds = 3600

[storage]
backend = "local"
directory = "media"
max_upload_size_mb = 10

[jobs]
enabled = true

[mail]
backend = "disabled"
public_origin = "http://127.0.0.1:3000"
reset_ttl_minutes = 30
"##
    )
}

fn env_example() -> &'static str {
    "DATABASE_URL=sqlite://db.sqlite3?mode=rwc\nBIND_ADDRESS=127.0.0.1:3000\n# FERRIOXID_CSRF_SECRET must contain 64 hexadecimal characters in production.\n# FERRIOXID_PASSWORD is read only when creating users non-interactively.\n# FERRIOXID_SMTP_PASSWORD must not be stored in ferrioxid.toml.\n# FERRIOXID_PUBLIC_ORIGIN sets the password reset URL, for example https://example.com.\n"
}

fn settings_rs() -> &'static str {
    r#"pub fn load() -> Result<ferrioxid::migrations::ProjectConfig, Box<dyn std::error::Error>> {
    let settings = ferrioxid::migrations::Settings::discover(std::path::Path::new("."))?;
    Ok(settings.project().clone())
}
"#
}

fn readme(name: &str) -> String {
    format!(
        r#"# {name}

This is an empty project, not a business app. No domain models or app migrations are generated.

```sh
ferrioxid migrate up
ferrioxid new app produkte
ferrioxid generate model Item --app produkte
ferrioxid migrate make initial --app produkte
ferrioxid migrate up
ferrioxid doctor
ferrioxid dev
```

The Rust package name is normalized; the project directory/display name is preserved. Project startup, configuration and root URLs live in `src/project/`. `src/apps.rs` explicitly registers app modules, runtime apps and app schemas. `project.apps_dir` selects `apps` (default) or `.` for new apps. Each app remains a Rust module in this Cargo package, not a separate crate. Do not move an existing app by only changing this setting.

App templates are loaded from `<app>/templates/<appname>/`. Files in project `templates/<appname>/` explicitly override matching local names and may also exist without a local counterpart. Use `new app <name> --templates shared` to create the initial shared template. App migrations are declared in `[app.<name>]` and registered in Rust; mismatches fail startup. Removing an app requires removing both declarations; it never deletes existing tables or data.

Run `migrate up` before starting the server. It applies framework auth/job migrations and only your explicit app migrations. The default bind is loopback; no user or password is preconfigured. Create an administrator explicitly with `ferrioxid admin create-user --email name@example.com --staff --superuser`. Mail is disabled by default. `.env` and `.ferrioxid/csrf.key` stay outside Git.
"#
    )
}

pub(crate) const APPS_REGISTRY: &str = r#"// App files are explicit Rust modules, even when they live outside src/.
// ferrioxid:app-modules:end

pub fn install(app: ferrioxid::App) -> Result<ferrioxid::App, ferrioxid::apps::AppError> {
    let app = app
        // ferrioxid:apps:end
        ;
    app.seal()
}

pub fn schema(app: Option<&str>) -> Result<ferrioxid::migrations::SchemaSnapshot, Box<dyn std::error::Error>> {
    match app {
        None => Ok(ferrioxid::migrations::SchemaSnapshot::empty()),
        // ferrioxid:app-schema:end
        Some(name) => Err(format!("App `{name}` is not registered in src/apps.rs.").into()),
    }
}
"#;

const URLS_SOURCE: &str = r#"use axum::routing::get;
use ferrioxid::urls::{UrlError, UrlSet};

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("project.ping", "/api/v1/ping", get(ping))
}

async fn ping() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}))
}
"#;

const MANAGEMENT_SOURCE: &str = r#"use ferrioxid::async_trait::async_trait;
use ferrioxid::management::{ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand, Registry};
use ferrioxid::migrations::{discover_project, ProjectConfig};

struct About;

#[async_trait]
impl ManagementCommand for About {
    fn spec(&self) -> CommandSpec {
        CommandSpec {
            name: "about",
            about: "Prints the project name. This command does not open the database.",
            args: &[ArgSpec {
                name: "name",
                help: "Text to print instead of the project name.",
                value: true,
                required: false,
            }],
        }
    }

    async fn run(&self, invocation: &Invocation<'_>) -> Result<(), CommandError> {
        let name = invocation
            .options
            .get("name")
            .cloned()
            .or_else(|| invocation.project.name.clone())
            .unwrap_or_else(|| "ferrioxid".to_owned());
        println!("{name}");
        Ok(())
    }
}

pub async fn execute_with(project: &ProjectConfig, args: &[String]) -> u8 {
    let mut registry = Registry::new();
    if let Err(error) = registry.register(About) {
        eprintln!("{error}");
        return 1;
    }
    registry.execute(project, args).await
}

pub async fn execute() -> u8 {
    let directory = match std::env::current_dir() {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let project = match discover_project(&directory) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{}", error.0);
            return 2;
        }
    };
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    execute_with(&project, &args).await
}
"#;

fn project_source(template: Template) -> String {
    let admin = if matches!(template, Template::Fullstack) {
        r#"let admin = ferrioxid::admin::AdminRegistry::new().with_branding(ferrioxid::admin::AdminBranding {
        site_title: settings.site_title.clone(), site_header: settings.site_header.clone(),
        app_name: settings.app_name.clone(), primary_color: settings.primary_color.clone(),
        default_locale: settings.default_locale.clone(), fallback_locale: settings.fallback_locale.clone(), currency: settings.currency.clone(),
    });
    let app = app.with_admin(admin);"#
    } else {
        ""
    };
    format!(
        r#"pub mod management;
pub mod settings;
pub mod urls;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {{
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {{
        [flag] if flag == "--print-schema" => {{
            println!("{{}}", crate::apps::schema(None)?.to_canonical_json()?);
            return Ok(());
        }},
        [flag, selector, name] if flag == "--print-schema" && selector == "--app" => {{
            println!("{{}}", crate::apps::schema(Some(name))?.to_canonical_json()?);
            return Ok(());
        }},
        [] => {{}},
        _ => return Err("Use --print-schema [--app <name>] or run without arguments to serve.".into()),
    }}
    let settings = settings::load()?;
    tracing_subscriber::fmt().with_env_filter("info").init();
    let db = ferrioxid::db::connect(&settings.resolved_database_url()).await?;
    let address = settings.bind_address()?;
    let mut auth_config = ferrioxid::auth::AuthConfig::from_env();
    auth_config.apply_project_session(settings.session_hours, settings.idle_minutes);
    auth_config.apply_bound_address(&address);
    let auth = ferrioxid::auth::AuthService::new(db.clone(), auth_config);
    let app = ferrioxid::App::new(db).with_settings(settings.clone()).with_auth(auth).with_mail(ferrioxid::mail_for(&settings)).urls(urls::routes()?)?;
    {admin}
    let app = crate::apps::install(app)?;
    let names = app.route_names().clone();
    let router = ferrioxid::pages::mount_pages_with(app.try_into_router()?, &settings, &names)?;
    let router = ferrioxid::web::prepare(router, &settings);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Ferrioxid lauscht auf http://{{address}}");
    axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    Ok(())
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{crate_name, validate_project_name};

    #[test]
    fn rejects_unsafe_project_names() {
        for name in ["", "../secret", "shop;rm", "con", "com9", "type", "Self"] {
            assert!(validate_project_name(name).is_err(), "{name}");
        }
        assert!(validate_project_name("Shop").is_ok());
        assert_eq!(crate_name("Shop"), "shop");
        assert_eq!(crate_name("MyShop"), "my_shop");
        assert_eq!(crate_name("HTTPServer"), "http_server");
    }
}
