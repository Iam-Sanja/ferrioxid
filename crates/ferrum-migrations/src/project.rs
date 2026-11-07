//! Explizite Projektintegration. Die CLI liest keine Rust-Quellen ein.

use crate::error::MigrationError;
use crate::snapshot::SchemaSnapshot;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub migrations_dir: PathBuf,
    pub schema_command: Option<String>,
    pub name: Option<String>,
    pub template: Option<String>,
    pub default_locale: String,
    pub fallback_locale: String,
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub admin_enabled: bool,
    pub embedded_assets: bool,
    pub site_title: String,
    pub site_header: String,
    pub auth_enabled: bool,
    pub session_hours: u32,
    pub idle_minutes: u32,
    pub templates_enabled: bool,
    pub template_engine: String,
    pub templates_dir: String,
    pub app_name: String,
    pub primary_color: String,
    pub currency: String,
    pub static_enabled: bool,
    pub static_url_prefix: String,
    pub static_directory: String,
    pub static_cache_seconds: u64,
    pub storage_backend: String,
    pub storage_directory: String,
    pub storage_max_upload_mb: u64,
    pub mail_backend: String,
    pub mail_host: String,
    pub mail_port: u16,
    pub mail_security: String,
    pub mail_from: String,
    pub mail_reply_to: String,
    pub mail_timeout_secs: u64,
    pub mail_public_origin: String,
    pub mail_reset_ttl_minutes: u32,
    pub custom: BTreeMap<String, BTreeMap<String, String>>,
}

impl std::fmt::Debug for ProjectConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectConfig")
            .field("root", &self.root)
            .field("name", &self.name)
            .field("template", &self.template)
            .field("database_url", &redact_database_url(&self.database_url))
            .field("host", &self.host)
            .field("port", &self.port)
            .field("debug", &self.debug)
            .field("custom", &self.custom)
            .finish_non_exhaustive()
    }
}

impl ProjectConfig {
    /// Dateipfad der Vorlagen. Er bleibt relativ zur Projektdatei.
    pub fn templates_directory(&self) -> PathBuf {
        self.root.join(&self.templates_dir)
    }

    pub fn static_root(&self) -> PathBuf {
        self.root.join(&self.static_directory)
    }

    pub fn storage_root(&self) -> PathBuf {
        self.root.join(&self.storage_directory)
    }

    pub fn max_upload_bytes(&self) -> u64 {
        self.storage_max_upload_mb.saturating_mul(1024 * 1024)
    }

    /// `DATABASE_URL` ersetzt die Datei-URL, ohne sie in Protokolle zu schreiben.
    pub fn resolved_database_url(&self) -> String {
        match std::env::var("DATABASE_URL") {
            Ok(value) if !value.is_empty() => value,
            _ => self.database_url.clone(),
        }
    }

    pub fn is_production(&self) -> bool {
        std::env::var("FERRUM_ENV")
            .ok()
            .is_some_and(|value| value.eq_ignore_ascii_case("production"))
    }

    /// `FERRUM_DEBUG` ersetzt `server.debug`. Ein ungültiger Wert bleibt ein Fehler.
    pub fn resolved_debug(&self) -> Result<bool, MigrationError> {
        match std::env::var("FERRUM_DEBUG") {
            Ok(value) if !value.is_empty() => parse_bool(&value).map_err(|_| {
                MigrationError::new(
                    "FERRUM_DEBUG erlaubt true oder false. Der ungültige Wert wird nicht ersetzt."
                        .to_owned(),
                )
            }),
            _ => Ok(self.debug),
        }
    }

    /// `BIND_ADDRESS` ersetzt Host und Port. `0.0.0.0` bleibt abgelehnt.
    pub fn bind_address(&self) -> Result<String, MigrationError> {
        if let Ok(value) = std::env::var("BIND_ADDRESS") {
            if !value.is_empty() {
                if value.starts_with("0.0.0.0") {
                    return Err(MigrationError::new(
                        "BIND_ADDRESS darf nicht 0.0.0.0 sein.".to_owned(),
                    ));
                }
                return Ok(value);
            }
        }
        Ok(format!("{}:{}", self.host, self.port))
    }

