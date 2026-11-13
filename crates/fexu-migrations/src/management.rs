//! Projektlokale Management-Befehle.
//!
//! Die Befehle werden in das Projekt kompiliert. Die Fexu-CLI startet
//! nur die feste Binary `fexu-manage` und übergibt Argumente einzeln.
//! Sie lädt keine Plugins und führt keine Zeichenkette als Shell-Befehl aus.

use std::collections::{BTreeMap, BTreeSet};

use fexu_db::driver::DatabaseConnection;

use crate::error::MigrationError;
use crate::project::ProjectConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandError {
    pub message: String,
    pub code: u8,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl From<MigrationError> for CommandError {
    fn from(error: MigrationError) -> Self {
        Self::new(error.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArgSpec {
    pub name: &'static str,
    pub help: &'static str,
    pub value: bool,
    pub required: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    pub name: &'static str,
    pub about: &'static str,
    pub args: &'static [ArgSpec],
}

#[derive(Clone, Debug)]
pub struct Invocation<'a> {
    pub project: &'a ProjectConfig,
    pub positional: Vec<String>,
    pub options: BTreeMap<String, String>,
    pub flags: BTreeSet<String>,
}

impl<'a> Invocation<'a> {
    /// Öffnet die Projektdatenbank erst, wenn der Befehl sie braucht.
    /// `--database-url` gilt nur für diesen Aufruf und wird nicht protokolliert.
    pub async fn database(&self) -> Result<DatabaseConnection, CommandError> {
        let url = match self.options.get("database-url") {
            Some(url) if !url.trim().is_empty() => url.clone(),
            Some(_) => {
                return Err(CommandError::usage(
                    "--database-url needs a non-empty value.".to_owned(),
                ))
            }
            None => self.project.resolved_database_url(),
        };
        fexu_db::connect(&url)
            .await
            .map_err(|error| CommandError::new(error.to_string().replace(&url, "[database]")))
    }
}

#[async_trait::async_trait]
pub trait ManagementCommand: Send + Sync {
    fn spec(&self) -> CommandSpec;
    async fn run(&self, invocation: &Invocation<'_>) -> Result<(), CommandError>;
}

pub struct Registry {
    commands: Vec<Box<dyn ManagementCommand>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        command: impl ManagementCommand + 'static,
    ) -> Result<(), CommandError> {
        let name = command.spec().name;
        if !is_command_name(name) {
            return Err(CommandError::usage(format!(
                "Command name '{name}' must be a lowercase identifier such as seed-shop."
            )));
        }
        if self
            .commands
            .iter()
            .any(|existing| existing.spec().name == name)
        {
            return Err(CommandError::usage(format!(
                "Command '{name}' is already registered."
            )));
        }
        self.commands.push(Box::new(command));
        Ok(())
    }

    pub fn help(&self) -> String {
        let mut lines = vec![
            "Project management commands".to_owned(),
            String::new(),
            "Run one command with `fexu manage <command>`.".to_owned(),
            "A command opens the database only when its code asks for a connection.".to_owned(),
            String::new(),
        ];
        for command in &self.commands {
            let spec = command.spec();
            lines.push(format!("  {:<16} {}", spec.name, spec.about));
            for arg in spec.args {
                let form = if arg.value {
                    format!("--{} <value>", arg.name)
                } else {
                    format!("--{}", arg.name)
                };
                let required = if arg.required { " required" } else { "" };
                lines.push(format!("    {form:<18} {}{required}", arg.help));
            }
        }
        lines.join("\n")
    }

    pub async fn execute(&self, project: &ProjectConfig, args: &[String]) -> u8 {
        if args.is_empty() || args == ["--help"] || args == ["-h"] {
            println!("{}", self.help());
            return 0;
        }
        let name = &args[0];
        if !is_command_name(name) {
            eprintln!(
                "Command names are lowercase identifiers such as seed-shop. Nothing was run."
            );
            return 2;
        }
        let Some(command) = self
            .commands
            .iter()
            .find(|command| command.spec().name == name)
        else {
            eprintln!("Unknown command '{name}'.");
            println!("{}", self.help());
            return 2;
        };
        if args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
        {
            println!("{}", command_help(&command.spec()));
            return 0;
        }
        let parsed = match parse_args(&command.spec(), &args[1..]) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("{error}");
                return error.code;
            }
        };
        let invocation = Invocation {
            project,
            positional: parsed.positional,
            options: parsed.options,
            flags: parsed.flags,
        };
        match command.run(&invocation).await {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                error.code
            }
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

fn command_help(spec: &CommandSpec) -> String {
    let mut lines = vec![format!("{}  {}", spec.name, spec.about), String::new()];
    if spec.args.is_empty() {
        lines.push("This command takes no options.".to_owned());
    }
    for arg in spec.args {
        let form = if arg.value {
            format!("--{} <value>", arg.name)
        } else {
            format!("--{}", arg.name)
        };
        lines.push(format!("{form}  {}", arg.help));
    }
    lines.push(
        "--database-url <url>  Uses this database for the command instead of fexu.toml.".to_owned(),
    );
    lines.join("\n")
}

struct ParsedArgs {
    positional: Vec<String>,
    options: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

fn parse_args(spec: &CommandSpec, args: &[String]) -> Result<ParsedArgs, CommandError> {
    let mut positional = Vec::new();
    let mut options = BTreeMap::new();
    let mut flags = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            positional.extend(args[index + 1..].iter().cloned());
            break;
        }
        if let Some(name) = arg.strip_prefix("--") {
            let (name, inline) = name
                .split_once('=')
                .map(|(name, value)| (name, Some(value.to_owned())))
                .unwrap_or((name, None));
            if name == "database-url" {
                let value = take_value(name, inline, args, &mut index)?;
                insert_option(&mut options, name, value)?;
                continue;
            }
            let declared = spec
                .args
                .iter()
                .find(|candidate| candidate.name == name)
                .ok_or_else(|| {
                    CommandError::usage(format!("Unknown option '--{name}'. Nothing was run."))
                })?;
            if declared.value {
                let value = take_value(name, inline, args, &mut index)?;
                insert_option(&mut options, name, value)?;
            } else {
                if inline.is_some() {
                    return Err(CommandError::usage(format!(
                        "Flag '--{name}' does not take a value."
                    )));
                }
                if !flags.insert(name.to_owned()) {
                    return Err(CommandError::usage(format!(
                        "Flag '--{name}' was given twice."
                    )));
                }
                index += 1;
            }
        } else if arg.starts_with('-') {
            return Err(CommandError::usage(format!(
                "Unknown option '{arg}'. Nothing was run."
            )));
        } else {
            positional.push(arg.clone());
            index += 1;
        }
    }
    if !positional.is_empty() {
        return Err(CommandError::usage(
            "This command uses named options. Positional arguments were not run.".to_owned(),
        ));
    }
    for arg in spec.args {
        if arg.required && arg.value && !options.contains_key(arg.name) {
            return Err(CommandError::usage(format!(
                "Missing required option '--{}'. Nothing was run.",
                arg.name
            )));
        }
        if arg.required && !arg.value && !flags.contains(arg.name) {
            return Err(CommandError::usage(format!(
                "Missing required flag '--{}'. Nothing was run.",
                arg.name
            )));
        }
    }
    Ok(ParsedArgs {
        positional,
        options,
        flags,
    })
}

fn take_value(
    name: &str,
    inline: Option<String>,
    args: &[String],
    index: &mut usize,
) -> Result<String, CommandError> {
    if let Some(value) = inline {
        *index += 1;
        if value.is_empty() {
            return Err(CommandError::usage(format!(
                "Option '--{name}' needs a value."
            )));
        }
        return Ok(value);
    }
    let value = args
        .get(*index + 1)
        .ok_or_else(|| CommandError::usage(format!("Option '--{name}' needs a value.")))?;
    if value.starts_with('-') {
        return Err(CommandError::usage(format!(
            "Option '--{name}' needs a value."
        )));
    }
    *index += 2;
    Ok(value.clone())
}

fn insert_option(
    options: &mut BTreeMap<String, String>,
    name: &str,
    value: String,
) -> Result<(), CommandError> {
    if options.insert(name.to_owned(), value).is_some() {
        return Err(CommandError::usage(format!(
            "Option '--{name}' was given twice."
        )));
    }
    Ok(())
}

pub fn is_command_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-')
        && (1..=64).contains(&name.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fexu_db::driver::ConnectionTrait;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn project(url: &str) -> ProjectConfig {
        ProjectConfig {
            root: PathBuf::from("."),
            migrations_dir: PathBuf::from("migrations"),
            schema_command: None,
            name: Some("test".into()),
            template: None,
            layout_version: 1,
            apps_dir: "apps".into(),
            default_locale: "de".into(),
            fallback_locale: "en".into(),
            database_url: url.into(),
            host: "127.0.0.1".into(),
            port: 3000,
            allow_wildcard_bind: false,
            debug: true,
            admin_enabled: false,
            embedded_assets: false,
            site_title: String::new(),
            site_header: String::new(),
            auth_enabled: false,
            session_hours: 1,
            idle_minutes: 1,
            templates_enabled: false,
            template_engine: "tera".into(),
            templates_dir: "templates".into(),
            app_name: "test".into(),
            primary_color: "#000".into(),
            currency: "EUR".into(),
            static_enabled: false,
            static_url_prefix: "/static".into(),
            static_directory: "static".into(),
            static_cache_seconds: 0,
            storage_backend: "local".into(),
            storage_directory: "media".into(),
            storage_max_upload_mb: 1,
            mail_backend: "disabled".into(),
            mail_host: String::new(),
            mail_port: 25,
            mail_security: "none".into(),
            mail_from: String::new(),
            mail_reply_to: String::new(),
            mail_timeout_secs: 1,
            mail_public_origin: String::new(),
            mail_reset_ttl_minutes: 1,
            jobs_enabled: false,
            jobs_poll_ms: 1,
            jobs_concurrency: 1,
            jobs_lease_secs: 1,
            jobs_max_attempts: 1,
            jobs_retention_hours: 1,
            jobs_jitter: false,
            custom: BTreeMap::new(),
            apps: Vec::new(),
        }
    }

    struct Remember(Arc<AtomicBool>);

    #[async_trait::async_trait]
    impl ManagementCommand for Remember {
        fn spec(&self) -> CommandSpec {
            CommandSpec {
                name: "remember",
                about: "Stores one note.",
                args: &[ArgSpec {
                    name: "note",
                    help: "Note text.",
                    value: true,
                    required: true,
                }],
            }
        }

        async fn run(&self, invocation: &Invocation<'_>) -> Result<(), CommandError> {
            self.0.store(true, Ordering::SeqCst);
            let db = invocation.database().await?;
            let backend = db.get_database_backend();
            db.execute(fexu_db::driver::Statement::from_string(
                backend,
                "CREATE TABLE IF NOT EXISTS markers (id TEXT PRIMARY KEY, note TEXT NOT NULL)",
            ))
            .await
            .map_err(|error| CommandError::new(error.to_string()))?;
            let note = invocation.options.get("note").cloned().unwrap_or_default();
            db.execute(fexu_db::driver::Statement::from_sql_and_values(
                backend,
                "INSERT INTO markers (id, note) VALUES (?, ?)",
                vec![
                    fexu_db::driver::Value::String(Some(Box::new("one".into()))),
                    fexu_db::driver::Value::String(Some(Box::new(note))),
                ],
            ))
            .await
            .map_err(|error| CommandError::new(error.to_string()))?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn registry_rejects_conflicts_unknown_commands_and_runs_async() {
        let hit = Arc::new(AtomicBool::new(false));
        let mut registry = Registry::new();
        registry.register(Remember(hit.clone())).unwrap();
        assert!(registry.register(Remember(hit.clone())).is_err());
        assert!(registry.help().contains("remember"));

        let file = std::env::temp_dir().join(format!(
            "fexu-manage-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let url = format!(
            "sqlite://{}?mode=rwc",
            file.display().to_string().replace('\\', "/")
        );
        let project = project(&url);
        let missing = registry.execute(&project, &["remember".into()]).await;
        assert_eq!(missing, 2);
        assert!(!hit.load(Ordering::SeqCst));

        let unknown = registry.execute(&project, &["missing".into()]).await;
        assert_eq!(unknown, 2);
        assert!(!hit.load(Ordering::SeqCst));

        let unsafe_name = registry.execute(&project, &["../secret".into()]).await;
        assert_eq!(unsafe_name, 2);

        let code = registry
            .execute(
                &project,
                &[
                    "remember".into(),
                    "--note".into(),
                    "stored".into(),
                    "--database-url".into(),
                    url.clone(),
                ],
            )
            .await;
        assert_eq!(code, 0);
        assert!(hit.load(Ordering::SeqCst));

        let db = fexu_db::connect(&url).await.unwrap();
        let rows = db
            .query_all(fexu_db::driver::Statement::from_string(
                fexu_db::driver::DatabaseBackend::Sqlite,
                "SELECT note FROM markers",
            ))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        drop(db);
        let _ = std::fs::remove_file(file);
    }
}
