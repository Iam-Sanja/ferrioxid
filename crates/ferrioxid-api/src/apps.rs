//! Statische App-Registrierung. Es wird kein Code aus `ferrioxid.toml` geladen.

use crate::pages::{PageError, Site};
use crate::urls::{UrlError, UrlSet};
use crate::App;
use ferrioxid_admin::{AdminAction, AdminFilter, AdminView, AdminWidget};
use ferrioxid_db::ModelRepository;
use ferrioxid_jobs::JobRegistry;
use ferrioxid_migrations::{
    package_order, same_migration_contract, PackageMigration, ProjectConfig,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tera::Context;

#[derive(Debug)]
pub struct AppError {
    pub code: &'static str,
    pub detail: String,
}

impl AppError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.code, self.detail)
    }
}

impl std::error::Error for AppError {}

impl From<UrlError> for AppError {
    fn from(error: UrlError) -> Self {
        Self {
            code: error.code,
            detail: error.detail,
        }
    }
}

impl From<ferrioxid_admin::ExtensionError> for AppError {
    fn from(error: ferrioxid_admin::ExtensionError) -> Self {
        Self::new("app.admin", error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    name: &'static str,
    label: Option<&'static str>,
    prefix: Option<&'static str>,
    depends_on: Vec<&'static str>,
}

impl AppConfig {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            label: None,
            prefix: None,
            depends_on: Vec::new(),
        }
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn prefix(mut self, prefix: &'static str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn depends_on(mut self, names: &[&'static str]) -> Self {
        self.depends_on = names.to_vec();
        self
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn display_label(&self) -> &'static str {
        self.label.unwrap_or(self.name)
    }
}

pub trait FerrioxidApplication {
    fn config(&self) -> AppConfig;
    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError>;

    /// Registriert Handler dieser App. `install` ruft die Methode nicht auf.
    fn register_jobs(&self, _registry: &mut JobRegistry) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateMode {
    Isolated,
    ProjectOverride,
}

#[derive(Clone, Debug)]
pub struct StaticMount {
    pub prefix: String,
    pub directory: PathBuf,
}

#[derive(Default)]
pub struct TemplateCatalog {
    sites: BTreeMap<String, Site>,
}

impl TemplateCatalog {
    pub fn set_routes(&mut self, routes: crate::urls::RouteMap) {
        for site in self.sites.values_mut() {
            site.set_routes(routes.clone());
        }
    }

    pub fn render(&self, name: &str, context: &Context) -> Result<String, PageError> {
        let app = name.split('/').next().unwrap_or("");
        let site = self.sites.get(app).ok_or_else(|| {
            PageError::new(format!(
                "Die Vorlage `{name}` gehört zu keiner registrierten App."
            ))
        })?;
        site.render(name, context)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.sites
            .contains_key(name.split('/').next().unwrap_or(""))
    }
}

pub struct Registration<'a> {
    app: &'a mut App,
    name: &'static str,
    label: &'static str,
    prefix: Option<&'static str>,
    depends_on: Vec<&'static str>,
}

impl<'a> Registration<'a> {
    pub fn routes(&mut self, set: UrlSet) -> Result<(), AppError> {
        let set = if let Some(prefix) = self.prefix {
            UrlSet::new().nest(prefix, set)?
        } else {
            set
        };
        for (route, _path) in set.names().iter() {
            let Some((namespace, _)) = route.split_once('.') else {
                return Err(AppError::new(
                    "app.route.namespace",
                    "Ein Routenname hat die Form bereich.name.",
                ));
            };
            if namespace != self.name {
                return Err(AppError::new(
                    "app.route.namespace",
                    format!("`{route}` gehört nicht zur App `{}`.", self.name),
                ));
            }
        }
        self.app.accept_urls(set)?;
        Ok(())
    }

    pub fn model<M: ModelRepository>(&mut self) -> Result<(), AppError> {
        let descriptor = M::descriptor();
        if self.app.tables.contains(descriptor.table) {
            return Err(AppError::new(
                "app.model.duplicate",
                format!(
                    "Die Tabelle `{}` ist bereits registriert.",
                    descriptor.table
                ),
            ));
        }
        self.app.register_in::<M>();
        Ok(())
    }

    pub fn admin_model<M>(&mut self) -> Result<(), AppError>
    where
        M: ModelRepository + 'static,
    {
        let registry = self.app.admin.take().unwrap_or_default();
        self.app.admin = Some(registry.with_app(self.name, self.label).add::<M>());
        Ok(())
    }

