//! Ferrum-CLI. Migrationen fragen die Modell-Registry über ferrum.toml ab.

use clap::{Parser, Subcommand};
use ferrum_migrations::{
    apply_directory, discover_project, load_schema_from_command, make_migration, migration_status,
    MakeOutcome, SchemaSnapshot,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "ferrum", about = "Ferrum-Entwicklungswerkzeuge", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Zeigt den aktuellen Funktionsumfang.
    Doctor,
    /// Versionierte Schema-Migrationen.
    Migrate {
        #[command(subcommand)]
        action: MigrateCommand,
    },
}

#[derive(Subcommand)]
enum MigrateCommand {
    /// Schreibt eine Migration aus dem registrierten Modellschema.
    Make {
        /// Kurzname, zum Beispiel add_note.
        name: Option<String>,
        /// Snapshot-JSON statt schema_command.
        #[arg(long)]
        schema_file: Option<PathBuf>,
        /// Überschreibt migrations_dir aus ferrum.toml.
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
    },
    /// Wendet ausstehende Migrationen an.
    Up {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        /// Erlaubt ausdrücklich als riskant markierte Löschungen.
        #[arg(long)]
        allow_destructive: bool,
    },
    /// Zeigt Backend, Dateien, Historie, Konflikte und Schemaabweichung.
    Status {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Doctor => {
            print_doctor();
            ExitCode::SUCCESS
        }
        Command::Migrate { action } => match run_migrate(action) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
    }
}

fn print_doctor() {
    println!("Ferrum: SQLite ist die lokale Standarddatenbank. PostgreSQL wählt DATABASE_URL.");
    println!("Standarddatei: sqlite://db.sqlite3?mode=rwc im Arbeitsverzeichnis.");
    println!("Modelle werden nicht aus dem Quelltext gelesen. ferrum.toml nennt schema_command und migrations_dir.");
    println!("Migrationen: ferrum migrate make <name>, ferrum migrate up, ferrum migrate status");
    println!("--bootstrap an myshop ist Legacy und schreibt kein Schema und keine Historie.");
    println!("Nicht implementiert: Admin, Authentifizierung, Rename-Ausführung, Tabellen-Neubau, Down-Migrationen.");
}

fn run_migrate(action: MigrateCommand) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    match action {
        MigrateCommand::Make {
            name,
            schema_file,
            migrations_dir,
        } => {
            let (directory, schema) = resolve_schema(schema_file, migrations_dir)?;
            match make_migration(&directory, name.as_deref().unwrap_or("change"), &schema)
                .map_err(|error| error.0)?
            {
                MakeOutcome::Unchanged => {
                    println!("Keine Schemaänderung. Es wurde keine Migration geschrieben.");
                }
                MakeOutcome::Written { path, id, risks } => {
                    println!("Migration geschrieben: {} ({id})", path.display());
                    if risks.is_empty() {
                        println!("Risiken: keine");
                    } else {
                        println!("Risiken:");
                        for risk in risks {
                            println!("  {}: {}", risk.code, risk.message);
                        }
                        println!(
                            "ferrum migrate up führt diese Datei erst mit --allow-destructive aus."
                        );
                    }
                }
            }
        }
        MigrateCommand::Up {
            database_url,
            migrations_dir,
            allow_destructive,
        } => {
            let directory = resolve_directory(migrations_dir)?;
            let url = resolve_database_url(database_url)?;
            let report = runtime.block_on(async {
                let db = ferrum_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                apply_directory(&db, &directory, allow_destructive)
                    .await
                    .map_err(|error| error.0)
            })?;
            if report.applied.is_empty() {
                println!("Keine ausstehenden Migrationen.");
            } else {
                for id in report.applied {
                    println!("Angewendet: {id}");
                }
            }
        }
        MigrateCommand::Status {
            database_url,
            migrations_dir,
            schema_file,
        } => {
            let directory = resolve_directory(migrations_dir)?;
            let schema = load_optional_schema(schema_file)?;
            let url = resolve_database_url(database_url)?;
            let report = runtime.block_on(async {
                let db = ferrum_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                migration_status(&db, &directory, schema.as_ref())
                    .await
                    .map_err(|error| error.0)
            })?;
            print!("{}", report.render());
            if !report.conflicts.is_empty() {
                return Err("Es gibt Migrationskonflikte.".to_owned());
            }
        }
    }
    Ok(())
}

fn resolve_directory(override_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(directory) = override_dir {
        return Ok(directory);
    }
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    Ok(project.migrations_dir)
}

fn resolve_schema(
    schema_file: Option<PathBuf>,
    migrations_dir: Option<PathBuf>,
) -> Result<(PathBuf, SchemaSnapshot), String> {
    if let Some(path) = schema_file {
        let directory = resolve_directory(migrations_dir)?;
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let schema = SchemaSnapshot::from_json(&text).map_err(|error| error.0)?;
        return Ok((directory, schema));
    }
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    let directory = migrations_dir.unwrap_or(project.migrations_dir);
    let command = project.schema_command.ok_or_else(|| {
        "ferrum.toml enthält kein schema_command. Übergib --schema-file.".to_owned()
    })?;
    let schema = load_schema_from_command(&project.root, &command).map_err(|error| error.0)?;
    Ok((directory, schema))
}

fn load_optional_schema(schema_file: Option<PathBuf>) -> Result<Option<SchemaSnapshot>, String> {
    if let Some(path) = schema_file {
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        return Ok(Some(
            SchemaSnapshot::from_json(&text).map_err(|error| error.0)?,
        ));
    }
    let Ok(project) =
        discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
    else {
        return Ok(None);
    };
    let Some(command) = project.schema_command else {
        return Ok(None);
    };
    Ok(Some(
        load_schema_from_command(&project.root, &command).map_err(|error| error.0)?,
    ))
}

fn resolve_database_url(override_url: Option<String>) -> Result<String, String> {
    if let Some(url) = override_url {
        return Ok(url);
    }
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }
    Ok(ferrum_db::DEFAULT_SQLITE_URL.to_owned())
}
