//! Ferrum-CLI. Migrationen fragen die Modell-Registry über ferrum.toml ab.

mod develop;
mod diagnose;
mod models;
mod scaffold;
mod text;

use clap::{Parser, Subcommand};
use ferrum_migrations::{
    apply_directory, baseline_database, discover_project, load_schema_from_command, make_migration,
    migration_status, reseal_unapplied, write_explicit_migration, MakeOutcome, MigrationOperation,
    SchemaSnapshot,
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
    /// Erzeugt ein eigenständiges Ferrum-Projekt.
    New {
        name: String,
        /// `api` oder `fullstack`. Standard ist `fullstack`.
        #[arg(long, default_value = "fullstack")]
        template: String,
        /// Pfad zum Ferrum-Repository, solange Ferrum nicht veröffentlicht ist.
        #[arg(long)]
        ferrum_path: Option<PathBuf>,
    },
    /// Prüft Werkzeuge, Konfiguration und Projektdateien.
    Doctor,
    /// Prüft Konfiguration, Migrationen und Kompilierung, ohne Daten zu ändern.
    Check,
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
}

#[derive(Subcommand)]
enum GenerateCommand {
    /// Legt ein Modell an und trägt es in die Registry ein.
    Model { name: String },
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
            ferrum_path,
        } => match scaffold::new_project(&name, &template, ferrum_path.as_deref()) {
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
        Command::Check => diagnose::check(),
        Command::Dev => develop::dev(),
        Command::Build => develop::build(),
        Command::Generate { action } => match action {
            GenerateCommand::Model { name } => match models::generate_model(&name) {
                Ok(path) => {
                    println!("{} {}", text::line("model_created"), path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            },
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
                let db = ferrum_db::connect(&url)
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
                let db = ferrum_db::connect(&url)
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
            let (auth_applied, report) = runtime.block_on(async {
                let db = ferrum_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth_applied = ferrum_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let report = apply_directory(&db, &directory, allow_destructive)
                    .await
                    .map_err(|error| error.0)?;
                Ok::<_, String>((auth_applied, report))
            })?;
            if auth_applied.is_empty() {
                println!("Keine ausstehenden Auth-Migrationen.");
            } else {
                for id in auth_applied {
                    println!("Auth-Migration angewendet: {id}");
                }
            }
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

fn print_risks(risks: &[ferrum_migrations::MigrationRisk]) {
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
        println!("Löschungen führt ferrum migrate up erst mit --allow-destructive aus.");
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
                let db = ferrum_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                ferrum_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth = ferrum_auth::AuthService::new(db, ferrum_auth::AuthConfig::from_env());
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

async fn open_auth(url: &str) -> Result<ferrum_auth::AuthService, String> {
    let db = ferrum_db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    ferrum_auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ferrum_auth::AuthService::new(
        db,
        ferrum_auth::AuthConfig::from_env(),
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

fn auth_message(error: ferrum_auth::AuthError) -> String {
    match error {
        ferrum_auth::AuthError::Validation(message) if message == "record already exists" => {
            "Der Benutzer oder die Zuordnung existiert bereits.".to_owned()
        }
        ferrum_auth::AuthError::Validation(message) if message == "user was not found" => {
            "Der Benutzer wurde nicht gefunden.".to_owned()
        }
        ferrum_auth::AuthError::Validation(message) if message == "email is invalid" => {
            "Die E-Mail-Adresse ist ungültig.".to_owned()
        }
        ferrum_auth::AuthError::Database(_) => {
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
    Ok(ferrum_db::DEFAULT_SQLITE_URL.to_owned())
}