    pub fn admin_view(&mut self, view: AdminView) -> Result<(), AppError> {
        let registry = self.app.admin.take().unwrap_or_default();
        self.app.admin = Some(registry.with_app(self.name, self.label).view(view)?);
        Ok(())
    }

    pub fn admin_action(&mut self, action: AdminAction) -> Result<(), AppError> {
        let registry = self.app.admin.take().unwrap_or_default();
        self.app.admin = Some(registry.try_action(action)?);
        Ok(())
    }

    pub fn admin_filter(&mut self, filter: AdminFilter) -> Result<(), AppError> {
        let registry = self.app.admin.take().unwrap_or_default();
        self.app.admin = Some(registry.filter(filter)?);
        Ok(())
    }

    pub fn admin_widget(&mut self, widget: AdminWidget) -> Result<(), AppError> {
        let registry = self.app.admin.take().unwrap_or_default();
        self.app.admin = Some(registry.widget(widget)?);
        Ok(())
    }

    pub fn migrations(&mut self, directory: impl AsRef<Path>) -> Result<(), AppError> {
        let directory = directory.as_ref();
        if directory
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(AppError::new(
                "app.migration.path",
                "Der Migrationspfad darf nicht aus dem Verzeichnis ausbrechen.",
            ));
        }
        self.app.migrations.push(PackageMigration {
            name: self.name.to_owned(),
            directory: directory.to_path_buf(),
            depends_on: self
                .depends_on
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        });
        Ok(())
    }

    pub fn templates(
        &mut self,
        directory: impl AsRef<Path>,
        mode: TemplateMode,
    ) -> Result<(), AppError> {
        let files = collect_templates(
            self.name,
            directory.as_ref(),
            mode,
            self.app.settings.as_ref(),
        )?;
        let site = Site::from_named(&files, &self.app.names)
            .map_err(|error| AppError::new("app.template", error.to_string()))?;
        if self
            .app
            .catalog
            .sites
            .insert(self.name.to_owned(), site)
            .is_some()
        {
            return Err(AppError::new(
                "app.template.duplicate",
                format!("Die App `{}` hat schon Vorlagen.", self.name),
            ));
        }
        Ok(())
    }

    pub fn static_files(&mut self, directory: impl AsRef<Path>) -> Result<(), AppError> {
        let mount = static_mount(self.name, directory.as_ref())?;
        if self
            .app
            .statics
            .iter()
            .any(|existing| existing.prefix == mount.prefix)
        {
            return Err(AppError::new(
                "app.static.duplicate",
                format!("Das Präfix `{}` ist schon vergeben.", mount.prefix),
            ));
        }
        self.app.statics.push(mount);
        Ok(())
    }

    pub fn project(&self) -> Result<&ProjectConfig, AppError> {
        self.app.settings.as_ref().ok_or_else(|| {
            AppError::new(
                "app.settings.missing",
                "with_settings wurde vor der App-Registrierung nicht aufgerufen.",
            )
        })
    }
}

impl App {
    pub fn install(mut self, application: impl FerrioxidApplication) -> Result<Self, AppError> {
        if self.sealed {
            return Err(AppError::new(
                "app.sealed",
                "Die Registry ist bereits versiegelt.",
            ));
        }
        let config = application.config();
        validate_app_name(config.name)?;
        if self.installed.iter().any(|name| name == config.name) {
            return Err(AppError::new(
                "app.duplicate",
                format!("Die App `{}` ist schon registriert.", config.name),
            ));
        }
        if let Some(prefix) = config.prefix {
            validate_prefix(prefix)?;
        }
        let mut registration = Registration {
            app: &mut self,
            name: config.name,
            label: config.display_label(),
            prefix: config.prefix,
            depends_on: config.depends_on,
        };
        application.contribute(&mut registration)?;
        self.installed.push(config.name.to_owned());
        Ok(self)
    }

