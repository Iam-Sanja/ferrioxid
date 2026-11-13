//! Typisierte Admin-Konfiguration über der bestehenden `AdminModelMeta`.
//!
//! `ModelAdmin` ersetzt die Registry nicht. `check` kopiert die Modellfelder und
//! darf sie nur enger machen: weniger Listen- und Suchfelder, zusätzliche
//! Nur-Lesen- und Hidden-Markierungen, eigene Fieldsets und zusätzliche
//! Aktionsrechte. `AdminRegistry::seal` schaltet die Kopie frei.

use ferrioxid_core::{AdminFieldMeta, AdminFieldset, AdminModelMeta, ColumnKind, ColumnSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub code: &'static str,
    pub detail: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone)]
pub struct ModelAdmin {
    key: &'static str,
    list_display: Option<Vec<&'static str>>,
    search_fields: Option<Vec<&'static str>>,
    filters: Option<Vec<&'static str>>,
    ordering: Option<Option<&'static str>>,
    readonly: Vec<&'static str>,
    hidden: Vec<&'static str>,
    fieldsets: Vec<FieldsetDecl>,
    fieldsets_set: bool,
    actions: Vec<(&'static str, &'static str)>,
    per_page: Option<u16>,
    links: Vec<ManyLink>,
}

#[derive(Debug, Clone, Copy)]
struct FieldsetDecl {
    name: &'static str,
    description: &'static str,
    optional: bool,
    fields: &'static [&'static str],
}

/// Geprüfte Many-to-Many-Angabe. Die HTML-Oberfläche schreibt sie über `replace_links`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManyLink {
    pub name: &'static str,
    pub label: &'static str,
    pub join_table: &'static str,
    pub parent_column: &'static str,
    pub child_column: &'static str,
    pub child_key: &'static str,
}

impl ManyLink {
    pub(crate) fn relation(self) -> ferrioxid_db::ManyToMany {
        ferrioxid_db::ManyToMany {
            join_table: self.join_table,
            parent_column: self.parent_column,
            child_column: self.child_column,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckedModelAdmin {
    key: &'static str,
    fields: Vec<AdminFieldMeta>,
    fieldsets: Option<Vec<AdminFieldset>>,
    ordering: Option<&'static str>,
    ordering_replaced: bool,
    actions: Vec<(&'static str, &'static str)>,
    per_page: Option<u16>,
    links: Vec<ManyLink>,
}

impl ModelAdmin {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            list_display: None,
            search_fields: None,
            filters: None,
            ordering: None,
            readonly: Vec::new(),
            hidden: Vec::new(),
            fieldsets: Vec::new(),
            fieldsets_set: false,
            actions: Vec::new(),
            per_page: None,
            links: Vec::new(),
        }
    }

    pub fn list_display(mut self, fields: &'static [&'static str]) -> Self {
        self.list_display = Some(fields.to_vec());
        self
    }

