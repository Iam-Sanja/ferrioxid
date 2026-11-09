//! `dumpdata` und `loaddata`.
//!
//! Die Befehle lesen das Schema über dieselbe Projekterkennung wie `migrate`.
//! Fixture-JSON wird nicht ausgeführt.

use std::collections::BTreeSet;
use std::path::PathBuf;

use ferrioxid_migrations::{
    dump_fixture, fixture_to_json, is_identifier, load_fixture, parse_fixture, LoadRequest,
    SchemaSnapshot,
};

pub struct DataError {
    message: String,
    usage: bool,
}

impl DataError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            usage: true,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            usage: false,
        }
    }

    pub fn code(&self) -> u8 {
        if self.usage {
            2
        } else {
            1
        }
    }
}

impl std::fmt::Display for DataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

pub fn dumpdata(
    model: Option<String>,
    app: Option<String>,
    schema_file: Option<PathBuf>,
    database_url: Option<String>,
    output: Option<PathBuf>,
) -> Result<(), DataError> {
    let schema = schema_for(app, schema_file)?;
    let tables = selected_tables(&schema, model)?;
    let url = database_url_for(database_url)?;
    let runtime =
        tokio::runtime::Runtime::new().map_err(|error| DataError::failed(error.to_string()))?;
    let file = runtime.block_on(async {
        let db = connect(&url).await?;
        dump_fixture(&db, &schema, tables.as_ref())
            .await
            .map_err(|error| DataError::failed(error.0))
    })?;
    let json = fixture_to_json(&file).map_err(|error| DataError::failed(error.0))?;
    match output {
        Some(path) => std::fs::write(&path, json).map_err(|error| {
            DataError::failed(format!("Could not write {}: {error}", path.display()))
        }),
        None => {
            println!("{json}");
            Ok(())
        }
    }
}

pub struct LoadOptions {
    pub file: PathBuf,
    pub dry_run: bool,
    pub upsert: bool,
    pub truncate: bool,
    pub yes: bool,
    pub model: Option<String>,
    pub app: Option<String>,
    pub schema_file: Option<PathBuf>,
    pub database_url: Option<String>,
}

pub fn loaddata(options: LoadOptions) -> Result<String, DataError> {
    let LoadOptions {
        file,
        dry_run,
        upsert,
        truncate,
        yes,
        model,
        app,
        schema_file,
        database_url,
    } = options;
    if truncate && !yes {
        return Err(DataError::usage(
            "loaddata --truncate deletes every row in the selected tables. Repeat the command with --yes to confirm. Nothing was written.",
        ));
    }
    if truncate && upsert {
        return Err(DataError::usage(
            "Choose either --upsert or --truncate, not both. Nothing was written.",
        ));
    }
    let text = std::fs::read_to_string(&file).map_err(|error| {
        DataError::failed(format!("Could not read {}: {error}", file.display()))
    })?;
    let fixture = parse_fixture(&text).map_err(|error| DataError::failed(error.0))?;
    let schema = schema_for(app, schema_file)?;
    let tables = selected_tables(&schema, model)?;
    let url = database_url_for(database_url)?;
    let request = LoadRequest {
        tables,
        upsert,
        truncate,
        dry_run,
    };
    let runtime =
        tokio::runtime::Runtime::new().map_err(|error| DataError::failed(error.to_string()))?;
    let report = runtime.block_on(async {
        let db = connect(&url).await?;
        load_fixture(&db, &schema, &fixture, &request)
            .await
            .map_err(|error| DataError::failed(error.0))
    })?;
    if !report.written {
        return Ok(format!(
            "dry-run: {} records would be loaded. Nothing was written.",
            report.records
        ));
    }
    Ok(format!("Loaded {} records.", report.records))
}

fn schema_for(
    app: Option<String>,
    schema_file: Option<PathBuf>,
) -> Result<SchemaSnapshot, DataError> {
    if app.is_some() && schema_file.is_some() {
        return Err(DataError::usage(
            "Use either --app or --schema-file. Nothing was written.",
        ));
    }
    if let Some(app) = app {
        return crate::resolve_app_schema(&app)
            .map(|(_directory, schema)| schema)
            .map_err(DataError::failed);
    }
    crate::resolve_schema(schema_file, None)
        .map(|(_directory, schema)| schema)
        .map_err(DataError::failed)
}

fn selected_tables(
    schema: &SchemaSnapshot,
    model: Option<String>,
) -> Result<Option<BTreeSet<String>>, DataError> {
    let Some(model) = model else {
        return Ok(None);
    };
    if !is_identifier(&model) {
        return Err(DataError::usage(format!(
            "--model '{model}' is not a table identifier. Nothing was written."
        )));
    }
    let known = schema.tables.iter().any(|table| table.name == model)
        || schema
            .external_tables
            .iter()
            .any(|table| table.table.name == model);
    if !known {
        return Err(DataError::failed(format!("Unknown model '{model}'.")));
    }
    Ok(Some(BTreeSet::from([model])))
}

fn database_url_for(override_url: Option<String>) -> Result<String, DataError> {
    let project = crate::optional_project().map_err(DataError::failed)?;
    crate::jobs_database_url(override_url, project.as_ref()).map_err(DataError::failed)
}

async fn connect(url: &str) -> Result<sea_orm::DatabaseConnection, DataError> {
    ferrioxid_db::connect(url).await.map_err(|error| {
        let message = error.to_string().replace(url, "[database]");
        DataError::failed(format!("database unavailable: {message}"))
    })
}
