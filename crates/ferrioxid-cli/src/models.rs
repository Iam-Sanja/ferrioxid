//! Generates a model and registers it using stable source markers.

use crate::scaffold::validate_project_name;
use ferrioxid_migrations::discover_project;
use std::path::{Path, PathBuf};

pub fn generate_model(name: &str, app: Option<&str>, timestamps: bool) -> Result<PathBuf, String> {
    validate_model_name(name)?;
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    generate_in_project(&project, name, app, timestamps)
}

fn generate_in_project(
    project: &ferrioxid_migrations::ProjectConfig,
    name: &str,
    app: Option<&str>,
    timestamps: bool,
) -> Result<PathBuf, String> {
    validate_model_name(name)?;
    if project.layout_version == 2 {
        let app = app
            .ok_or("Models belong to apps. Use ferrioxid generate model <Name> --app <appname>.")?;
        return generate_app_model(project, name, app, timestamps);
    }
    if app.is_some() {
        return Err(
            "--app requires a layout-2 project. Existing root models are preserved.".to_owned(),
        );
    }
    let module = snake_case(name);
    validate_project_name(&module)?;
    let model_path = project.root.join(format!("src/models/{module}.rs"));
    if model_path.exists() {
        return Err(format!(
            "Model `{name}` already exists and will not be overwritten."
        ));
    }
    let package = package_name(&project.root)?;
    let template = project.template.as_deref().unwrap_or("api");
    let modules_path = project.root.join("src/models/mod.rs");
    let main_path = project.root.join("src/main.rs");
    let modules = std::fs::read_to_string(&modules_path).map_err(|error| error.to_string())?;
    let modules = insert_text(
        &modules,
        "// ferrioxid:models:end",
        &format!("pub mod {module};"),
    )?;
    let mut main = std::fs::read_to_string(&main_path).map_err(|error| error.to_string())?;
    for (marker, line) in [
        ("// ferrioxid:uses:end", format!("use {package}::models::{module}::{name};")),
        ("// ferrioxid:schema:end", format!("        RegisteredModel {{ table: {name}::descriptor().table, columns: {name}::columns(), constraints: {name}::constraints() }},")),
        ("// ferrioxid:registry:end", format!("        .register::<{name}>()")),
    ] {
        main = insert_text(&main, marker, &line)?;
    }
    if main.contains("// ferrioxid:admin:end") {
        main = insert_text(
            &main,
            "// ferrioxid:admin:end",
            &format!("        .add::<{name}>()"),
        )?;
    }
    crate::scaffold::write(
        &model_path,
        &model_source(name, &module, template, timestamps),
    )?;
    std::fs::write(modules_path, modules).map_err(|error| error.to_string())?;
    std::fs::write(main_path, main).map_err(|error| error.to_string())?;
    Ok(model_path)
}

pub fn validate_model_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("The model name cannot be empty.".to_owned());
    };
    if !first.is_ascii_uppercase() || !chars.all(|char| char.is_ascii_alphanumeric()) {
        return Err("The model name must use PascalCase without path characters.".to_owned());
    }
    Ok(())
}

fn snake_case(name: &str) -> String {
    let mut result = String::new();
    for (index, char) in name.chars().enumerate() {
        if char.is_ascii_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.push(char.to_ascii_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}

fn model_source(name: &str, module: &str, template: &str, timestamps: bool) -> String {
    let table = format!("{module}s");
    let attributes = if template == "fullstack" {
        format!(
            "    table = \"{table}\",\n    api = \"/api/v1/{table}\",\n    admin = true,\n    admin_label = \"{name}\",\n    permissions = \"{module}s\""
        )
    } else if template == "protected-api" {
        format!("    table = \"{table}\",\n    api = \"/api/v1/{table}\",\n    permissions = \"{module}s\"")
    } else {
        format!(
            "    table = \"{table}\",\n    api = \"/api/v1/{table}\",\n    allow_anonymous = true"
        )
    };
    let body = format!(
        r#"#[derive(FerrioxidModel, Serialize, Deserialize)]
#[ferrioxid(
{attributes}
)]
pub struct {name} {{
    #[field(primary_key, auto)]
    pub id: Uuid,

    #[field(required, min_length = 1)]
    pub name: String,
}}
"#
    );
    let model = if timestamps {
        format!("ferrioxid::with_timestamps! {{\n{body}}}\n")
    } else {
        body
    };
    format!("use ferrioxid::prelude::*;\nuse serde::{{Deserialize, Serialize}};\n\n{model}")
}

pub(crate) fn insert_text(text: &str, marker: &str, line: &str) -> Result<String, String> {
    if text.contains(line) {
        return Err(format!("Registration already exists: {line}"));
    }
    if text.matches(marker).count() != 1 {
        return Err(format!(
            "Expected one {marker} marker. No changes were made."
        ));
    }
    let index = text.find(marker).expect("one marker");
    Ok(format!("{}{line}\n{}", &text[..index], &text[index..]))
}