    pub fn search_fields(mut self, fields: &'static [&'static str]) -> Self {
        self.search_fields = Some(fields.to_vec());
        self
    }

    pub fn filters(mut self, fields: &'static [&'static str]) -> Self {
        self.filters = Some(fields.to_vec());
        self
    }

    /// `None` hebt die Modellsortierung auf. Ein führendes `-` sortiert absteigend.
    pub fn ordering(mut self, ordering: Option<&'static str>) -> Self {
        self.ordering = Some(ordering);
        self
    }

    pub fn readonly(mut self, fields: &'static [&'static str]) -> Self {
        self.readonly = fields.to_vec();
        self
    }

    pub fn hidden(mut self, fields: &'static [&'static str]) -> Self {
        self.hidden = fields.to_vec();
        self
    }

    pub fn fieldset(
        mut self,
        name: &'static str,
        description: &'static str,
        optional: bool,
        fields: &'static [&'static str],
    ) -> Self {
        self.fieldsets_set = true;
        self.fieldsets.push(FieldsetDecl {
            name,
            description,
            optional,
            fields,
        });
        self
    }

    /// Zusätzliches Recht. `code` hat die Form `ressource.aktion` und wird mit `holds` geprüft.
    pub fn action_permission(mut self, action: &'static str, code: &'static str) -> Self {
        self.actions.push((action, code));
        self
    }

    pub fn per_page(mut self, per_page: u16) -> Self {
        self.per_page = Some(per_page);
        self
    }

    /// Bestehende Kinddatensätze über eine Verbindungstabelle. Wirksam nach `seal`.
    ///
    /// `name` darf keine Modellspalte sein. `child_key` muss beim Versiegeln registriert sein.
    pub fn many_to_many(
        mut self,
        name: &'static str,
        label: &'static str,
        join_table: &'static str,
        parent_column: &'static str,
        child_column: &'static str,
        child_key: &'static str,
    ) -> Self {
        self.links.push(ManyLink {
            name,
            label,
            join_table,
            parent_column,
            child_column,
            child_key,
        });
        self
    }

    pub fn check(
        self,
        meta: &AdminModelMeta,
        columns: &[ColumnSpec],
    ) -> Result<CheckedModelAdmin, ConfigError> {
        if self.key != meta.key || !ident(self.key) {
            return Err(error(
                "admin.configure",
                format!("`{}` does not match the registered model key.", self.key),
            ));
        }
        if self
            .per_page
            .is_some_and(|value| !(1..=100).contains(&value))
        {
            return Err(error(
                "admin.configure",
                "per_page must be from 1 to 100.".into(),
            ));
        }
        let mut fields: Vec<AdminFieldMeta> = meta.fields.to_vec();
        apply_hidden(&mut fields, &self.hidden)?;
        apply_readonly(&mut fields, &self.readonly)?;
        if let Some(names) = &self.list_display {
            apply_flag(&mut fields, names, Flag::List)?;
        }
        if let Some(names) = &self.search_fields {
            apply_flag(&mut fields, names, Flag::Search)?;
            for name in names {
                let column = columns.iter().find(|column| column.name == *name);
                if column.is_none_or(|column| column.kind != ColumnKind::Text) {
                    return Err(error(
                        "admin.search",
                        format!("`{name}` is not a text column and cannot be searched."),
                    ));
                }
            }
        }
        if let Some(names) = &self.filters {
            apply_flag(&mut fields, names, Flag::Filter)?;
        }
        let fieldsets = if self.fieldsets_set {
            Some(assign_fieldsets(&mut fields, &self.fieldsets)?)
        } else {
            None
        };
        let (ordering, ordering_replaced) = match self.ordering {
            None => (meta.ordering, false),
            Some(value) => {
                if let Some(raw) = value {
                    let column = raw.strip_prefix('-').unwrap_or(raw);
                    let allowed = fields.iter().any(|field| {
                        field.name == column
                            && !field.hidden
                            && !field.sensitive
                            && (field.sortable || field.name == "id")
                    });
                    if !ident(column) || !allowed {
                        return Err(error(
                            "admin.ordering",
                            format!("`{raw}` is not a visible sortable field."),
                        ));
                    }
                }
                (value, true)
            }
        };
        let mut actions = Vec::new();
        for (action, code) in &self.actions {
            if !matches!(*action, "read" | "create" | "update" | "delete") {
                return Err(error(
                    "admin.permission",
                    format!("`{action}` is not a model action."),
                ));
            }
            if actions.iter().any(|(existing, _)| existing == action) {
                return Err(error(
                    "admin.permission",
                    format!("`{action}` is declared more than once."),
                ));
            }
            if !permission_code(code) {
                return Err(error(
                    "admin.permission",
                    format!("`{code}` must look like resource.action."),
                ));
            }
            actions.push((*action, *code));
        }
        let links = checked_links(&self.links, columns)?;
        Ok(CheckedModelAdmin {
            key: self.key,
            fields,
            fieldsets,
            ordering,
            ordering_replaced,
            actions,
            per_page: self.per_page,
            links,
        })
    }
}

