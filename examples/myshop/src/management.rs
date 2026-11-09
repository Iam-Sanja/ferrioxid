//! Projektbefehle für myshop. Die Ferrioxid-CLI führt diesen Code nicht aus.

use std::path::{Component, Path, PathBuf};

use ferrioxid::async_trait::async_trait;
use ferrioxid::core::FerrioxidModel;
use ferrioxid::management::{
    ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand, Registry,
};
use ferrioxid::migrations::{
    apply_directory, discover_project, load_fixture, parse_fixture, LoadRequest, ProjectConfig,
    RegisteredModel, SchemaSnapshot,
};

use crate::models::category::Category;
use crate::models::product::Product;

struct SeedShop;

#[async_trait]
impl ManagementCommand for SeedShop {
    fn spec(&self) -> CommandSpec {
        CommandSpec {
            name: "seed-shop",
            about:
                "Upserts fixtures/catalog.json. Existing primary keys are updated, not duplicated.",
            args: &[
                ArgSpec {
                    name: "file",
                    help: "Fixture path inside this project.",
                    value: true,
                    required: false,
                },
                ArgSpec {
                    name: "insert",
                    help: "Insert only. An existing primary key rolls the whole file back.",
                    value: false,
                    required: false,
                },
            ],
        }
    }

    async fn run(&self, invocation: &Invocation<'_>) -> Result<(), CommandError> {
        let relative = invocation
            .options
            .get("file")
            .cloned()
            .unwrap_or_else(|| "fixtures/catalog.json".to_owned());
        let path = fixture_path(Path::new(env!("CARGO_MANIFEST_DIR")), &relative)?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            CommandError::new(format!("Could not read {}: {error}", path.display()))
        })?;
        let file = parse_fixture(&text)?;
        let db = invocation.database().await?;
        apply_directory(&db, &invocation.project.migrations_dir, false).await?;
        let request = LoadRequest {
            upsert: !invocation.flags.contains("insert"),
            ..LoadRequest::default()
        };
        let report = load_fixture(&db, &catalog_schema()?, &file, &request).await?;
        println!("Loaded {} records.", report.records);
        Ok(())
    }
}

pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(SeedShop)
        .expect("seed-shop is a valid unique command");
    registry
}

pub async fn execute_with(project: &ProjectConfig, args: &[String]) -> u8 {
    registry().execute(project, args).await
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

fn catalog_schema() -> Result<SchemaSnapshot, CommandError> {
    SchemaSnapshot::from_registered(&[
        RegisteredModel {
            table: Category::descriptor().table,
            columns: Category::columns(),
            constraints: Category::constraints(),
        },
        RegisteredModel {
            table: Product::descriptor().table,
            columns: Product::columns(),
            constraints: Product::constraints(),
        },
    ])
    .map_err(CommandError::from)
}

fn fixture_path(root: &Path, raw: &str) -> Result<PathBuf, CommandError> {
    let candidate = Path::new(raw);
    if raw.trim().is_empty()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CommandError::usage(
            "The fixture path must stay inside the project or the system temp directory. Nothing was written.",
        ));
    }
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let parent = path.parent().ok_or_else(|| {
        CommandError::usage("The fixture path must name a file. Nothing was written.")
    })?;
    let parent = parent.canonicalize().map_err(|error| {
        CommandError::new(format!("Could not resolve {}: {error}", path.display()))
    })?;
    let root = root.canonicalize().map_err(|error| {
        CommandError::new(format!("Could not resolve the project root: {error}"))
    })?;
    let temp = std::env::temp_dir().canonicalize().map_err(|error| {
        CommandError::new(format!("Could not resolve the temp directory: {error}"))
    })?;
    if parent.starts_with(&root) || parent.starts_with(&temp) {
        Ok(path)
    } else {
        Err(CommandError::usage(
            "The fixture path must stay inside the project or the system temp directory. Nothing was written.",
        ))
    }
}