    /// Produktion mit aktivem Debug ist ein Fehler. Es gibt keinen stillen Wechsel.
    pub fn enforce_runtime(&self) -> Result<(), MigrationError> {
        if self.is_production() && self.resolved_debug()? {
            return Err(MigrationError::new(
                "server.debug ist in Produktion nicht erlaubt.".to_owned(),
            ));
        }
        self.bind_address().map(|_| ())
    }

    pub fn custom_settings<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<T, MigrationError> {
        let section = self
            .custom
            .get(name)
            .ok_or_else(|| MigrationError::new(format!("custom.{name} ist nicht konfiguriert.")))?;
        let mut object = serde_json::Map::new();
        for (key, value) in section {
            object.insert(key.clone(), coerce_custom(value));
        }
        serde_json::from_value(serde_json::Value::Object(object)).map_err(|_| {
            MigrationError::new(format!(
                "custom.{name} passt nicht zur erwarteten Struktur."
            ))
        })
    }
}

/// Gelesene Projekteinstellungen. Kein globales Singleton.
#[derive(Clone, Debug)]
pub struct Settings {
    project: ProjectConfig,
}

impl Settings {
    pub fn discover(start: &Path) -> Result<Self, MigrationError> {
        Self::from_project(discover_project(start)?)
    }

    pub fn from_project(project: ProjectConfig) -> Result<Self, MigrationError> {
        project.enforce_runtime()?;
        Ok(Self { project })
    }

    pub fn project(&self) -> &ProjectConfig {
        &self.project
    }
}

pub fn redact_database_url(value: &str) -> String {
    if let Some(rest) = value.split_once("://").map(|(_, rest)| rest) {
        if let Some((userinfo, host)) = rest.split_once('@') {
            if userinfo.contains(':') {
                let scheme = value
                    .split_once("://")
                    .map(|(scheme, _)| scheme)
                    .unwrap_or("");
                return format!("{scheme}://[redacted]@{host}");
            }
        }
    }
    if value.to_ascii_lowercase().contains("password=") {
        return "[redacted]".to_owned();
    }
    value.to_owned()
}

pub fn discover_project(start: &Path) -> Result<ProjectConfig, MigrationError> {
    let mut current = if start.is_dir() {
        Some(start)
    } else {
        start.parent()
    };
    while let Some(directory) = current {
        let manifest = directory.join("ferrum.toml");
        if manifest.is_file() {
            return parse_manifest(&manifest);
        }
        current = directory.parent();
    }
    Err(MigrationError::new(
        "Keine ferrum.toml gefunden. Die CLI erkennt Modelle nicht aus dem Quelltext. Lege eine ferrum.toml mit migrations_dir und schema_command an oder übergib --schema-file und --migrations-dir.",
    ))
}