impl CheckedModelAdmin {
    pub fn key(&self) -> &'static str {
        self.key
    }

    pub(crate) fn fields(&self) -> &[AdminFieldMeta] {
        &self.fields
    }

    pub(crate) fn fieldsets(&self) -> Option<&[AdminFieldset]> {
        self.fieldsets.as_deref()
    }

    pub(crate) fn ordering(&self) -> Option<&'static str> {
        self.ordering
    }

    pub(crate) fn ordering_replaced(&self) -> bool {
        self.ordering_replaced
    }

    pub(crate) fn actions(&self) -> &[(&'static str, &'static str)] {
        &self.actions
    }

    pub(crate) fn per_page(&self) -> Option<u16> {
        self.per_page
    }

    pub(crate) fn links(&self) -> &[ManyLink] {
        &self.links
    }
}

fn checked_links(links: &[ManyLink], columns: &[ColumnSpec]) -> Result<Vec<ManyLink>, ConfigError> {
    if links.len() > 8 {
        return Err(error(
            "admin.relation",
            "A model can declare at most 8 many-to-many widgets.".into(),
        ));
    }
    let mut checked = Vec::new();
    for link in links {
        if !ident(link.name)
            || columns.iter().any(|column| column.name == link.name)
            || checked
                .iter()
                .any(|existing: &ManyLink| existing.name == link.name)
        {
            return Err(error(
                "admin.relation",
                format!("`{}` is not a free many-to-many name.", link.name),
            ));
        }
        if !ident(link.join_table)
            || !ident(link.parent_column)
            || !ident(link.child_column)
            || !ident(link.child_key)
            || link.parent_column == link.child_column
        {
            return Err(error(
                "admin.relation",
                format!("`{}` does not name a safe join table.", link.name),
            ));
        }
        if link.label.is_empty()
            || link.label.len() > 80
            || link.label.chars().any(|char| char.is_control())
        {
            return Err(error(
                "admin.relation",
                format!("`{}` needs a short visible label.", link.name),
            ));
        }
        checked.push(*link);
    }
    Ok(checked)
}

#[derive(Clone, Copy)]
enum Flag {
    List,
    Search,
    Filter,
}

fn apply_hidden(fields: &mut [AdminFieldMeta], names: &[&str]) -> Result<(), ConfigError> {
    require_known(fields, names)?;
    for name in names {
        if *name == "id" {
            return Err(error("admin.hidden", "id cannot be hidden.".into()));
        }
        let field = fields.iter_mut().find(|field| field.name == *name).unwrap();
        if field.sensitive {
            continue;
        }
        field.hidden = true;
        field.list_display = false;
        field.list_link = false;
        field.searchable = false;
        field.sortable = false;
        field.filterable = false;
    }
    Ok(())
}

fn apply_readonly(fields: &mut [AdminFieldMeta], names: &[&str]) -> Result<(), ConfigError> {
    require_known(fields, names)?;
    for name in names {
        let field = fields.iter_mut().find(|field| field.name == *name).unwrap();
        field.readonly = true;
    }
    Ok(())
}

fn apply_flag(
    fields: &mut [AdminFieldMeta],
    names: &[&str],
    flag: Flag,
) -> Result<(), ConfigError> {
    require_known(fields, names)?;
    if matches!(flag, Flag::List) && names.is_empty() {
        return Err(error(
            "admin.configure",
            "list_display needs at least one field.".into(),
        ));
    }
    for field in fields.iter_mut() {
        let selected = names.contains(&field.name);
        if selected && (field.hidden || field.sensitive) {
            return Err(error(
                if field.sensitive {
                    "admin.secret"
                } else {
                    "admin.hidden"
                },
                format!("`{}` cannot be shown, searched or filtered.", field.name),
            ));
        }
        match flag {
            Flag::List => {
                field.list_display = selected;
                if !selected {
                    field.list_link = false;
                }
            }
            Flag::Search => field.searchable = selected,
            Flag::Filter => field.filterable = selected,
        }
    }
    Ok(())
}