    pub fn seal(mut self) -> Result<Self, AppError> {
        if self.sealed {
            return Ok(self);
        }
        package_order(&self.migrations).map_err(|error| AppError::new("app.migration", error.0))?;
        let declared = self
            .settings
            .as_ref()
            .map(|settings| settings.apps.as_slice())
            .unwrap_or(&[]);
        if !self.migrations.is_empty() || !declared.is_empty() {
            let Some(settings) = self.settings.as_ref() else {
                return Err(AppError::new(
                    "app.migration.undeclared",
                    "App-Migrationen sind nicht an ferrioxid.toml gebunden. Der Start gilt nicht als geprüft.",
                ));
            };
            let declared = settings
                .apps
                .iter()
                .map(|app| PackageMigration {
                    name: app.name.clone(),
                    directory: app.migrations.clone(),
                    depends_on: app.depends_on.clone(),
                })
                .collect::<Vec<_>>();
            same_migration_contract(&self.migrations, &declared, &settings.root)
                .map_err(|error| AppError::new("app.migration.mismatch", error.0))?;
        }
        if let Some(registry) = self.admin.take() {
            self.admin = Some(
                registry
                    .seal()
                    .map_err(|error| AppError::new("app.admin", error.to_string()))?,
            );
        }
        self.sealed = true;
        Ok(self)
    }

    pub fn installed_apps(&self) -> &[String] {
        &self.installed
    }
}

fn validate_app_name(name: &str) -> Result<(), AppError> {
    let mut chars = name.chars();
    let ok = matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && name.len() <= 32;
    if !ok
        || matches!(
            name,
            "admin" | "auth" | "api" | "static" | "healthz" | "uploads"
        )
    {
        return Err(AppError::new(
            "app.name",
            format!("`{name}` ist kein zulässiger App-Name."),
        ));
    }
    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<(), AppError> {
    if !prefix.starts_with('/')
        || prefix.contains("//")
        || prefix.contains("..")
        || prefix.contains('\\')
        || prefix == "/admin"
        || prefix.starts_with("/admin/")
        || prefix == "/auth"
        || prefix.starts_with("/auth/")
    {
        return Err(AppError::new(
            "app.prefix",
            format!("`{prefix}` ist kein zulässiges URL-Präfix."),
        ));
    }
    Ok(())
}

fn static_mount(app: &str, directory: &Path) -> Result<StaticMount, AppError> {
    if directory
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AppError::new(
            "app.static.path",
            "Der Static-Pfad darf nicht aus dem Verzeichnis ausbrechen.",
        ));
    }
    Ok(StaticMount {
        prefix: format!("/static/{app}"),
        directory: directory.to_path_buf(),
    })
}

fn collect_templates(
    app: &str,
    directory: &Path,
    mode: TemplateMode,
    settings: Option<&ProjectConfig>,
) -> Result<Vec<(String, String)>, AppError> {
    let mut files = Vec::new();
    if directory.is_dir() {
        let root = directory
            .canonicalize()
            .map_err(|error| AppError::new("app.template.path", error.to_string()))?;
        walk_templates(app, &root, &root, &mut files)?;
    } else if mode == TemplateMode::Isolated || directory.exists() {
        return Err(AppError::new(
            "app.template.path",
            format!("Template directory for {app} is missing or invalid."),
        ));
    }
    if let Some(settings) = settings {
        let project = settings.templates_directory();
        if project.is_dir() {
            let root = project
                .canonicalize()
                .map_err(|error| AppError::new("app.template.path", error.to_string()))?;
            let shared = root.join(app);
            if shared.exists() {
                let mut overrides = Vec::new();
                walk_templates(app, &root, &shared, &mut overrides)?;
                for (name, body) in overrides {
                    if let Some((_, existing)) = files.iter_mut().find(|(local, _)| local == &name)
                    {
                        if mode == TemplateMode::Isolated {
                            return Err(AppError::new("app.template.conflict",
                                format!("{name} already exists in the project. Overrides must be explicitly enabled.")));
                        }
                        *existing = body;
                    } else if mode == TemplateMode::ProjectOverride {
                        files.push((name, body));
                    }
                }
            }
            let base = root.join("base.html");
            if base.is_file() {
                let canonical = base
                    .canonicalize()
                    .map_err(|error| AppError::new("app.template.path", error.to_string()))?;
                if !canonical.starts_with(&root) {
                    return Err(AppError::new(
                        "app.template.path",
                        "The base template escapes the project templates directory.",
                    ));
                }
                let body = std::fs::read_to_string(base)
                    .map_err(|error| AppError::new("app.template", error.to_string()))?;
                files.push(("base.html".to_owned(), body));
            }
        }
    }
    Ok(files)
}