fn generate_app_model(
    project: &ferrioxid_migrations::ProjectConfig,
    name: &str,
    app: &str,
    timestamps: bool,
) -> Result<PathBuf, String> {
    crate::apps::validate_app_name(app)?;
    let declared = project
        .apps
        .iter()
        .find(|entry| entry.name == app)
        .ok_or_else(|| format!("App {app} is not declared in ferrioxid.toml."))?;
    let directory = declared
        .migrations
        .parent()
        .ok_or("The app directory is missing.")?;
    crate::apps::ensure_project_directory(&project.root, directory)?;
    let module = snake_case(name);
    validate_project_name(&module)?;
    let path = directory.join(format!("models/{module}.rs"));
    crate::apps::ensure_project_directory(&project.root, path.parent().expect("models directory"))?;
    if path.exists() || path.symlink_metadata().is_ok() {
        return Err(format!(
            "Model {name} already exists and will not be overwritten."
        ));
    }
    let modules_path = directory.join("models/mod.rs");
    let admin_path = directory.join("admin.rs");
    crate::apps::ensure_project_directory(&project.root, &modules_path)?;
    crate::apps::ensure_project_directory(&project.root, &admin_path)?;
    let mut modules = std::fs::read_to_string(&modules_path).map_err(|error| error.to_string())?;
    for (marker, line) in [
        ("// ferrioxid:models:end", format!("pub mod {module};")),
        ("// ferrioxid:registry:end", format!("    _registry.model::<{module}::{name}>()?;")),
        ("// ferrioxid:schema:end", format!("        ferrioxid::migrations::RegisteredModel {{ table: <{module}::{name} as ferrioxid::core::FerrioxidModel>::descriptor().table, columns: <{module}::{name} as ferrioxid::core::FerrioxidModel>::columns(), constraints: <{module}::{name} as ferrioxid::core::FerrioxidModel>::constraints() }},")),
    ] {
        modules = insert_text(&modules, marker, &line)?;
    }
    let fullstack = project.template.as_deref() == Some("fullstack");
    let admin = if fullstack {
        let text = std::fs::read_to_string(&admin_path).map_err(|error| error.to_string())?;
        Some(insert_text(
            &text,
            "// ferrioxid:admin:end",
            &format!("    _registry.admin_model::<super::models::{module}::{name}>()?;"),
        )?)
    } else {
        None
    };
    let template = if fullstack {
        "fullstack"
    } else {
        "protected-api"
    };
    crate::scaffold::write(&path, &model_source(name, &module, template, timestamps))?;
    std::fs::write(modules_path, modules).map_err(|error| error.to_string())?;
    if let Some(admin) = admin {
        std::fs::write(admin_path, admin).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

fn package_name(root: &Path) -> Result<String, String> {
    let text =
        std::fs::read_to_string(root.join("Cargo.toml")).map_err(|error| error.to_string())?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name = ") {
            let name = value.trim().trim_matches('"').to_owned();
            if name.is_empty() {
                return Err("The package name is empty.".to_owned());
            }
            return Ok(name);
        }
    }
    Err("Cargo.toml does not contain a package name.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{model_source, snake_case, validate_model_name};

    #[test]
    fn legacy_models_still_use_root_markers_and_fail_without_partial_writes() {
        let root =
            std::env::temp_dir().join(format!("ferrioxid-legacy-model-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src/models")).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"legacy\"\n").unwrap();
        std::fs::write(root.join("src/models/mod.rs"), "// ferrioxid:models:end\n").unwrap();
        std::fs::write(
            root.join("src/main.rs"),
            "// ferrioxid:uses:end\n// ferrioxid:schema:end\n// ferrioxid:registry:end\n",
        )
        .unwrap();
        let project = ferrioxid_migrations::discover_project(&root).unwrap();
        let path = super::generate_in_project(&project, "Customer", None, false).unwrap();
        assert_eq!(path, root.join("src/models/customer.rs"));
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("allow_anonymous = true"));
        let before = std::fs::read(root.join("src/models/mod.rs")).unwrap();
        std::fs::write(root.join("src/main.rs"), "// no markers").unwrap();
        assert!(super::generate_in_project(&project, "Invoice", None, false).is_err());
        assert!(!root.join("src/models/invoice.rs").exists());
        assert_eq!(
            std::fs::read(root.join("src/models/mod.rs")).unwrap(),
            before
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn timestamps_stay_optional() {
        let plain = model_source("Product", "product", "api", false);
        assert!(!plain.contains("with_timestamps"));
        let stamped = model_source("Product", "product", "api", true);
        assert!(stamped.contains("ferrioxid::with_timestamps!"));
        assert!(stamped.contains("pub name: String"));
    }

    #[test]
    fn model_names_stay_on_one_identifier() {
        assert!(validate_model_name("Customer").is_ok());
        assert!(validate_model_name("../Customer").is_err());
        assert!(validate_model_name("customer").is_err());
        assert_eq!(snake_case("ProductNote"), "product_note");
    }
}