fn assign_fieldsets(
    fields: &mut [AdminFieldMeta],
    sections: &[FieldsetDecl],
) -> Result<Vec<AdminFieldset>, ConfigError> {
    let mut seen = Vec::new();
    let mut rendered = Vec::new();
    for section in sections {
        if !ident(section.name) || section.description.len() > 200 {
            return Err(error(
                "admin.fieldset",
                format!("`{}` is not a valid fieldset.", section.name),
            ));
        }
        if rendered
            .iter()
            .any(|item: &AdminFieldset| item.name == section.name)
        {
            return Err(error(
                "admin.fieldset",
                format!("`{}` is declared more than once.", section.name),
            ));
        }
        require_known(fields, section.fields)?;
        for name in section.fields {
            if seen.contains(name) {
                return Err(error(
                    "admin.fieldset",
                    format!("`{name}` belongs to more than one fieldset."),
                ));
            }
            seen.push(*name);
            let field = fields.iter_mut().find(|field| field.name == *name).unwrap();
            field.fieldset = section.name;
        }
        rendered.push(AdminFieldset {
            name: section.name,
            description: section.description,
            optional: section.optional,
        });
    }
    for field in fields.iter() {
        if !field.hidden && !field.sensitive && !seen.contains(&field.name) {
            return Err(error(
                "admin.fieldset",
                format!("`{}` is visible and has no fieldset.", field.name),
            ));
        }
    }
    Ok(rendered)
}

fn require_known(fields: &[AdminFieldMeta], names: &[&str]) -> Result<(), ConfigError> {
    let mut seen = Vec::new();
    for name in names {
        if !ident(name) || !fields.iter().any(|field| field.name == *name) {
            return Err(error(
                "admin.configure",
                format!("`{name}` is not a field of this model."),
            ));
        }
        if seen.contains(name) {
            return Err(error(
                "admin.configure",
                format!("`{name}` is listed more than once."),
            ));
        }
        seen.push(*name);
    }
    Ok(())
}

pub(crate) fn relation_targets_registered(
    columns: &[ColumnSpec],
    tables: &[&str],
) -> Result<(), String> {
    for column in columns {
        let Some(table) = column.foreign_table else {
            continue;
        };
        if !ident(table) || !tables.contains(&table) {
            return Err(format!(
                "`{}` references `{table}`, which is not registered in this admin.",
                column.name
            ));
        }
    }
    Ok(())
}

fn permission_code(code: &str) -> bool {
    let Some((resource, action)) = code.split_once('.') else {
        return false;
    };
    !code.contains(' ')
        && code.len() <= 80
        && ident(resource)
        && ident(action)
        && code.split('.').nth(2).is_none()
}

fn ident(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 64
}

fn error(code: &'static str, detail: String) -> ConfigError {
    ConfigError { code, detail }
}

#[cfg(test)]
mod tests {
    use super::ModelAdmin;
    use ferrioxid_core::{
        AdminFieldMeta, AdminModelMeta, ColumnKind, ColumnSpec, ReferentialAction,
    };

    fn field(name: &'static str, sensitive: bool, hidden: bool) -> AdminFieldMeta {
        AdminFieldMeta {
            name,
            label: name,
            list_display: name == "id" || name == "title",
            list_link: name == "title",
            searchable: name == "title",
            sortable: name == "title" || name == "id",
            filterable: name == "title",
            readonly: name == "id",
            hidden,
            sensitive,
            fieldset: "",
        }
    }