pub fn load_schema_from_command(
    root: &Path,
    command: &str,
) -> Result<SchemaSnapshot, MigrationError> {
    let output = Command::new(command_program())
        .args(command_args(command))
        .current_dir(root)
        .output()
        .map_err(|error| {
            MigrationError::new(format!(
                "Schema-Befehl konnte nicht gestartet werden: {error}"
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MigrationError::new(format!(
            "Schema-Befehl ist fehlgeschlagen.\n{stderr}"
        )));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_| {
        MigrationError::new("Die Modell-Registry hat kein UTF-8-Schema geliefert.".to_owned())
    })?;
    SchemaSnapshot::from_json(stdout.trim())
}

fn parse_manifest(path: &Path) -> Result<ProjectConfig, MigrationError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        MigrationError::new(format!(
            "{} konnte nicht gelesen werden: {error}",
            path.display()
        ))
    })?;
    let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut section = String::new();
    let mut migrations_dir = None;
    let mut schema_command = None;
    let mut name = None;
    let mut template = None;
    let mut database_url = ferrum_db::DEFAULT_SQLITE_URL.to_owned();
    let mut host = "127.0.0.1".to_owned();
    let mut port = 3000_u16;
    let mut admin_enabled = false;
    let mut embedded_assets = false;
    let mut default_locale = "de".to_owned();
    let mut fallback_locale = "en".to_owned();
    let mut debug = false;
    let mut site_title = "Ferrum Admin".to_owned();
    let mut site_header = "Ferrum Verwaltung".to_owned();
    let mut auth_enabled = false;
    let mut session_hours = 12_u32;
    let mut idle_minutes = 30_u32;
    let mut templates_enabled = false;
    let mut template_engine = "tera".to_owned();
    let mut templates_dir = "templates".to_owned();
    let mut app_name = None;
    let mut primary_color = "#0f172a".to_owned();
    let mut currency = "EUR".to_owned();
    let mut static_enabled = false;
    let mut static_url_prefix = "/static".to_owned();
    let mut static_directory = "static".to_owned();
    let mut static_cache_seconds = 3600_u64;
    let mut storage_backend = "local".to_owned();
    let mut storage_directory = "media".to_owned();
    let mut storage_max_upload_mb = 10_u64;
    let mut mail_backend = "disabled".to_owned();
    let mut mail_host = String::new();
    let mut mail_port = 587_u16;
    let mut mail_security = "starttls".to_owned();
    let mut mail_from = String::new();
    let mut mail_reply_to = String::new();
    let mut mail_timeout_secs = 15_u64;
    let mut mail_public_origin = String::new();
    let mut mail_reset_ttl_minutes = 30_u32;
    let mut custom: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (line_number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            if section == "custom" {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: [custom] braucht einen Namen, zum Beispiel [custom.shop].",
                    path.display()
                )));
            }
            if let Some(name) = section.strip_prefix("custom.") {
                if !valid_custom_name(name) {
                    return Err(MigrationError::new(format!(
                        "{}:{line_number}: custom.{name} ist kein gültiger Abschnitt.",
                        path.display()
                    )));
                }
            } else if !matches!(
                section.as_str(),
                "project"
                    | "database"
                    | "server"
                    | "admin"
                    | "auth"
                    | "templates"
                    | "branding"
                    | "static"
                    | "storage"
                    | "mail"
            ) {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: unbekannter Abschnitt `[{section}]`.",
                    path.display()
                )));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(MigrationError::new(format!(
                "{}:{line_number}: erwartet wird schluessel = \"wert\".",
                path.display()
            )));
        };
        let key = key.trim();
        let value = parse_value(value.trim()).ok_or_else(|| {
            MigrationError::new(format!(
                "{}:{line_number}: der Wert von `{key}` ist ungültig.",
                path.display()
            ))
        })?;
        if let Some(name) = section.strip_prefix("custom.") {
            if secret_config_key(key) {
                return Err(MigrationError::new(
                    "Secrets gehören nicht in ferrum.toml. Verwende eine Umgebungsvariable."
                        .to_owned(),
                ));
            }
            if !valid_custom_name(key) {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: custom.{name}.{key} ist kein gültiger Schlüssel.",
                    path.display()
                )));
            }
            custom
                .entry(name.to_owned())
                .or_default()
                .insert(key.to_owned(), value);
            continue;
        }
        let qualified = if section.is_empty() {
            key.to_owned()
        } else {
            format!("{section}.{key}")
        };
        match qualified.as_str() {
            "migrations_dir" | "project.migrations_dir" => {
                migrations_dir = Some(root.join(value));
            }
            "schema_command" | "project.schema_command" => schema_command = Some(value),
            "project.name" => name = Some(value),
            "project.template" => template = Some(value),
            "project.default_locale" => default_locale = value,
            "project.fallback_locale" => fallback_locale = value,
            "database.url" => database_url = value,
            "server.host" => host = value,
            "server.port" => {
                port = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: server.port muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "server.debug" => debug = parse_bool(&value)?,
            "admin.enabled" => admin_enabled = parse_bool(&value)?,
            "admin.embedded_assets" => embedded_assets = parse_bool(&value)?,
            "admin.site_title" => site_title = value,
            "admin.site_header" => site_header = value,
            "auth.enabled" => auth_enabled = parse_bool(&value)?,
            "auth.session_hours" => {
                session_hours = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: auth.session_hours muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "auth.idle_minutes" => {
                idle_minutes = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: auth.idle_minutes muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "templates.enabled" => templates_enabled = parse_bool(&value)?,
            "templates.engine" => template_engine = value,
            "templates.templates_dir" => templates_dir = value,
            "branding.app_name" => app_name = Some(value),
            "branding.primary_color" => primary_color = value,
            "branding.currency" => currency = value,
            "static.enabled" => static_enabled = parse_bool(&value)?,
            "static.url_prefix" => static_url_prefix = value,
            "static.directory" => static_directory = value,
            "static.cache_seconds" => {
                static_cache_seconds = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: static.cache_seconds muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "storage.backend" => storage_backend = value,
            "storage.directory" => storage_directory = value,
            "mail.password" | "mail.secret" | "mail.smtp_password" => {
                return Err(MigrationError::new(
                    "SMTP-Passwörter stehen nicht in ferrum.toml. Setze FERRUM_SMTP_PASSWORD."
                        .to_owned(),
                ));
            }
            "mail.backend" => mail_backend = value,
            "mail.host" => mail_host = value,
            "mail.port" => {
                mail_port = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: mail.port muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "mail.security" => mail_security = value,
            "mail.from" => mail_from = value,
            "mail.reply_to" => mail_reply_to = value,
            "mail.timeout_secs" => {
                mail_timeout_secs = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: mail.timeout_secs muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "mail.public_origin" => mail_public_origin = value,
            "mail.reset_ttl_minutes" => {
                mail_reset_ttl_minutes = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: mail.reset_ttl_minutes muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            "storage.max_upload_size_mb" => {
                storage_max_upload_mb = value.parse().map_err(|_| {
                    MigrationError::new(format!(
                        "{}:{line_number}: storage.max_upload_size_mb muss eine Zahl sein.",
                        path.display()
                    ))
                })?;
            }
            _ => {
                return Err(MigrationError::new(format!(
                    "{}:{line_number}: unbekannter Schlüssel `{qualified}`.",
                    path.display()
                )))
            }
        }
    }
    let migrations_dir = migrations_dir.ok_or_else(|| {
        MigrationError::new(format!("{} enthält kein migrations_dir.", path.display()))
    })?;
    if let Some(template) = &template {
        if template != "api" && template != "fullstack" {
            return Err(MigrationError::new(format!(
                "project.template `{template}` ist unbekannt. Erlaubt sind api und fullstack."
            )));
        }
    }
    validate_locale(&default_locale)?;
    validate_locale(&fallback_locale)?;
    if !(1..=168).contains(&session_hours) {
        return Err(MigrationError::new(
            "auth.session_hours muss zwischen 1 und 168 liegen.".to_owned(),
        ));
    }
    if !(1..=1440).contains(&idle_minutes)
        || u64::from(idle_minutes) > u64::from(session_hours) * 60
    {
        return Err(MigrationError::new(
            "auth.idle_minutes muss zwischen 1 und der Sitzungsdauer liegen.".to_owned(),
        ));
    }
    if template_engine != "tera" {
        return Err(MigrationError::new(
            "templates.engine unterstützt derzeit nur tera.".to_owned(),
        ));
    }
    if templates_dir.is_empty()
        || templates_dir.contains("..")
        || Path::new(&templates_dir).is_absolute()
    {
        return Err(MigrationError::new(
            "templates.templates_dir muss ein relativer Ordner ohne .. sein.".to_owned(),
        ));
    }
    validate_label("admin.site_title", &site_title)?;
    validate_label("admin.site_header", &site_header)?;
    let app_name = app_name.unwrap_or_else(|| name.clone().unwrap_or_else(|| "Ferrum".to_owned()));
    validate_label("branding.app_name", &app_name)?;
    if !is_hex_color(&primary_color) {
        return Err(MigrationError::new(
            "branding.primary_color muss #rgb oder #rrggbb sein.".to_owned(),
        ));
    }
    if currency.len() != 3 || !currency.chars().all(|char| char.is_ascii_uppercase()) {
        return Err(MigrationError::new(
            "branding.currency muss ein ISO-4217-Code aus drei Großbuchstaben sein.".to_owned(),
        ));
    }
    if !matches!(storage_backend.as_str(), "local") {
        return Err(MigrationError::new(
            "storage.backend unterstützt derzeit nur local.".to_owned(),
        ));
    }
    if !relative_dir(&static_directory) || !relative_dir(&storage_directory) {
        return Err(MigrationError::new(
            "static.directory und storage.directory müssen relative Ordner ohne .. sein."
                .to_owned(),
        ));
    }
    if !valid_static_prefix(&static_url_prefix) {
        return Err(MigrationError::new(
            "static.url_prefix muss mit / beginnen und darf Admin, API und Auth nicht überdecken."
                .to_owned(),
        ));
    }
    if static_cache_seconds > 2_592_000 {
        return Err(MigrationError::new(
            "static.cache_seconds darf höchstens 2592000 sein.".to_owned(),
        ));
    }
    if !(1..=100).contains(&storage_max_upload_mb) {
        return Err(MigrationError::new(
            "storage.max_upload_size_mb muss zwischen 1 und 100 liegen.".to_owned(),
        ));
    }
    validate_mail(MailFields {
        backend: &mail_backend,
        host: &mail_host,
        port: mail_port,
        security: &mail_security,
        from: &mail_from,
        reply_to: &mail_reply_to,
        timeout_secs: mail_timeout_secs,
        origin: &mail_public_origin,
        ttl_minutes: mail_reset_ttl_minutes,
    })?;
    if host == "0.0.0.0" {
        return Err(MigrationError::new(
            "server.host darf nicht 0.0.0.0 sein.".to_owned(),
        ));
    }
    Ok(ProjectConfig {
        root,
        migrations_dir,
        schema_command,
        name,
        template,
        default_locale,
        fallback_locale,
        database_url,
        host,
        port,
        debug,
        admin_enabled,
        embedded_assets,
        site_title,
        site_header,
        auth_enabled,
        session_hours,
        idle_minutes,
        templates_enabled,
        template_engine,
        templates_dir,
        app_name,
        primary_color,
        currency,
        static_enabled,
        static_url_prefix,
        static_directory,
        static_cache_seconds,
        storage_backend,
        storage_directory,
        storage_max_upload_mb,
        mail_backend,
        mail_host,
        mail_port,
        mail_security,
        mail_from,
        mail_reply_to,
        mail_timeout_secs,
        mail_public_origin,
        mail_reset_ttl_minutes,
        custom,
    })
}

struct MailFields<'a> {
    backend: &'a str,
    host: &'a str,
    port: u16,
    security: &'a str,
    from: &'a str,
    reply_to: &'a str,
    timeout_secs: u64,
    origin: &'a str,
    ttl_minutes: u32,
}

fn validate_mail(mail: MailFields<'_>) -> Result<(), MigrationError> {
    let MailFields {
        backend,
        host,
        port,
        security,
        from,
        reply_to,
        timeout_secs,
        origin,
        ttl_minutes,
    } = mail;
    if !matches!(backend, "disabled" | "console" | "memory" | "smtp") {
        return Err(MigrationError::new(
            "mail.backend erlaubt disabled, console, memory oder smtp.".to_owned(),
        ));
    }
    if !(1..=30).contains(&timeout_secs) {
        return Err(MigrationError::new(
            "mail.timeout_secs muss zwischen 1 und 30 liegen.".to_owned(),
        ));
    }
    if !(5..=1440).contains(&ttl_minutes) {
        return Err(MigrationError::new(
            "mail.reset_ttl_minutes muss zwischen 5 und 1440 liegen.".to_owned(),
        ));
    }
    if port == 0 {
        return Err(MigrationError::new(
            "mail.port muss zwischen 1 und 65535 liegen.".to_owned(),
        ));
    }
    if !reply_to.is_empty() && !plausible_email(reply_to) {
        return Err(MigrationError::new(
            "mail.reply_to muss eine E-Mail-Adresse ohne Steuerzeichen sein.".to_owned(),
        ));
    }
    if backend == "disabled" {
        return Ok(());
    }
    if !plausible_email(from) {
        return Err(MigrationError::new(
            "mail.from ist für ein aktives Backend erforderlich.".to_owned(),
        ));
    }
    if !trusted_origin(origin) {
        return Err(MigrationError::new(
            "mail.public_origin muss http://127.0.0.1, http://localhost oder https sein."
                .to_owned(),
        ));
    }
    if backend == "smtp" {
        if host.is_empty()
            || host
                .chars()
                .any(|char| char.is_whitespace() || char.is_control())
        {
            return Err(MigrationError::new(
                "mail.host ist für smtp erforderlich.".to_owned(),
            ));
        }
        if !matches!(security, "starttls" | "tls") {
            return Err(MigrationError::new(
                "mail.security erlaubt starttls oder tls. Klartext ist nicht vorgesehen."
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn plausible_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !value
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
        && value.len() <= 254
}

fn trusted_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    if rest.is_empty()
        || rest.contains('@')
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
    {
        return false;
    }
    let host = rest.split(':').next().unwrap_or("");
    match scheme {
        "http" => host == "127.0.0.1" || host == "localhost",
        "https" => {
            !host.is_empty()
                && !host
                    .chars()
                    .any(|char| char.is_control() || char.is_whitespace())
        }
        _ => false,
    }
}

fn relative_dir(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("..")
        && !Path::new(value).is_absolute()
        && !value.contains('\\')
}

fn valid_static_prefix(value: &str) -> bool {
    if !value.starts_with('/')
        || value.contains("..")
        || value.contains("//")
        || value.contains(' ')
    {
        return false;
    }
    let reserved = ["/admin", "/api", "/auth", "/healthz", "/uploads"];
    !reserved
        .iter()
        .any(|prefix| value == *prefix || value.starts_with(&format!("{prefix}/")))
}

fn validate_locale(value: &str) -> Result<(), MigrationError> {
    if value == "de" || value == "en" {
        Ok(())
    } else {
        Err(MigrationError::new(format!(
            "Sprache `{value}` ist noch nicht vorbereitet. Erlaubt sind de und en."
        )))
    }
}

fn validate_label(key: &str, value: &str) -> Result<(), MigrationError> {
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(|char| char.is_control())
    {
        return Err(MigrationError::new(format!(
            "{key} muss 1 bis 80 sichtbare Zeichen haben."
        )));
    }
    Ok(())
}

fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 6) && hex.chars().all(|char| char.is_ascii_hexdigit())
}

fn valid_custom_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 32
}

fn secret_config_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("password")
        || key.contains("secret")
        || key.contains("token")
        || key.contains("credential")
}

fn coerce_custom(value: &str) -> serde_json::Value {
    match value {
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        _ if !value.is_empty() && value.chars().all(|char| char.is_ascii_digit()) => value
            .parse::<u64>()
            .map(|number| serde_json::Value::Number(number.into()))
            .unwrap_or_else(|_| serde_json::Value::String(value.to_owned())),
        _ => serde_json::Value::String(value.to_owned()),
    }
}

fn parse_value(value: &str) -> Option<String> {
    if let Some(quoted) = unquote(value) {
        return Some(quoted.to_owned());
    }
    if value == "true" || value == "false" || value.chars().all(|char| char.is_ascii_digit()) {
        return Some(value.to_owned());
    }
    None
}

fn parse_bool(value: &str) -> Result<bool, MigrationError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MigrationError::new(format!(
            "`{value}` ist kein Wahrheitswert. Erlaubt sind true und false."
        ))),
    }
}

