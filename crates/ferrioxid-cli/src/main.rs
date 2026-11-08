//! Ferrioxid-CLI. Migrationen fragen die Modell-Registry über ferrum.toml ab.

mod apps;
mod develop;
mod diagnose;
mod jobs;
mod models;
mod scaffold;
mod text;

use clap::{Parser, Subcommand};
use ferrioxid_migrations::{
    apply_directory, apply_packages, baseline_database, discover_project,
    inspect_migrations_with_packages, load_schema_from_command, make_migration,
    readonly_database_url, render_diagnosis, reseal_unapplied, runtime_contract_note,
    write_explicit_migration, MakeOutcome, MigrationOperation, PackageMigration, SchemaSnapshot,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "ferrioxid", about = "Ferrioxid-Entwicklungswerkzeuge", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Erzeugt ein eigenständiges Ferrioxid-Projekt.
    New {
        name: String,
        /// `api` oder `fullstack`. Standard ist `fullstack`.
        #[arg(long, default_value = "fullstack")]
        template: String,
        /// Pfad zum Ferrioxid-Repository, solange Ferrioxid nicht veröffentlicht ist.
        #[arg(long)]
        ferrioxid_path: Option<PathBuf>,
    },
    /// Prüft Werkzeuge, Konfiguration und Projektdateien.
    Doctor,
    /// Prüft Konfiguration, Migrationen und Kompilierung, ohne Daten zu ändern.
    Check {
        /// Warnungen, ausstehende Migrationen und Legacy-Projekte beenden mit Exitcode 2.
        #[arg(long)]
        strict: bool,
    },
    /// Analysiert ein bestehendes Projekt und schreibt höchstens ein fehlendes Legacy-Manifest.
    Upgrade {
        /// Nur Diagnose. Es wird keine Datei geschrieben.
        #[arg(long)]
        check: bool,
        /// Zeigt den Dateiplan. Es wird keine Datei geschrieben.
        #[arg(long)]
        dry_run: bool,
        /// Legt ein fehlendes Legacy-Manifest an. Überschreibt keine Projektdatei.
        #[arg(long)]
        yes: bool,
    },
    /// Startet den Entwicklungsserver und startet ihn nach Rust-Änderungen neu.
    Dev,
    /// Kompiliert das Projekt. Fullstack bettet das Admin-Frontend ein.
    Build,
    /// Erzeugt Quelltext.
    Generate {
        #[command(subcommand)]
        action: GenerateCommand,
    },
    /// Versionierte Schema-Migrationen.
    Migrate {
        #[command(subcommand)]
        action: MigrateCommand,
    },
    /// Benutzer anlegen. Das Passwort wird verdeckt abgefragt.
    Admin {
        #[command(subcommand)]
        action: AdminCommand,
    },
    /// Benutzer, Rollen und Berechtigungen.
    Auth {
        #[command(subcommand)]
        action: AuthCommand,
    },
    /// Startet den Job-Worker. Der Webserver bleibt aus.
    Worker {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        concurrency: Option<u16>,
        #[arg(long)]
        poll_ms: Option<u64>,
    },
    /// Zeigt und steuert persistente Jobs.
    Jobs {
        #[command(subcommand)]
        action: JobsCommand,
    },
}