    fn meta() -> AdminModelMeta {
        let fields: &'static [AdminFieldMeta] = Box::leak(
            vec![
                field("id", false, false),
                field("title", false, false),
                field("body", false, false),
                field("secret", true, true),
            ]
            .into_boxed_slice(),
        );
        AdminModelMeta {
            key: "notes",
            label: "Notes",
            permission: Some("notes"),
            fields,
            ordering: Some("title"),
            per_page: 25,
            fieldsets: &[],
            inlines: &[],
            date_hierarchy: None,
        }
    }

    fn columns() -> Vec<ColumnSpec> {
        ["id", "title", "body", "secret", "owner_id"]
            .into_iter()
            .map(|name| ColumnSpec {
                name,
                kind: if name == "id" || name == "owner_id" {
                    ColumnKind::Uuid
                } else {
                    ColumnKind::Text
                },
                nullable: name == "owner_id",
                primary_key: name == "id",
                default_bool: None,
                unique: name == "owner_id",
                indexed: false,
                foreign_table: (name == "owner_id").then_some("users"),
                foreign_column: (name == "owner_id").then_some("id"),
                on_delete: (name == "owner_id").then_some(ReferentialAction::Restrict),
                precision: 0,
                scale: 0,
                choices: &[],
            })
            .collect()
    }

    #[test]
    fn configuration_narrows_fields_and_rejects_secrets() {
        let checked = ModelAdmin::new("notes")
            .list_display(&["title"])
            .search_fields(&["title"])
            .filters(&["title"])
            .readonly(&["body"])
            .ordering(Some("-title"))
            .per_page(10)
            .action_permission("update", "notes.publish")
            .check(&meta(), &columns())
            .unwrap();
        let body = checked
            .fields()
            .iter()
            .find(|field| field.name == "body")
            .unwrap();
        assert!(body.readonly);
        assert!(!body.list_display);
        let title = checked
            .fields()
            .iter()
            .find(|field| field.name == "title")
            .unwrap();
        assert!(title.list_display && title.searchable);
        assert_eq!(checked.ordering(), Some("-title"));
        assert_eq!(checked.per_page(), Some(10));
        let secret = ModelAdmin::new("notes")
            .list_display(&["secret"])
            .check(&meta(), &columns());
        assert_eq!(secret.unwrap_err().code, "admin.secret");
        let unknown = ModelAdmin::new("other").check(&meta(), &columns());
        assert_eq!(unknown.unwrap_err().code, "admin.configure");
    }

    #[test]
    fn fieldsets_cover_visible_fields_and_relations_must_be_registered() {
        let checked = ModelAdmin::new("notes")
            .fieldset("main", "Main", false, &["id", "title", "body"])
            .check(&meta(), &columns())
            .unwrap();
        assert_eq!(checked.fieldsets().unwrap().len(), 1);
        let missing = ModelAdmin::new("notes")
            .fieldset("main", "Main", false, &["title"])
            .check(&meta(), &columns());
        assert_eq!(missing.unwrap_err().code, "admin.fieldset");
        let gap = super::relation_targets_registered(&columns(), &["notes"]);
        assert!(gap.unwrap_err().contains("users"));
        super::relation_targets_registered(&columns(), &["notes", "users"]).unwrap();
    }

    #[test]
    fn many_to_many_rejects_column_names_and_unsafe_joins() {
        let checked = ModelAdmin::new("notes")
            .many_to_many("tags", "Tags", "note_tags", "note_id", "tag_id", "tags")
            .check(&meta(), &columns())
            .unwrap();
        assert_eq!(checked.links().len(), 1);
        assert_eq!(checked.links()[0].child_key, "tags");
        let column = ModelAdmin::new("notes")
            .many_to_many("title", "Tags", "note_tags", "note_id", "tag_id", "tags")
            .check(&meta(), &columns());
        assert_eq!(column.unwrap_err().code, "admin.relation");
        let same = ModelAdmin::new("notes")
            .many_to_many("tags", "Tags", "note_tags", "tag_id", "tag_id", "tags")
            .check(&meta(), &columns());
        assert_eq!(same.unwrap_err().code, "admin.relation");
    }
}