fn unquote(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn command_program() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "sh"
    }
}

fn command_args(command: &str) -> Vec<String> {
    if cfg!(windows) {
        vec!["/C".to_owned(), command.to_owned()]
    } else {
        vec!["-c".to_owned(), command.to_owned()]
    }
}

#[cfg(test)]
mod tests {
    use super::parse_manifest;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_manifest(body: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("zeit")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ferrum-settings-{stamp}"));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("ferrum.toml");
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn legacy_manifest_keeps_defaults() {
        let path = write_manifest(
            "migrations_dir = \"migrations\"\nschema_command = \"cargo run --quiet -- --print-schema\"\n",
        );
        let config = parse_manifest(&path).expect("legacy");
        assert_eq!(config.default_locale, "de");
        assert!(!config.templates_enabled);
        assert_eq!(config.session_hours, 12);
        assert_eq!(config.primary_color, "#0f172a");
        assert_eq!(config.currency, "EUR");
        assert!(!config.static_enabled);
        assert_eq!(config.storage_backend, "local");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn full_settings_parse_and_unknown_keys_fail() {
        let path = write_manifest(
            r##"
migrations_dir = "migrations"
[project]
name = "myshop"
template = "fullstack"
default_locale = "de"
fallback_locale = "en"
[server]
host = "127.0.0.1"
port = 3000
debug = true
[database]
url = "sqlite://db.sqlite3?mode=rwc"
[admin]
enabled = true
embedded_assets = true
site_title = "Ferrum Admin"
site_header = "Ferrum Verwaltung"
[auth]
enabled = true
session_hours = 12
idle_minutes = 30
[templates]
enabled = true
engine = "tera"
templates_dir = "templates"
[branding]
app_name = "MyShop"
primary_color = "#0f172a"
"##,
        );
        let config = parse_manifest(&path).expect("settings");
        assert_eq!(config.app_name, "MyShop");
        assert!(config.templates_enabled);
        assert!(config.debug);
        let _ = fs::remove_dir_all(path.parent().unwrap());

        let broken = write_manifest(
            "migrations_dir = \"migrations\"\n[branding]\nprimary_color = \"red\"\n",
        );
        assert!(parse_manifest(&broken).is_err());
        let _ = fs::remove_dir_all(broken.parent().unwrap());
        let unknown = write_manifest("migrations_dir = \"migrations\"\nsecret = \"no\"\n");
        assert!(parse_manifest(&unknown).is_err());
        let _ = fs::remove_dir_all(unknown.parent().unwrap());
    }

    #[test]
    fn custom_settings_env_and_production_stay_explicit() {
        let path = write_manifest(
            "migrations_dir = \"migrations\"\n[custom.shop]\ncurrency = \"EUR\"\nitems_per_page = 25\nopen = true\n",
        );
        let config = parse_manifest(&path).expect("custom");
        #[derive(serde::Deserialize)]
        struct ShopSettings {
            currency: String,
            items_per_page: usize,
            open: bool,
        }
        let shop = config.custom_settings::<ShopSettings>("shop").unwrap();
        assert_eq!(shop.currency, "EUR");
        assert_eq!(shop.items_per_page, 25);
        assert!(shop.open);
        assert!(config.custom_settings::<ShopSettings>("missing").is_err());
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("super-secret-value"));
        let secret = write_manifest(
            "migrations_dir = \"migrations\"\n[database]\nurl = \"postgres://ferrum:super-secret-value@localhost/ferrum\"\n",
        );
        let hidden = parse_manifest(&secret).unwrap();
        let debug = format!("{hidden:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("super-secret-value"));
        let denied = write_manifest(
            "migrations_dir = \"migrations\"\n[custom.shop]\npassword = \"super-secret-value\"\n",
        );
        let error = parse_manifest(&denied).unwrap_err();
        assert!(!error.0.contains("super-secret-value"));
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous_env = std::env::var("FERRUM_ENV").ok();
        let previous_debug = std::env::var("FERRUM_DEBUG").ok();
        unsafe {
            std::env::set_var("FERRUM_ENV", "production");
            std::env::set_var("FERRUM_DEBUG", "true");
        }
        let error = config.enforce_runtime().unwrap_err();
        assert!(error.0.contains("Produktion"));
        assert!(!error.0.contains("super-secret-value"));
        unsafe {
            std::env::set_var("FERRUM_DEBUG", "maybe");
        }
        assert!(config.resolved_debug().is_err());
        unsafe {
            std::env::remove_var("FERRUM_DEBUG");
            std::env::remove_var("FERRUM_ENV");
        }
        if let Some(value) = previous_env {
            unsafe { std::env::set_var("FERRUM_ENV", value) };
        }
        if let Some(value) = previous_debug {
            unsafe { std::env::set_var("FERRUM_DEBUG", value) };
        }
        drop(guard);
        let _ = fs::remove_dir_all(path.parent().unwrap());
        let _ = fs::remove_dir_all(secret.parent().unwrap());
        let _ = fs::remove_dir_all(denied.parent().unwrap());
    }
}