fn walk_templates(
    app: &str,
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, String)>,
) -> Result<(), AppError> {
    let canonical = directory
        .canonicalize()
        .map_err(|error| AppError::new("app.template.path", error.to_string()))?;
    if !canonical.starts_with(root)
        || std::fs::symlink_metadata(directory)
            .map_err(|error| AppError::new("app.template.path", error.to_string()))?
            .file_type()
            .is_symlink()
    {
        return Err(AppError::new(
            "app.template.path",
            "Template directories must not escape their root or contain links.",
        ));
    }
    for entry in std::fs::read_dir(directory)
        .map_err(|error| AppError::new("app.template.path", error.to_string()))?
    {
        let entry = entry.map_err(|error| AppError::new("app.template.path", error.to_string()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == ".." || name.contains('\\') {
            return Err(AppError::new(
                "app.template.path",
                "Versteckte oder ausbrechende Vorlagennamen sind nicht erlaubt.",
            ));
        }
        if path.is_dir() {
            walk_templates(app, root, &path, files)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("html") {
            continue;
        }
        let canonical = path.canonicalize().map_err(|error| {
            AppError::new(
                "app.template.path",
                format!("Vorlage ist nicht lesbar: {error}"),
            )
        })?;
        if !canonical.starts_with(root) {
            return Err(AppError::new(
                "app.template.path",
                "Eine Vorlage liegt außerhalb des App-Verzeichnisses.",
            ));
        }
        let relative = canonical
            .strip_prefix(root)
            .map_err(|_| AppError::new("app.template.path", "Der relative Vorlagenname fehlt."))?;
        let template_name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        if template_name.split('/').any(|part| part == "admin")
            || template_name == "base.html"
            || template_name == "login.html"
        {
            return Err(AppError::new(
                "app.template.reserved",
                format!("`{template_name}` darf keine Admin- oder Basisvorlage ersetzen."),
            ));
        }
        if !template_name.starts_with(&format!("{app}/")) {
            return Err(AppError::new(
                "app.template.namespace",
                format!("`{template_name}` muss unter `{app}/` liegen."),
            ));
        }
        let body = std::fs::read_to_string(&canonical)
            .map_err(|error| AppError::new("app.template", error.to_string()))?;
        if files.iter().any(|(existing, _)| existing == &template_name) {
            return Err(AppError::new(
                "app.template.duplicate",
                format!("`{template_name}` ist doppelt vorhanden."),
            ));
        }
        files.push((template_name, body));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_app_name, AppConfig, FerrioxidApplication, Registration, TemplateMode};
    use crate::urls::UrlSet;
    use crate::App;
    use axum::routing::get;
    use ferrioxid_db::connect;
    use tower::ServiceExt;

    struct Shop;
    struct Blog;

    impl FerrioxidApplication for Shop {
        fn config(&self) -> AppConfig {
            AppConfig::new("shop").label("Shop").prefix("/shop")
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.routes(UrlSet::new().route("shop.list", "/items", get(|| async { "ok" }))?)
        }
    }

    impl FerrioxidApplication for Blog {
        fn config(&self) -> AppConfig {
            AppConfig::new("blog").prefix("/blog")
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.routes(UrlSet::new().route("blog.list", "/posts", get(|| async { "ok" }))?)
        }
    }

    #[tokio::test]
    async fn install_rejects_duplicates_reserved_names_and_route_collisions() {
        let db = connect("sqlite::memory:").await.unwrap();
        let app = App::new(db).install(Shop).unwrap().install(Blog).unwrap();
        assert_eq!(app.installed_apps(), ["shop".to_owned(), "blog".to_owned()]);
        assert!(validate_app_name("admin").is_err());
        let db = connect("sqlite::memory:").await.unwrap();
        let duplicate = App::new(db).install(Shop).unwrap().install(Shop);
        assert!(matches!(duplicate, Err(error) if error.code == "app.duplicate"));
        let db = connect("sqlite::memory:").await.unwrap();
        let collision = App::new(db).install(Shop).unwrap().urls(
            UrlSet::new()
                .route("site.same", "/shop/items", get(|| async { "x" }))
                .unwrap(),
        );
        assert!(collision.is_err());
    }

    #[tokio::test]
    async fn templates_reject_escape_and_admin_names() {
        let root = std::env::temp_dir().join(format!(
            "ferrioxid-templates-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let directory = root.join("templates");
        std::fs::create_dir_all(directory.join("shop")).unwrap();
        std::fs::write(directory.join("shop/list.html"), "<p>ok</p>").unwrap();
        std::fs::create_dir_all(directory.join("admin")).unwrap();
        std::fs::write(directory.join("admin/login.html"), "<p>no</p>").unwrap();
        let db = connect("sqlite::memory:").await.unwrap();
        let installed = App::new(db).install(TemplateApp {
            directory: directory.clone(),
        });
        assert!(installed.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    struct TemplateApp {
        directory: std::path::PathBuf,
    }

    impl FerrioxidApplication for TemplateApp {
        fn config(&self) -> AppConfig {
            AppConfig::new("shop")
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.templates(&self.directory, TemplateMode::Isolated)
        }
    }

    #[test]
    fn the_settings_namespace_is_the_app_name() {
        assert_eq!(AppConfig::new("shop").name(), "shop");
        assert_eq!(AppConfig::new("shop").display_label(), "shop");
    }

    struct Alpha;
    struct Beta;

    impl FerrioxidApplication for Alpha {
        fn config(&self) -> AppConfig {
            AppConfig::new("alpha").depends_on(&["beta"])
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.migrations("alpha/migrations")
        }
    }

    impl FerrioxidApplication for Beta {
        fn config(&self) -> AppConfig {
            AppConfig::new("beta").depends_on(&["alpha"])
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.migrations("beta/migrations")
        }
    }

    #[tokio::test]
    async fn seal_rejects_a_cycle_and_a_later_install() {
        let db = connect("sqlite::memory:").await.unwrap();
        let cycle = App::new(db)
            .install(Alpha)
            .unwrap()
            .install(Beta)
            .unwrap()
            .seal();
        assert!(matches!(cycle, Err(error) if error.code == "app.migration"));
        let db = connect("sqlite::memory:").await.unwrap();
        let sealed = App::new(db).install(Shop).unwrap().seal().unwrap();
        let again = sealed.install(Blog);
        assert!(matches!(again, Err(error) if error.code == "app.sealed"));
    }

    struct FilesApp {
        templates: std::path::PathBuf,
        assets: std::path::PathBuf,
    }

    impl FerrioxidApplication for FilesApp {
        fn config(&self) -> AppConfig {
            AppConfig::new("shop").prefix("/shop")
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.templates(&self.templates, TemplateMode::Isolated)?;
            registry.static_files(&self.assets)?;
            registry.routes(UrlSet::new().route("shop.list", "/", get(|| async { "ok" }))?)?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn static_files_use_the_app_prefix_and_reject_escape() {
        let root = std::env::temp_dir().join(format!(
            "ferrioxid-static-app-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let templates = root.join("templates");
        let assets = root.join("static/shop");
        std::fs::create_dir_all(templates.join("shop")).unwrap();
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(templates.join("shop/list.html"), "<p>liste</p>").unwrap();
        std::fs::write(assets.join("shop.css"), b"body{}").unwrap();
        let db = connect("sqlite::memory:").await.unwrap();
        let app = App::new(db)
            .install(FilesApp {
                templates: templates.clone(),
                assets: assets.clone(),
            })
            .unwrap();
        let router = app.into_router();
        let response = router
            .oneshot(
                axum::http::Request::get("/static/shop/shop.css")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let db = connect("sqlite::memory:").await.unwrap();
        let escaped = App::new(db).install(FilesApp {
            templates,
            assets: std::path::PathBuf::from("static/../secret"),
        });
        assert!(matches!(escaped, Err(error) if error.code == "app.static.path"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn isolated_templates_refuse_a_project_file_with_the_same_name() {
        let root = std::env::temp_dir().join(format!(
            "ferrioxid-template-override-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("templates/shop")).unwrap();
        std::fs::create_dir_all(root.join("app/shop")).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n",
        )
        .unwrap();
        std::fs::write(root.join("templates/shop/list.html"), "<p>projekt</p>").unwrap();
        std::fs::write(root.join("app/shop/list.html"), "<p>app</p>").unwrap();
        let settings = ferrioxid_migrations::discover_project(&root).unwrap();
        let db = connect("sqlite::memory:").await.unwrap();
        let installed = App::new(db).with_settings(settings).install(TemplateApp {
            directory: root.join("app"),
        });
        assert!(matches!(installed, Err(error) if error.code == "app.template.conflict"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_templates_override_local_and_support_shared_only_files() {
        let root = std::env::temp_dir().join(format!("ferrioxid-overlay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("templates/shop")).unwrap();
        std::fs::create_dir_all(root.join("local/shop")).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("templates/base.html"),
            "<main>{% block content %}{% endblock %}</main>",
        )
        .unwrap();
        std::fs::write(
            root.join("templates/shop/index.html"),
            "{% extends \"base.html\" %}{% block content %}shared {{ value }}{% endblock %}",
        )
        .unwrap();
        std::fs::write(root.join("templates/shop/extra.html"), "extra").unwrap();
        std::fs::write(root.join("local/shop/index.html"), "local").unwrap();
        std::fs::write(root.join("local/shop/fallback.html"), "fallback").unwrap();
        let settings = ferrioxid_migrations::discover_project(&root).unwrap();
        for directory in [root.join("local"), root.join("missing")] {
            let files = super::collect_templates(
                "shop",
                &directory,
                TemplateMode::ProjectOverride,
                Some(&settings),
            )
            .unwrap();
            let site =
                crate::pages::Site::from_named(&files, &crate::urls::RouteMap::default()).unwrap();
            let mut context = tera::Context::new();
            context.insert("value", "<script>");
            let html = site.render("shop/index.html", &context).unwrap();
            assert!(html.contains("shared"));
            assert!(html.contains("&lt;script&gt;"));
            assert!(!html.contains("<script>"));
            assert_eq!(site.render("shop/extra.html", &context).unwrap(), "extra");
        }
        let files = super::collect_templates(
            "shop",
            &root.join("local"),
            TemplateMode::ProjectOverride,
            Some(&settings),
        )
        .unwrap();
        assert!(files
            .iter()
            .any(|(name, body)| name == "shop/fallback.html" && body == "fallback"));
        std::fs::create_dir_all(root.join("templates/shop/admin")).unwrap();
        std::fs::write(root.join("templates/shop/admin/login.html"), "reserved").unwrap();
        assert!(super::collect_templates(
            "shop",
            &root.join("local"),
            TemplateMode::ProjectOverride,
            Some(&settings)
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn shared_templates_reject_links_and_directory_cycles() {
        let root =
            std::env::temp_dir().join(format!("ferrioxid-template-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("templates/shop")).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(root.join("templates"), root.join("templates/shop/loop"))
            .unwrap();
        let settings = ferrioxid_migrations::discover_project(&root).unwrap();
        assert!(super::collect_templates(
            "shop",
            &root.join("missing"),
            TemplateMode::ProjectOverride,
            Some(&settings)
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    struct Migrated;

    impl FerrioxidApplication for Migrated {
        fn config(&self) -> AppConfig {
            AppConfig::new("shop")
        }

        fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), super::AppError> {
            registry.migrations("src/apps/shop/migrations")
        }
    }

    #[tokio::test]
    async fn start_seals_installed_apps_and_rejects_a_migration_mismatch() {
        let db = connect("sqlite::memory:").await.unwrap();
        assert!(App::new(db)
            .install(Shop)
            .unwrap()
            .try_into_router()
            .is_ok());
        let db = connect("sqlite::memory:").await.unwrap();
        assert!(App::new(db).try_into_router().is_ok());
        let db = connect("sqlite::memory:").await.unwrap();
        let invalid = App::new(db)
            .install(Alpha)
            .unwrap()
            .install(Beta)
            .unwrap()
            .try_into_router();
        assert!(matches!(invalid, Err(error) if error.code == "app.migration"));
        let db = connect("sqlite::memory:").await.unwrap();
        let undeclared = App::new(db).install(Migrated).unwrap().try_into_router();
        assert!(matches!(undeclared, Err(error) if error.code == "app.migration.undeclared"));
        let root = std::env::temp_dir().join(format!(
            "ferrioxid-contract-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n[app.blog]\nmigrations = \"src/apps/blog/migrations\"\n",
        )
        .unwrap();
        let settings = ferrioxid_migrations::discover_project(&root).unwrap();
        let db = connect("sqlite::memory:").await.unwrap();
        let mismatch = App::new(db)
            .with_settings(settings)
            .install(Migrated)
            .unwrap()
            .try_into_router();
        assert!(matches!(mismatch, Err(error) if error.code == "app.migration.mismatch"));
        let _ = std::fs::remove_dir_all(root);
    }
}