#[derive(Subcommand)]
enum JobsCommand {
    /// Listet Jobs. Der Befehl schreibt nicht.
    List {
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Zeigt einen Job. Die Payload bleibt ohne `--show-payload` verborgen.
    Inspect {
        id: String,
        /// Gibt die gespeicherte Payload aus. Sie kann personenbezogene Daten enthalten.
        #[arg(long)]
        show_payload: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Setzt einen fehlgeschlagenen oder abgebrochenen Job zurück.
    Retry {
        id: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Bricht einen wartenden Job ab.
    Cancel {
        id: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum GenerateCommand {
    /// Legt ein Modell an und trägt es in die Registry ein.
    Model {
        name: String,
        /// Ergänzt die verwalteten Felder created_at und updated_at.
        #[arg(long)]
        timestamps: bool,
    },
    /// Legt eine Ferrioxid-App an, ohne vorhandene Dateien zu überschreiben.
    App { name: String },
    /// Schreibt `src/bin/worker.rs`, ohne vorhandene Dateien zu überschreiben.
    Worker,
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Legt einen Benutzer an. Es gibt kein voreingestelltes Admin-Passwort.
    CreateUser {
        /// Login. Ohne Angabe wird die Adresse abgefragt.
        #[arg(long)]
        email: Option<String>,
        /// Öffnet die Verwaltung. Ein Superuser ohne dieses Flag bleibt ausgesperrt.
        #[arg(long)]
        staff: bool,
        /// Modellrechte. Setzt Staff nicht und öffnet die Verwaltung nicht allein.
        #[arg(long)]
        superuser: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Listet Benutzer ohne Passwort-Hash.
    ListUsers {
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Legt eine Rolle an oder erkennt eine vorhandene.
    CreateRole {
        name: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Vergibt `resource.read|create|update|delete` an eine Rolle.
    GrantPermission {
        code: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Entzieht einer Rolle eine Berechtigung.
    RevokePermission {
        code: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Weist einem Benutzer eine Rolle zu.
    AssignRole {
        #[arg(long)]
        email: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Entzieht einem Benutzer eine Rolle.
    RevokeRole {
        #[arg(long)]
        email: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
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
    /// Schreibt eine explizite Spaltenumbenennung. Das registrierte Schema muss bereits den neuen Namen enthalten.
    RenameColumn {
        #[arg(long)]
        table: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        /// Kurzname der Datei. Standard ist rename_<ziel>.
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
    },
    /// Schreibt eine explizite Tabellenumbenennung. Das registrierte Schema muss bereits den neuen Namen enthalten.
    RenameTable {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
    },
    /// Berechnet die Checksum einer noch nicht angewendeten Datei neu.
    Reseal {
        path: PathBuf,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Markiert vorhandene SQLite-Migrationen als angewendet, wenn das Schema exakt passt. Ohne --confirm wird nichts geschrieben.
    Baseline {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        /// Schreibt nach erfolgreicher Prüfung nur die Historie.
        #[arg(long)]
        confirm: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::New {
            name,
            template,
            ferrioxid_path,
        } => match scaffold::new_project(&name, &template, ferrioxid_path.as_deref()) {
            Ok(path) => {
                println!("{} {}", text::line("project_created"), path.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Command::Doctor => diagnose::doctor(),
        Command::Check { strict } => diagnose::check(strict),
        Command::Upgrade {
            check,
            dry_run,
            yes,
        } => diagnose::upgrade(check, dry_run, yes),
        Command::Dev => develop::dev(),
        Command::Build => develop::build(),
        Command::Generate { action } => match action {
            GenerateCommand::Worker => match apps::generate_worker() {
                Ok(path) => {
                    println!("Worker angelegt: {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            },
            GenerateCommand::App { name } => match apps::generate_app(&name) {
                Ok(path) => {
                    println!("App angelegt: {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            },
            GenerateCommand::Model { name, timestamps } => {
                match models::generate_model(&name, timestamps) {
                    Ok(path) => {
                        println!("{} {}", text::line("model_created"), path.display());
                        ExitCode::SUCCESS
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        ExitCode::from(1)
                    }
                }
            }
        },
        Command::Migrate { action } => match run_migrate(action) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Command::Admin { action } => match run_admin(action) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Command::Auth { action } => match run_auth(action) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Command::Worker {
            database_url,
            concurrency,
            poll_ms,
        } => {
            let project = match optional_project() {
                Ok(project) => project,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            };
            let url = match jobs_database_url(database_url, project.as_ref()) {
                Ok(url) => url,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            };
            jobs::worker(url, project.as_ref(), concurrency, poll_ms)
        }
        Command::Jobs { action } => {
            let project = match optional_project() {
                Ok(project) => project,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            };
            if project
                .as_ref()
                .is_some_and(|project| !project.jobs_enabled)
                && matches!(
                    action,
                    JobsCommand::Retry { .. } | JobsCommand::Cancel { .. }
                )
            {
                eprintln!("job.disabled jobs.enabled ist false.");
                return ExitCode::from(2);
            }
            let database_url = match &action {
                JobsCommand::List { database_url }
                | JobsCommand::Inspect { database_url, .. }
                | JobsCommand::Retry { database_url, .. }
                | JobsCommand::Cancel { database_url, .. } => database_url.clone(),
            };
            let url = match jobs_database_url(database_url, project.as_ref()) {
                Ok(url) => url,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            };
            match action {
                JobsCommand::List { .. } => jobs::list(url),
                JobsCommand::Inspect {
                    id, show_payload, ..
                } => jobs::inspect(url, id, show_payload),
                JobsCommand::Retry { id, yes, .. } => jobs::retry(url, id, yes),
                JobsCommand::Cancel { id, yes, .. } => jobs::cancel(url, id, yes),
            }
        }
    }
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
                    print_risks(&risks);
                }
            }
        }
        MigrateCommand::RenameColumn {
            table,
            from,
            to,
            name,
            schema_file,
            migrations_dir,
        } => {
            let (directory, schema) = resolve_schema(schema_file, migrations_dir)?;
            let slug = name.unwrap_or_else(|| format!("rename_{to}"));
            let outcome = write_explicit_migration(
                &directory,
                &slug,
                &schema,
                MigrationOperation::RenameColumn { table, from, to },
            )
            .map_err(|error| error.0)?;
            print_written(outcome);
        }
        MigrateCommand::RenameTable {
            from,
            to,
            name,
            schema_file,
            migrations_dir,
        } => {
            let (directory, schema) = resolve_schema(schema_file, migrations_dir)?;
            let slug = name.unwrap_or_else(|| format!("rename_{to}"));
            let outcome = write_explicit_migration(
                &directory,
                &slug,
                &schema,
                MigrationOperation::RenameTable { from, to },
            )
            .map_err(|error| error.0)?;
            print_written(outcome);
        }
        MigrateCommand::Reseal { path, database_url } => {
            let url = resolve_database_url(database_url)?;
            let file = runtime.block_on(async {
                let db = ferrioxid_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                reseal_unapplied(&db, &path).await.map_err(|error| error.0)
            })?;
            println!(
                "Checksum neu geschrieben für {}. Angewendete Migrationen bleiben unverändert.",
                file.id
            );
        }
        MigrateCommand::Baseline {
            database_url,
            migrations_dir,
            confirm,
        } => {
            let directory = resolve_directory(migrations_dir)?;
            let url = resolve_database_url(database_url)?;
            let ids = runtime.block_on(async {
                let db = ferrioxid_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                baseline_database(&db, &directory, confirm)
                    .await
                    .map_err(|error| error.0)
            })?;
            println!("Baseline geschrieben. Es wurden keine Tabellen und keine Daten verändert.");
            for id in ids {
                println!("Als angewendet markiert: {id}");
            }
        }
        MigrateCommand::Up {
            database_url,
            migrations_dir,
            allow_destructive,
        } => {
            let directory = resolve_directory(migrations_dir)?;
            let url = resolve_database_url(database_url)?;
            let (auth_applied, jobs_applied, report, package_report) = runtime.block_on(async {
                let db = ferrioxid_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth_applied = ferrioxid_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let jobs_applied = ferrioxid_jobs::apply_migrations(&db)
                    .await
                    .map_err(|error| error.detail().to_owned())?;
                let report = apply_directory(&db, &directory, allow_destructive)
                    .await
                    .map_err(|error| error.0)?;
                let packages = declared_packages()?;
                let package_report = apply_packages(&db, &packages, allow_destructive)
                    .await
                    .map_err(|error| error.0)?;
                Ok::<_, String>((auth_applied, jobs_applied, report, package_report))
            })?;
            if auth_applied.is_empty() {
                println!("Keine ausstehenden Auth-Migrationen.");
            } else {
                for id in auth_applied {
                    println!("Auth-Migration angewendet: {id}");
                }
            }
            if jobs_applied.is_empty() {
                println!("Keine ausstehenden Job-Migrationen.");
            } else {
                for id in jobs_applied {
                    println!("Job-Migration angewendet: {id}");
                }
            }
            if report.applied.is_empty() {
                println!("Keine ausstehenden Migrationen.");
            } else {
                for id in report.applied {
                    println!("Angewendet: {id}");
                }
            }
            if package_report.is_empty() {
                println!("Keine ausstehenden App-Migrationen.");
            } else {
                for id in package_report {
                    println!("App-Migration angewendet: {id}");
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
            let readonly = readonly_database_url(&url).map_err(|error| error.0)?;
            let report = runtime.block_on(async {
                let db = ferrioxid_db::connect(&readonly)
                    .await
                    .map_err(|error| format!("migration.database.unavailable: {error}"))?;
                inspect_migrations_with_packages(
                    &db,
                    &ferrioxid_auth::migrations_dir(),
                    &directory,
                    schema.as_ref(),
                    &declared_packages()?,
                )
                .await
                .map_err(|error| error.0)
            })?;
            print!("{}", render_diagnosis(&text::locale(), &report));
            if !report.packages.is_empty() {
                println!("{}", runtime_contract_note());
            }
            if report.has_error() {
                return Err("Es gibt Migrationskonflikte.".to_owned());
            }
            let jobs_current = runtime.block_on(async {
                let db = ferrioxid_db::connect(&readonly)
                    .await
                    .map_err(|error| format!("migration.database.unavailable: {error}"))?;
                jobs::print_status(&db).await
            })?;
            if !jobs_current {
                println!("Job-Migrationen sind nicht vollständig angewendet. ferrioxid migrate up holt sie nach. Dieser Status schreibt nicht.");
            }
        }
    }
    Ok(())
}

fn optional_project() -> Result<Option<ferrioxid_migrations::ProjectConfig>, String> {
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    match ferrioxid_migrations::discover_project(&directory) {
        Ok(project) => Ok(Some(project)),
        Err(error) if error.0.contains("Keine ferrum.toml") => Ok(None),
        Err(error) => Err(error.0),
    }
}

fn jobs_database_url(
    override_url: Option<String>,
    project: Option<&ferrioxid_migrations::ProjectConfig>,
) -> Result<String, String> {
    if let Some(url) = override_url {
        return Ok(url);
    }
    if let Some(project) = project {
        return Ok(project.resolved_database_url());
    }
    match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => Ok(url),
        _ => Err(
            "job.database Es gibt keine Datenbank-URL. Setze DATABASE_URL oder --database-url."
                .to_owned(),
        ),
    }
}

fn print_written(outcome: MakeOutcome) {
    match outcome {
        MakeOutcome::Unchanged => {
            println!("Keine Schemaänderung. Es wurde keine Migration geschrieben.");
        }
        MakeOutcome::Written { path, id, risks } => {
            println!("Migration geschrieben: {} ({id})", path.display());
            print_risks(&risks);
        }
    }
}

fn print_risks(risks: &[ferrioxid_migrations::MigrationRisk]) {
    if risks.is_empty() {
        println!("Risiken: keine");
        return;
    }
    println!("Risiken:");
    for risk in risks {
        println!("  {}: {}", risk.code, risk.message);
    }
    let destructive = risks
        .iter()
        .any(|risk| risk.code == "drop_table" || risk.code == "drop_column");
    if destructive {
        println!("Löschungen führt ferrioxid migrate up erst mit --allow-destructive aus.");
    }
}

fn declared_packages() -> Result<Vec<PackageMigration>, String> {
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    match discover_project(&directory) {
        Ok(project) => Ok(project
            .apps
            .into_iter()
            .map(|app| PackageMigration {
                name: app.name,
                directory: app.migrations,
                depends_on: app.depends_on,
            })
            .collect()),
        Err(error) if error.0.contains("Keine ferrum.toml") => Ok(Vec::new()),
        Err(error) => Err(error.0),
    }
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

fn run_admin(action: AdminCommand) -> Result<(), String> {
    match action {
        AdminCommand::CreateUser {
            email,
            staff,
            superuser,
            database_url,
        } => {
            let email = match email {
                Some(email) => email,
                None => prompt_line("E-Mail: ")?,
            };
            let password = if let Ok(value) = std::env::var("FERRUM_PASSWORD") {
                if value.len() < 12 {
                    return Err(
                        "FERRUM_PASSWORD muss mindestens 12 Zeichen haben. Es gibt kein Standardpasswort."
                            .to_owned(),
                    );
                }
                value
            } else {
                let password = prompt_secret("Passwort: ")?;
                let confirm = prompt_secret("Passwort wiederholen: ")?;
                if password != confirm {
                    return Err("Die Passwörter stimmen nicht überein.".to_owned());
                }
                password
            };
            let url = resolve_database_url(database_url)?;
            let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
            let user = runtime.block_on(async {
                let db = ferrioxid_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                ferrioxid_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth =
                    ferrioxid_auth::AuthService::new(db, ferrioxid_auth::AuthConfig::from_env());
                auth.create_user(&email, &password, staff, superuser)
                    .await
                    .map_err(auth_message)
            })?;
            println!(
                "{} {} ({})",
                crate::text::line("user_created"),
                user.email,
                user.id
            );
            println!("{}", crate::text::line("flag_active"));
            println!(
                "{}",
                crate::text::line(if user.is_staff {
                    "flag_staff_on"
                } else {
                    "flag_staff_off"
                })
            );
            println!(
                "{}",
                crate::text::line(if user.is_superuser {
                    "flag_super_on"
                } else {
                    "flag_super_off"
                })
            );
            if user.is_superuser && !user.is_staff {
                println!("{}", crate::text::line("super_without_staff"));
            }
            Ok(())
        }
    }
}

fn run_auth(action: AuthCommand) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    match action {
        AuthCommand::ListUsers { database_url } => {
            let url = resolve_database_url(database_url)?;
            let users = runtime.block_on(async {
                let service = open_auth(&url).await?;
                service.list_users().await.map_err(auth_message)
            })?;
            for user in users {
                println!(
                    "{}  {}  active={} staff={} superuser={}",
                    user.id, user.email, user.is_active, user.is_staff, user.is_superuser
                );
            }
        }
        AuthCommand::CreateRole { name, database_url } => {
            let url = resolve_database_url(database_url)?;
            let id = runtime.block_on(async {
                let service = open_auth(&url).await?;
                service.ensure_role(&name).await.map_err(auth_message)
            })?;
            println!("Rolle bereit: {name} ({id})");
        }
        AuthCommand::GrantPermission {
            code,
            role,
            database_url,
        } => {
            let url = resolve_database_url(database_url)?;
            runtime.block_on(async {
                let service = open_auth(&url).await?;
                service
                    .grant_permission(&role, &code)
                    .await
                    .map_err(auth_message)
            })?;
            println!("Berechtigung {code} an Rolle {role} vergeben.");
        }
        AuthCommand::RevokePermission {
            code,
            role,
            database_url,
        } => {
            let url = resolve_database_url(database_url)?;
            runtime.block_on(async {
                let service = open_auth(&url).await?;
                service
                    .revoke_permission(&role, &code)
                    .await
                    .map_err(auth_message)
            })?;
            println!("Berechtigung {code} von Rolle {role} entzogen.");
        }
        AuthCommand::AssignRole {
            email,
            role,
            database_url,
        } => {
            let url = resolve_database_url(database_url)?;
            runtime.block_on(async {
                let service = open_auth(&url).await?;
                service
                    .assign_role(&email, &role)
                    .await
                    .map_err(auth_message)
            })?;
            println!("Rolle {role} an {email} zugewiesen.");
        }
        AuthCommand::RevokeRole {
            email,
            role,
            database_url,
        } => {
            let url = resolve_database_url(database_url)?;
            runtime.block_on(async {
                let service = open_auth(&url).await?;
                service
                    .revoke_role(&email, &role)
                    .await
                    .map_err(auth_message)
            })?;
            println!("Rolle {role} von {email} entzogen.");
        }
    }
    Ok(())
}

async fn open_auth(url: &str) -> Result<ferrioxid_auth::AuthService, String> {
    let db = ferrioxid_db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    ferrioxid_auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ferrioxid_auth::AuthService::new(
        db,
        ferrioxid_auth::AuthConfig::from_env(),
    ))
}

fn prompt_line(label: &str) -> Result<String, String> {
    use std::io::Write;
    eprint!("{label}");
    std::io::stderr()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    Ok(line.trim().to_owned())
}

fn prompt_secret(label: &str) -> Result<String, String> {
    rpassword::prompt_password(label).map_err(|error| error.to_string())
}

fn auth_message(error: ferrioxid_auth::AuthError) -> String {
    match error {
        ferrioxid_auth::AuthError::Validation(message) if message == "record already exists" => {
            "Der Benutzer oder die Zuordnung existiert bereits.".to_owned()
        }
        ferrioxid_auth::AuthError::Validation(message) if message == "user was not found" => {
            "Der Benutzer wurde nicht gefunden.".to_owned()
        }
        ferrioxid_auth::AuthError::Validation(message) if message == "email is invalid" => {
            "Die E-Mail-Adresse ist ungültig.".to_owned()
        }
        ferrioxid_auth::AuthError::Database(_) => {
            "Die Datenbankoperation ist fehlgeschlagen.".to_owned()
        }
        other => other.to_string(),
    }
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
    Ok(ferrioxid_db::DEFAULT_SQLITE_URL.to_owned())
}
