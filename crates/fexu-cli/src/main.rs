//! Fexu CLI. Migrations obtain the model registry through fexu.toml.

mod apps;
mod data;
mod develop;
mod diagnose;
mod jobs;
mod manage;
mod models;
mod scaffold;
mod text;

use clap::{Parser, Subcommand};
use fexu_migrations::{
    apply_directory, apply_packages, baseline_database, discover_project, inspect_framework,
    load_schema_from_command, make_migration_with_backfills, plan_database, readonly_database_url,
    render_diagnosis, reseal_unapplied, resolve_installed_schemas, runtime_contract_note,
    write_explicit_migration, BackfillLiteral, ColumnBackfill, InstalledApp, MakeOutcome,
    MigrationOperation, PackageMigration, SchemaSnapshot,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "fexu", about = "Fexu development tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Creates a project or an app inside an existing project.
    New {
        #[command(subcommand)]
        action: NewCommand,
    },
    /// Checks tools, configuration, and project files.
    Doctor,
    /// Checks configuration, migrations, and compilation without changing data.
    Check {
        /// Warnings, pending migrations, and legacy projects result in exit code 2.
        #[arg(long)]
        strict: bool,
        /// Read-only deployment checks. Does not migrate, write files, or print secret values.
        #[arg(long)]
        deploy: bool,
    },
    /// Inspects an existing project and writes at most one missing legacy manifest.
    Upgrade {
        /// Diagnostics only. No files are written.
        #[arg(long)]
        check: bool,
        /// Shows planned file changes without writing files.
        #[arg(long)]
        dry_run: bool,
        /// Creates a missing legacy manifest without overwriting project files.
        #[arg(long)]
        yes: bool,
    },
    /// Starts the development server and restarts it when Rust files change.
    Dev,
    /// Builds the project. Fullstack embeds the admin frontend.
    Build,
    /// Generates source files.
    Generate {
        #[command(subcommand)]
        action: GenerateCommand,
    },
    /// Versioned database schema migrations.
    Migrate {
        #[command(subcommand)]
        action: MigrateCommand,
    },
    /// Creates a user and securely prompts for a password.
    Admin {
        #[command(subcommand)]
        action: AdminCommand,
    },
    /// Users, roles, and permissions.
    Auth {
        #[command(subcommand)]
        action: AuthCommand,
    },
    /// Starts the job worker without running the web server.
    Worker {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        concurrency: Option<u16>,
        #[arg(long)]
        poll_ms: Option<u64>,
    },
    /// Lists and manages persistent jobs.
    Jobs {
        #[command(subcommand)]
        action: JobsCommand,
    },
    /// Writes selected database rows as a versioned JSON fixture.
    Dumpdata {
        /// Writes this table only. Dependencies are not added automatically.
        #[arg(long)]
        model: Option<String>,
        /// Uses the schema of one declared layout-2 app.
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        database_url: Option<String>,
        /// Replaces this file. Standard output is used when the flag is absent.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Inserts a versioned JSON fixture. Existing rows are left unchanged.
    Loaddata {
        file: PathBuf,
        /// Checks the file and references without writing.
        #[arg(long)]
        dry_run: bool,
        /// Updates rows whose primary key already exists.
        #[arg(long)]
        upsert: bool,
        /// Deletes the selected tables before inserting. Requires --yes.
        #[arg(long)]
        truncate: bool,
        /// Confirms --truncate. Without it, truncate does not open a write.
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Runs a command compiled into this project's fexu-manage binary.
    Manage {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum NewCommand {
    /// Creates an empty standalone project, without business models.
    Project {
        name: String,
        #[arg(long, default_value = "fullstack", value_parser = ["api", "fullstack"])]
        template: String,
        /// App modules live in apps/<name> or directly in the project root.
        #[arg(long, default_value = "apps", value_parser = ["apps", "."])]
        apps_dir: String,
        #[arg(long)]
        fexu_path: Option<PathBuf>,
    },
    /// Creates and registers an empty app in a layout-2 project.
    App {
        name: String,
        /// Place the initial HTML template inside the app or in templates/<name>.
        #[arg(long, default_value = "local", value_parser = ["local", "shared"])]
        templates: String,
    },
    #[command(external_subcommand)]
    Legacy(Vec<String>),
}

#[derive(Subcommand)]
enum JobsCommand {
    /// Lists jobs without modifying them.
    List {
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Inspects a job. The payload stays hidden unless `--show-payload` is supplied.
    Inspect {
        id: String,
        /// Displays the stored payload, which may contain personal data.
        #[arg(long)]
        show_payload: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Retries a failed or cancelled job.
    Retry {
        id: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Cancels a queued job.
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
    /// Creates a model and registers it.
    Model {
        name: String,
        /// Required for models in layout-2 projects.
        #[arg(long)]
        app: Option<String>,
        /// Adds managed created_at and updated_at fields.
        #[arg(long)]
        timestamps: bool,
    },
    /// Removed command: use `fexu new app <name>`.
    App { name: String },
    /// Writes `src/bin/worker.rs` without overwriting existing files.
    Worker,
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Creates a user. There is no default administrator password.
    CreateUser {
        /// Login email address. Prompt if omitted.
        #[arg(long)]
        email: Option<String>,
        /// Grants admin access. Superusers also need this flag.
        #[arg(long)]
        staff: bool,
        /// Grants model permissions but does not grant staff or admin access.
        #[arg(long)]
        superuser: bool,
        #[arg(long)]
        database_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Lists users without exposing password hashes.
    ListUsers {
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Creates a role or reuses an existing one.
    CreateRole {
        name: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Grants `resource.read|create|update|delete` to a role.
    GrantPermission {
        code: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Revokes a permission from a role.
    RevokePermission {
        code: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Assigns a role to a user.
    AssignRole {
        #[arg(long)]
        email: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Revokes a role from a user.
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
    /// Creates a migration from the registered model schema.
    Make {
        /// Short name, for example add_note.
        name: Option<String>,
        /// Use the declared app's schema and migration directory.
        #[arg(long, conflicts_with_all = ["schema_file", "migrations_dir"])]
        app: Option<String>,
        /// Schema snapshot JSON instead of schema_command.
        #[arg(long)]
        schema_file: Option<PathBuf>,
        /// Explicit value for a new NOT NULL column, for example products.stock:i64=0.
        #[arg(long = "backfill")]
        backfill: Vec<String>,
        /// Overrides migrations_dir from fexu.toml.
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
    },
    /// Applies pending migrations.
    Up {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        /// Explicitly allows destructive schema changes.
        #[arg(long)]
        allow_destructive: bool,
    },
    /// Reverts the latest applied app migration.
    Down {
        #[arg(long)]
        database_url: Option<String>,
        /// Reverts the latest migration of this declared app.
        #[arg(long, conflicts_with = "migrations_dir")]
        app: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        /// Allows dropping a table or column created by that migration.
        #[arg(long)]
        allow_destructive: bool,
        /// Prints the inverse plan without changing the database.
        #[arg(long)]
        dry_run: bool,
    },
    /// Compares migration files, history, and the live schema without writing.
    Plan {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
    },
    /// Shows database backend, files, history, conflicts, and schema drift.
    Status {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
    },
    /// Creates an explicit column rename. The registered schema must already use the new name.
    RenameColumn {
        #[arg(long)]
        table: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        /// Short file name. Defaults to rename_<target>.
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        schema_file: Option<PathBuf>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
    },
    /// Creates an explicit table rename. The registered schema must already use the new name.
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
    /// Recalculates the checksum of a migration that has not been applied.
    Reseal {
        path: PathBuf,
        #[arg(long)]
        database_url: Option<String>,
    },
    /// Baselines existing SQLite migrations only when the schema matches exactly. Requires --confirm to write.
    Baseline {
        #[arg(long)]
        database_url: Option<String>,
        #[arg(long)]
        migrations_dir: Option<PathBuf>,
        /// Writes migration history only after successful verification.
        #[arg(long)]
        confirm: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::New { action } => {
            let (result, label) = match action {
                NewCommand::Project { name, template, apps_dir, fexu_path } => (
                    scaffold::new_project(&name, &template, &apps_dir, fexu_path.as_deref()),
                    text::line("project_created"),
                ),
                NewCommand::App { name, templates } => (apps::generate_app(&name, &templates), "Created app:"),
                NewCommand::Legacy(arguments) => (Err(format!("The implicit new command ({} arguments) was removed. Use fexu new project <name> or fexu new app <name>. Existing projects are not changed.", arguments.len())), ""),
            };
            match result {
                Ok(path) => {
                    println!("{label} {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Doctor => diagnose::doctor(),
        Command::Check { strict, deploy } => diagnose::check(strict, deploy),
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
                    println!("Worker created: {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            },
            GenerateCommand::App { name: _ } => {
                eprintln!("Use `fexu new app <name>`. `generate app` no longer writes files. See docs/project-generator.md for existing projects.");
                ExitCode::from(2)
            }
            GenerateCommand::Model {
                name,
                app,
                timestamps,
            } => match models::generate_model(&name, app.as_deref(), timestamps) {
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
                eprintln!("job.disabled jobs.enabled is false.");
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
        Command::Dumpdata {
            model,
            app,
            schema_file,
            database_url,
            output,
        } => match data::dumpdata(model, app, schema_file, database_url, output) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(error.code())
            }
        },
        Command::Loaddata {
            file,
            dry_run,
            upsert,
            truncate,
            yes,
            model,
            app,
            schema_file,
            database_url,
        } => match data::loaddata(data::LoadOptions {
            file,
            dry_run,
            upsert,
            truncate,
            yes,
            model,
            app,
            schema_file,
            database_url,
        }) {
            Ok(message) => {
                println!("{message}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(error.code())
            }
        },
        Command::Manage { args } => manage::manage(args),
    }
}

fn run_migrate(action: MigrateCommand) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    match action {
        MigrateCommand::Make {
            name,
            app,
            schema_file,
            migrations_dir,
            backfill,
        } => {
            let (directory, schema) = match app {
                Some(name) => resolve_app_schema(&name)?,
                None => resolve_schema(schema_file, migrations_dir)?,
            };
            let backfills = backfill
                .iter()
                .map(|value| parse_backfill(value))
                .collect::<Result<Vec<_>, _>>()?;
            match make_migration_with_backfills(
                &directory,
                name.as_deref().unwrap_or("change"),
                &schema,
                &backfills,
            )
            .map_err(|error| error.0)?
            {
                MakeOutcome::Unchanged => {
                    println!("No schema changes. No migration was written.");
                }
                MakeOutcome::Written { path, id, risks } => {
                    println!("Migration written: {} ({id})", path.display());
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
                let db = fexu_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                reseal_unapplied(&db, &path).await.map_err(|error| error.0)
            })?;
            println!(
                "Checksum updated for {}. Applied migrations remain unchanged.",
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
                let db = fexu_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                baseline_database(&db, &directory, confirm)
                    .await
                    .map_err(|error| error.0)
            })?;
            println!("Baseline written. No tables or data were changed.");
            for id in ids {
                println!("Marked as applied: {id}");
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
                let db = fexu_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth_applied = fexu_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let jobs_applied = fexu_jobs::apply_migrations(&db)
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
                println!("No pending authentication migrations.");
            } else {
                for id in auth_applied {
                    println!("Authentication migration applied: {id}");
                }
            }
            if jobs_applied.is_empty() {
                println!("No pending job migrations.");
            } else {
                for id in jobs_applied {
                    println!("Job migration applied: {id}");
                }
            }
            if report.applied.is_empty() {
                println!("No pending migrations.");
            } else {
                for id in report.applied {
                    println!("Applied: {id}");
                }
            }
            if package_report.is_empty() {
                println!("No pending app migrations.");
            } else {
                for id in package_report {
                    println!("App migration applied: {id}");
                }
            }
        }
        MigrateCommand::Down {
            database_url,
            app,
            migrations_dir,
            allow_destructive,
            dry_run,
        } => {
            let (directory, namespace) = match app {
                Some(name) => (resolve_app_directory(&name)?, Some(name)),
                None => (resolve_directory(migrations_dir)?, None),
            };
            let url = resolve_database_url(database_url)?;
            let plan = runtime.block_on(async {
                let db = fexu_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                match namespace {
                    Some(name) => fexu_migrations::revert_namespaced(
                        &db,
                        &directory,
                        &name,
                        allow_destructive,
                        dry_run,
                    )
                    .await
                    .map_err(|error| error.0),
                    None => {
                        fexu_migrations::revert_latest(&db, &directory, allow_destructive, dry_run)
                            .await
                            .map_err(|error| error.0)
                    }
                }
            })?;
            if dry_run {
                println!("Dry run. No tables or rows were changed.");
            } else if plan.executed {
                println!("Reverted: {}", plan.id);
            }
            for step in plan.steps {
                println!("Inverse: {step}");
            }
            if plan.destructive {
                println!(
                    "This revert drops a table or column. Data in those objects is not restored."
                );
            }
        }
        MigrateCommand::Plan {
            database_url,
            migrations_dir,
            schema_file,
        } => {
            let directory = resolve_directory(migrations_dir)?;
            let schema = load_optional_schema(schema_file)?;
            let url = resolve_database_url(database_url)?;
            let readonly = readonly_database_url(&url).map_err(|error| error.0)?;
            let report = runtime.block_on(async {
                let db = fexu_db::connect(&readonly)
                    .await
                    .map_err(|error| format!("migration.database.unavailable: {error}"))?;
                plan_database(&db, &directory, schema.as_ref())
                    .await
                    .map_err(|error| error.0)
            })?;
            println!("Migration plan is read-only.");
            println!("backend: {}", report.backend);
            if report.pending.is_empty() {
                println!("pending: none");
            } else {
                for id in &report.pending {
                    println!("pending: {id}");
                }
            }
            for finding in &report.findings {
                println!("{}: {}", finding.level, finding.message);
            }
            if report.has_error() || report.has_drift() {
                return Err("Migration plan found conflicts or live schema drift.".to_owned());
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
                let db = fexu_db::connect(&readonly)
                    .await
                    .map_err(|error| format!("migration.database.unavailable: {error}"))?;
                let auth_files =
                    fexu_auth::embedded_migrations().map_err(|error| error.to_string())?;
                let jobs_files = fexu_jobs::embedded_migrations()
                    .map_err(|error| format!("{} {}", error.code(), error.detail()))?;
                inspect_framework(
                    &db,
                    &auth_files,
                    &jobs_files,
                    fexu_jobs::JOB_HISTORY,
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
                return Err("Migration conflicts were detected.".to_owned());
            }
            let jobs_current = runtime.block_on(async {
                let db = fexu_db::connect(&readonly)
                    .await
                    .map_err(|error| format!("migration.database.unavailable: {error}"))?;
                jobs::print_status(&db).await
            })?;
            if !jobs_current {
                println!("Job migrations are not fully applied. Run fexu migrate up. This status check is read-only.");
            }
        }
    }
    Ok(())
}

pub(crate) fn optional_project() -> Result<Option<fexu_migrations::ProjectConfig>, String> {
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    match fexu_migrations::discover_project(&directory) {
        Ok(project) => Ok(Some(project)),
        Err(error) if error.0.contains("No fexu.toml found") => Ok(None),
        Err(error) => Err(error.0),
    }
}

pub(crate) fn jobs_database_url(
    override_url: Option<String>,
    project: Option<&fexu_migrations::ProjectConfig>,
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
            "job.database No database URL was provided. Set DATABASE_URL or use --database-url."
                .to_owned(),
        ),
    }
}

fn parse_backfill(text: &str) -> Result<ColumnBackfill, String> {
    let (path, value) = text.split_once('=').ok_or(
        "A backfill must look like table.column:i64=0, table.column:text=value, or table.column:bool=true.",
    )?;
    let (path, kind) = path.rsplit_once(':').ok_or(
        "A backfill must name the type: table.column:i64=0, table.column:text=value, or table.column:bool=true.",
    )?;
    let (table, column) = path
        .split_once('.')
        .ok_or("A backfill must name table.column before the type.")?;
    if table.is_empty() || column.is_empty() {
        return Err("A backfill needs both a table and a column.".to_owned());
    }
    let parsed = match kind {
        "bool" => BackfillLiteral::Bool(
            value
                .parse::<bool>()
                .map_err(|_| format!("Backfill `{text}` needs a bool value of true or false."))?,
        ),
        "i64" => BackfillLiteral::I64(
            value
                .parse::<i64>()
                .map_err(|_| format!("Backfill `{text}` needs a base-10 integer."))?,
        ),
        "text" => BackfillLiteral::Text(value.to_owned()),
        _ => {
            return Err(format!(
                "Backfill `{text}` must use bool, i64, or text. No value was invented."
            ))
        }
    };
    Ok(ColumnBackfill {
        table: table.to_owned(),
        column: column.to_owned(),
        value: parsed,
    })
}

fn print_written(outcome: MakeOutcome) {
    match outcome {
        MakeOutcome::Unchanged => {
            println!("No schema changes. No migration was written.");
        }
        MakeOutcome::Written { path, id, risks } => {
            println!("Migration written: {} ({id})", path.display());
            print_risks(&risks);
        }
    }
}

fn print_risks(risks: &[fexu_migrations::MigrationRisk]) {
    if risks.is_empty() {
        println!("Risks: none");
        return;
    }
    println!("Risks:");
    for risk in risks {
        println!("  {}: {}", risk.code, risk.message);
    }
    let destructive = risks
        .iter()
        .any(|risk| risk.code == "drop_table" || risk.code == "drop_column");
    if destructive {
        println!("Destructive changes require fexu migrate up --allow-destructive.");
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
        Err(error) if error.0.contains("No fexu.toml found") => Ok(Vec::new()),
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

fn resolve_app_directory(name: &str) -> Result<PathBuf, String> {
    apps::validate_app_name(name)?;
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    project
        .apps
        .iter()
        .find(|app| app.name == name)
        .map(|app| app.migrations.clone())
        .ok_or_else(|| format!("App `{name}` is not declared in fexu.toml."))
}

pub(crate) fn resolve_app_schema(name: &str) -> Result<(PathBuf, SchemaSnapshot), String> {
    apps::validate_app_name(name)?;
    let project = discover_project(&std::env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.0)?;
    if project.layout_version != 2 {
        return Err("--app schema selection requires a layout-2 project. Existing projects can use --schema-file and --migrations-dir explicitly.".to_owned());
    }
    if !project.apps.iter().any(|app| app.name == name) {
        return Err(format!("App `{name}` is not declared in fexu.toml."));
    }
    let command = project
        .schema_command
        .as_ref()
        .ok_or("schema_command is missing")?;
    let mut installed = Vec::with_capacity(project.apps.len());
    for app in &project.apps {
        let schema =
            load_schema_from_command(&project.root, &format!("{command} --app {}", app.name))
                .map_err(|error| error.0)?;
        installed.push(InstalledApp {
            name: app.name.clone(),
            depends_on: app.depends_on.clone(),
            schema,
        });
    }
    let resolved = resolve_installed_schemas(&installed).map_err(|error| error.0)?;
    let app = resolved
        .into_iter()
        .find(|app| app.name == name)
        .expect("declared app is present after resolution");
    let directory = project
        .apps
        .iter()
        .find(|candidate| candidate.name == name)
        .expect("app was declared")
        .migrations
        .clone();
    Ok((directory, app.schema))
}

pub(crate) fn resolve_schema(
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
        "fexu.toml does not define schema_command. Supply --schema-file.".to_owned()
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
                None => prompt_line("Email: ")?,
            };
            let password = match fexu_core::aliased_env("FEXU_PASSWORD") {
                Err(error) => return Err(error),
                Ok(Some(value)) if value.len() < 12 => {
                    return Err(
                        "FEXU_PASSWORD must contain at least 12 characters. There is no default password."
                            .to_owned(),
                    );
                }
                Ok(Some(value)) => value,
                Ok(None) => {
                    let password = prompt_secret("Password: ")?;
                    let confirm = prompt_secret("Confirm password: ")?;
                    if password != confirm {
                        return Err("Passwords do not match.".to_owned());
                    }
                    password
                }
            };
            let url = resolve_database_url(database_url)?;
            let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
            let user = runtime.block_on(async {
                let db = fexu_db::connect(&url)
                    .await
                    .map_err(|error| error.to_string())?;
                fexu_auth::apply_migrations(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let auth = fexu_auth::AuthService::new(db, fexu_auth::AuthConfig::from_env());
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
            println!("Role ready: {name} ({id})");
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
            println!("Granted permission {code} to role {role}.");
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
            println!("Revoked permission {code} from role {role}.");
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
            println!("Assigned role {role} to {email}.");
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
            println!("Revoked role {role} from {email}.");
        }
    }
    Ok(())
}

async fn open_auth(url: &str) -> Result<fexu_auth::AuthService, String> {
    let db = fexu_db::connect(url)
        .await
        .map_err(|error| error.to_string())?;
    fexu_auth::apply_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(fexu_auth::AuthService::new(
        db,
        fexu_auth::AuthConfig::from_env(),
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

fn auth_message(error: fexu_auth::AuthError) -> String {
    match error {
        fexu_auth::AuthError::Validation(message) if message == "record already exists" => {
            "The user or assignment already exists.".to_owned()
        }
        fexu_auth::AuthError::Validation(message) if message == "user was not found" => {
            "User not found.".to_owned()
        }
        fexu_auth::AuthError::Validation(message) if message == "email is invalid" => {
            "Invalid email address.".to_owned()
        }
        fexu_auth::AuthError::Database(_) => "Database operation failed.".to_owned(),
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
    Ok(fexu_db::DEFAULT_SQLITE_URL.to_owned())
}
