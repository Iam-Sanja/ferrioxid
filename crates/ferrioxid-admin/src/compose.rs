//! Eltern und Kinder einer Has-Many-Beziehung in einer Transaktion.

use crate::audit::{self, Event};
use crate::drafts;
use crate::registry::ManagedModel;
use ferrioxid_auth::{AuthService, AuthUser};
use ferrioxid_core::{temporal, AdminInline, ColumnKind, ColumnSpec, FerrioxidError};
use ferrioxid_db::{query_admin_page, AdminPageRequest, DeleteOutcome};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait, Value as DbValue};
use serde_json::{Map, Number, Value};
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

pub enum SaveResult {
    Saved(Uuid),
    Invalid(Map<String, Value>),
    Conflict(Map<String, Value>),
    Denied,
    Missing,
}

pub async fn groups(
    db: &DatabaseConnection,
    auth: &AuthService,
    user: &AuthUser,
    parent: &dyn ManagedModel,
    registry_child: impl Fn(&str) -> Option<std::sync::Arc<dyn ManagedModel>>,
    id: Option<Uuid>,
    values: &Map<String, Value>,
) -> (Vec<Value>, String) {
    let mut rendered = Vec::new();
    let mut basis_parent = Map::new();
    if let Some(id) = id {
        if let Ok(Some(record)) = parent.read(db, id, &parent.write_scope(user)).await {
            if let Some(object) = record.as_object() {
                basis_parent = snapshot_fields(parent.meta().fields, object);
            }
        }
    }
    let mut basis_children = Vec::new();
    for spec in parent.meta().inlines.iter().filter(|spec| spec.form) {
        let Some(child) = registry_child(spec.child_table) else {
            continue;
        };
        if !linked(parent, spec, child.as_ref()) {
            continue;
        }
        if auth
            .authorize(user, child.meta().permission, "read")
            .await
            .is_err()
        {
            continue;
        }
        let stored = match id {
            Some(parent_id) => load_children(db, child.as_ref(), spec, parent_id, user).await,
            None => Vec::new(),
        };
        if stored.len() > usize::from(spec.max_count) {
            rendered.push(serde_json::json!({
                "label": label(spec, child.meta().label),
                "error": "validation.inline_max",
                "rows": [],
                "add_name": Value::Null,
                "extra_name": format!("extra_{}", spec.name),
                "extra": 0
            }));
            continue;
        }
        for row in &stored {
            if let (Some(id), Some(object)) =
                (row.get("id").and_then(Value::as_str), row.as_object())
            {
                basis_children.push((id.to_owned(), snapshot_fields(child.meta().fields, object)));
            }
        }
        let extra = extra_count(spec, values, stored.len());
        let mut rows = Vec::new();
        for (index, row) in stored.iter().enumerate() {
            rows.push(render_row(spec, child.as_ref(), index, Some(row), values));
        }
        for offset in 0..extra {
            rows.push(render_row(
                spec,
                child.as_ref(),
                stored.len() + offset,
                None,
                values,
            ));
        }
        rendered.push(serde_json::json!({
            "label": label(spec, child.meta().label),
            "error": values.get(&format!("__inline_{}", spec.name)).and_then(Value::as_str).unwrap_or(""),
            "rows": rows,
            "add_name": spec.can_add.then_some("rows"),
            "extra_name": format!("extra_{}", spec.name),
            "extra": extra
        }));
    }
    (rendered, fingerprint(&basis_parent, &basis_children))
}

#[allow(clippy::too_many_arguments)]
pub async fn save(
    db: &DatabaseConnection,
    auth: &AuthService,
    user: &AuthUser,
    parent: &dyn ManagedModel,
    child_of: impl Fn(&str) -> Option<std::sync::Arc<dyn ManagedModel>>,
    id: Option<Uuid>,
    parent_body: Map<String, Value>,
    form: &HashMap<String, String>,
) -> SaveResult {
    let parsed = match parse(parent, &child_of, form) {
        Ok(parsed) => parsed,
        Err(code) => return SaveResult::Invalid(redisplay(&parent_body, form, &code)),
    };
    let tx = match db.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            return SaveResult::Invalid(redisplay(&parent_body, form, &error.to_string()))
        }
    };
    let operation = Uuid::new_v4();
    let result = apply(
        auth,
        user,
        parent,
        &child_of,
        &tx,
        id,
        parent_body.clone(),
        parsed,
        form,
        operation,
    )
    .await;
    match ferrioxid_db::conclude(tx, result, FerrioxidError::database).await {
        Ok(saved) => SaveResult::Saved(saved),
        Err(FerrioxidError::NotFound) => SaveResult::Missing,
        Err(FerrioxidError::Forbidden | FerrioxidError::Denied(_)) => SaveResult::Denied,
        Err(FerrioxidError::Validation(code)) if code == "conflict.stale" => {
            SaveResult::Conflict(redisplay(&parent_body, form, &code))
        }
        Err(FerrioxidError::Validation(code)) => {
            SaveResult::Invalid(redisplay(&parent_body, form, &code))
        }
        Err(_) => SaveResult::Invalid(redisplay(&parent_body, form, "validation.inline_failed")),
    }
}

pub async fn store_extra(
    db: &DatabaseConnection,
    user: &AuthUser,
    parent: &dyn ManagedModel,
    id: Option<Uuid>,
    form: &HashMap<String, String>,
) -> Result<Uuid, FerrioxidError> {
    let mut values = draft_map(parent, form);
    for spec in parent
        .meta()
        .inlines
        .iter()
        .filter(|spec| spec.form && spec.can_add)
    {
        let key = format!("extra_{}", spec.name);
        let current = form
            .get(&key)
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(spec.extra);
        let next = current.saturating_add(1).min(spec.max_count);
        values.insert(key, Value::from(next));
    }
    drafts::insert(
        db,
        user.session_id,
        user.id,
        parent.meta().key,
        id.map(|value| value.to_string()).as_deref(),
        "form",
        &values,
    )
    .await
}

type Prepared = Vec<(AdminInline, std::sync::Arc<dyn ManagedModel>, Vec<Row>)>;

struct Row {
    id: Option<Uuid>,
    remove: bool,
    values: Map<String, Value>,
}

#[allow(clippy::too_many_arguments)]
async fn apply(
    auth: &AuthService,
    user: &AuthUser,
    parent: &dyn ManagedModel,
    _child_of: &impl Fn(&str) -> Option<std::sync::Arc<dyn ManagedModel>>,
    tx: &DatabaseTransaction,
    id: Option<Uuid>,
    parent_body: Map<String, Value>,
    rows: Prepared,
    form: &HashMap<String, String>,
    operation: Uuid,
) -> Result<Uuid, FerrioxidError> {
    let action = if id.is_some() { "update" } else { "create" };
    auth.authorize_on(tx, user, parent.meta().permission, action)
        .await
        .map_err(|_| FerrioxidError::Forbidden)?;
    let parent_scope = parent.write_scope(user);
    if let Some(id) = id {
        let current = parent
            .read_in(tx, id, &parent_scope)
            .await?
            .ok_or(FerrioxidError::NotFound)?;
        let mut children = Vec::new();
        for (spec, child, _) in &rows {
            for row in load_children_tx(tx, child.as_ref(), spec, id, user).await? {
                if let (Some(child_id), Some(object)) =
                    (row.get("id").and_then(Value::as_str), row.as_object())
                {
                    children.push((
                        child_id.to_owned(),
                        snapshot_fields(child.meta().fields, object),
                    ));
                }
            }
        }
        let current_fields = current
            .as_object()
            .map(|object| snapshot_fields(parent.meta().fields, object))
            .unwrap_or_default();
        let expected = form.get("basis").map(String::as_str).unwrap_or("");
        if fingerprint(&current_fields, &children) != expected {
            return Err(FerrioxidError::Validation("conflict.stale".into()));
        }
    }
    let creating = id.is_none();
    let saved = if let Some(id) = id {
        parent
            .update_in(tx, id, Value::Object(parent_body), &parent_scope, user)
            .await?;
        id
    } else {
        let created = parent
            .create_in(tx, Value::Object(parent_body), user)
            .await?;
        created
            .get("id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(FerrioxidError::NotFound)?
    };
    for (spec, child, rows) in rows {
        auth.authorize_on(tx, user, child.meta().permission, "read")
            .await
            .map_err(|_| FerrioxidError::Forbidden)?;
        let scope = child.write_scope(user);
        for row in rows {
            if row.remove {
                let child_id = row
                    .id
                    .ok_or_else(|| FerrioxidError::Validation("validation.inline_field".into()))?;
                if !spec.can_delete {
                    return Err(FerrioxidError::Validation("validation.inline_field".into()));
                }
                auth.authorize_on(tx, user, child.meta().permission, "delete")
                    .await
                    .map_err(|_| FerrioxidError::Forbidden)?;
                ensure_owned(tx, child.as_ref(), &spec, saved, child_id, user).await?;
                if child.delete_in(tx, child_id, &scope, user).await? != DeleteOutcome::Deleted {
                    return Err(FerrioxidError::NotFound);
                }
                write_child_audit(
                    tx,
                    user,
                    child.meta().key,
                    child_id,
                    "delete",
                    "removed",
                    operation,
                )
                .await?;
                continue;
            }
            let mut body = row.values;
            body.insert(spec.column.to_owned(), Value::String(saved.to_string()));
            if let Some(child_id) = row.id {
                if !spec.can_change {
                    return Err(FerrioxidError::Validation("validation.inline_field".into()));
                }
                auth.authorize_on(tx, user, child.meta().permission, "update")
                    .await
                    .map_err(|_| FerrioxidError::Forbidden)?;
                let stored = ensure_owned(tx, child.as_ref(), &spec, saved, child_id, user).await?;
                reject_readonly(&stored, &body, &spec)?;
                child
                    .update_in(tx, child_id, Value::Object(body.clone()), &scope, user)
                    .await?;
                write_child_audit(
                    tx,
                    user,
                    child.meta().key,
                    child_id,
                    "update",
                    &summary_of(&stored, &body),
                    operation,
                )
                .await?;
            } else {
                if !spec.can_add {
                    return Err(FerrioxidError::Validation("validation.inline_field".into()));
                }
                auth.authorize_on(tx, user, child.meta().permission, "create")
                    .await
                    .map_err(|_| FerrioxidError::Forbidden)?;
                let created = child
                    .create_in(tx, Value::Object(body.clone()), user)
                    .await?;
                let child_id = created
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .ok_or(FerrioxidError::NotFound)?;
                write_child_audit(
                    tx,
                    user,
                    child.meta().key,
                    child_id,
                    "create",
                    &summary_of(&Map::new(), &body),
                    operation,
                )
                .await?;
            }
        }
    }
    audit_parent(
        parent,
        tx,
        user,
        &saved,
        operation,
        if creating { "create" } else { "update" },
    )
    .await?;
    Ok(saved)
}

fn parse(
    parent: &dyn ManagedModel,
    child_of: &impl Fn(&str) -> Option<std::sync::Arc<dyn ManagedModel>>,
    form: &HashMap<String, String>,
) -> Result<Prepared, String> {
    let mut parsed = Vec::new();
    for spec in parent
        .meta()
        .inlines
        .iter()
        .copied()
        .filter(|spec| spec.form)
    {
        let child = child_of(spec.child_table).ok_or("validation.inline_field")?;
        if !linked(parent, &spec, child.as_ref()) {
            return Err("validation.inline_field".into());
        }
        reject_unknown_keys(form, &spec, child.as_ref())?;
        let mut seen = HashSet::new();
        let mut rows = Vec::new();
        for index in 0..usize::from(spec.max_count) {
            let prefix = format!("inline_{}_{index}_", spec.name);
            let id = form
                .get(&format!("{prefix}id"))
                .filter(|value| !value.is_empty())
                .map(|value| {
                    Uuid::parse_str(value).map_err(|_| "validation.inline_field".to_owned())
                })
                .transpose()?;
            if let Some(id) = id {
                if !seen.insert(id) {
                    return Err("validation.inline_duplicate".into());
                }
            }
            let remove = form
                .get(&format!("{prefix}remove"))
                .is_some_and(|value| value == "1" || value == "on" || value == "true");
            let fields = editable_fields(&spec, child.as_ref());
            let any = id.is_some()
                || remove
                || fields.iter().any(|field| {
                    form.get(&format!("{prefix}{}", field.name))
                        .is_some_and(|value| !value.is_empty())
                });
            if !any || (remove && id.is_none()) {
                continue;
            }
            let mut values = Map::new();
            for field in fields {
                let key = format!("{prefix}{}", field.name);
                let posted = form.get(&key).map(String::as_str).unwrap_or("");
                let column = child
                    .columns()
                    .iter()
                    .find(|column| column.name == field.name)
                    .ok_or_else(|| "validation.inline_field".to_owned())?;
                if posted.is_empty() {
                    if !column.nullable && column.default_bool.is_none() && !remove {
                        return Err(format!("{key} validation.required"));
                    }
                    if column.nullable {
                        values.insert(field.name.to_owned(), Value::Null);
                    }
                    continue;
                }
                values.insert(
                    field.name.to_owned(),
                    typed(column, posted).map_err(|_| format!("{key} validation.inline_field"))?,
                );
            }
            rows.push(Row { id, remove, values });
        }
        let active = rows.iter().filter(|row| !row.remove).count();
        if active > usize::from(spec.max_count) || active < usize::from(spec.min_count) {
            return Err("validation.inline_max".into());
        }
        parsed.push((spec, child, rows));
    }
    Ok(parsed)
}

async fn ensure_owned(
    tx: &DatabaseTransaction,
    child: &dyn ManagedModel,
    spec: &AdminInline,
    parent_id: Uuid,
    child_id: Uuid,
    user: &AuthUser,
) -> Result<Map<String, Value>, FerrioxidError> {
    let Some(record) = child
        .read_in(tx, child_id, &child.write_scope(user))
        .await?
    else {
        return Err(FerrioxidError::Validation(
            "validation.inline_foreign".into(),
        ));
    };
    let owned =
        record.get(spec.column).and_then(Value::as_str) == Some(parent_id.to_string().as_str());
    if !owned {
        return Err(FerrioxidError::Validation(
            "validation.inline_foreign".into(),
        ));
    }
    Ok(record.as_object().cloned().unwrap_or_default())
}

fn reject_readonly(
    stored: &Map<String, Value>,
    posted: &Map<String, Value>,
    spec: &AdminInline,
) -> Result<(), FerrioxidError> {
    for name in spec.readonly_fields {
        let left = stored.get(*name).map(canonical).unwrap_or_default();
        let right = posted.get(*name).map(canonical).unwrap_or_default();
        if !right.is_empty() && left != right {
            return Err(FerrioxidError::Validation("validation.readonly".into()));
        }
    }
    Ok(())
}

fn reject_unknown_keys(
    form: &HashMap<String, String>,
    spec: &AdminInline,
    child: &dyn ManagedModel,
) -> Result<(), String> {
    let prefix = format!("inline_{}_", spec.name);
    let names: HashSet<&str> = editable_fields(spec, child)
        .into_iter()
        .map(|field| field.name)
        .collect();
    for key in form.keys() {
        let Some(rest) = key.strip_prefix(&prefix) else {
            continue;
        };
        let Some((index, field)) = rest.split_once('_') else {
            return Err("validation.inline_field".into());
        };
        let Ok(position) = index.parse::<usize>() else {
            return Err("validation.inline_field".into());
        };
        if position >= usize::from(spec.max_count) {
            return Err("validation.inline_max".into());
        }
        if field == "id" || field == "remove" || names.contains(field) {
            continue;
        }
        return Err("validation.inline_field".into());
    }
    Ok(())
}

fn editable_fields<'a>(
    spec: &'a AdminInline,
    child: &'a dyn ManagedModel,
) -> Vec<&'a ferrioxid_core::AdminFieldMeta> {
    child
        .meta()
        .fields
        .iter()
        .filter(|field| {
            field.name != "id"
                && field.name != spec.column
                && !field.hidden
                && !field.sensitive
                && !crate::registry::sensitive(field.name)
                && (spec.fields.is_empty() || spec.fields.contains(&field.name))
                && !spec.readonly_fields.contains(&field.name)
                && !field.readonly
        })
        .collect()
}

fn typed(column: &ColumnSpec, text: &str) -> Result<Value, &'static str> {
    match column.kind {
        ColumnKind::BigInt => text
            .parse::<i64>()
            .map(|value| Value::Number(Number::from(value)))
            .map_err(|_| "validation.inline_field"),
        ColumnKind::Boolean => Ok(Value::Bool(matches!(text, "true" | "1" | "on"))),
        ColumnKind::Date => {
            temporal::parse_date(text).map(|value| Value::String(temporal::format_date(value)))
        }
        ColumnKind::Time => {
            temporal::parse_time(text).map(|value| Value::String(temporal::format_time(value)))
        }
        ColumnKind::DateTime => temporal::parse_datetime_local(text)
            .or_else(|_| temporal::parse_datetime_utc(text))
            .map(|value| Value::String(temporal::format_datetime(value))),
        ColumnKind::Uuid | ColumnKind::Text => Ok(Value::String(text.to_owned())),
        ColumnKind::Decimal => {
            ferrioxid_core::format_decimal_text(text, column.precision, column.scale)
                .map(Value::String)
                .map_err(|_| "validation.inline_field")
        }
        ColumnKind::Choice => {
            if ferrioxid_core::choice_is_known(column.choices, text) {
                Ok(Value::String(text.to_owned()))
            } else {
                Err("validation.inline_field")
            }
        }
    }
}

async fn load_children(
    db: &DatabaseConnection,
    child: &dyn ManagedModel,
    spec: &AdminInline,
    parent_id: Uuid,
    user: &AuthUser,
) -> Vec<Value> {
    load_children_tx(db, child, spec, parent_id, user)
        .await
        .unwrap_or_default()
}

async fn load_children_tx(
    db: &impl sea_orm::ConnectionTrait,
    child: &dyn ManagedModel,
    spec: &AdminInline,
    parent_id: Uuid,
    user: &AuthUser,
) -> Result<Vec<Value>, FerrioxidError> {
    let sort = spec
        .ordering
        .filter(|name| child.columns().iter().any(|column| column.name == *name))
        .unwrap_or("id");
    let page = query_admin_page(
        db,
        child.descriptor().table,
        child.columns(),
        AdminPageRequest {
            search_columns: &[],
            search: None,
            sort_column: sort,
            descending: false,
            equal_filters: &[(spec.column, DbValue::from(parent_id.to_string()))],
            bounds: &[],
            offset: 0,
            limit: u64::from(spec.max_count.clamp(1, 50)),
            scope: &child.read_scope(user),
        },
    )
    .await?;
    if page.total > i64::from(spec.max_count) {
        return Err(FerrioxidError::Validation("validation.inline_max".into()));
    }
    Ok(page.rows)
}

fn render_row(
    spec: &AdminInline,
    child: &dyn ManagedModel,
    index: usize,
    stored: Option<&Value>,
    posted: &Map<String, Value>,
) -> Value {
    let prefix = format!("inline_{}_{index}_", spec.name);
    let id = stored
        .and_then(|row| row.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let mut fields = Vec::new();
    for field in child.meta().fields {
        if field.name == "id" || field.name == spec.column || field.hidden || field.sensitive {
            continue;
        }
        if !spec.fields.is_empty() && !spec.fields.contains(&field.name) {
            continue;
        }
        let name = format!("{prefix}{}", field.name);
        let current = posted
            .get(&name)
            .cloned()
            .or_else(|| stored.and_then(|row| row.get(field.name).cloned()))
            .unwrap_or(Value::Null);
        let readonly = field.readonly || spec.readonly_fields.contains(&field.name);
        fields.push(serde_json::json!({
            "name": name,
            "label": field.label,
            "fieldset": "",
            "kind": if readonly { "readonly" } else if matches_bool(child, field.name) { "boolean" } else { "text" },
            "input": input_type(child, field.name),
            "required": false,
            "invalid": false,
            "error": "",
            "hint": "",
            "pick": "",
            "value": match &current { Value::String(text) => text.clone(), Value::Number(number) => number.to_string(), Value::Bool(flag) => flag.to_string(), _ => String::new() },
            "raw": match &current { Value::String(text) => text.clone(), Value::Number(number) => number.to_string(), _ => String::new() },
            "locked": false,
            "checked": checked_value(&current)
        }));
    }
    serde_json::json!({
        "heading": format!("{}", index + 1),
        "id": id,
        "id_name": format!("{prefix}id"),
        "remove_name": spec.can_delete.then(|| format!("{prefix}remove")),
        "remove": posted.get(&format!("{prefix}remove")).and_then(Value::as_str) == Some("1"),
        "fields": fields
    })
}

fn checked_value(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::String(text) => text == "true" || text == "1",
        _ => false,
    }
}

fn matches_bool(child: &dyn ManagedModel, name: &str) -> bool {
    child
        .columns()
        .iter()
        .any(|column| column.name == name && column.kind == ColumnKind::Boolean)
}

fn input_type(child: &dyn ManagedModel, name: &str) -> &'static str {
    match child
        .columns()
        .iter()
        .find(|column| column.name == name)
        .map(|column| column.kind)
    {
        Some(ColumnKind::BigInt) => "number",
        Some(ColumnKind::Date) => "date",
        Some(ColumnKind::Time) => "time",
        Some(ColumnKind::DateTime) => "datetime-local",
        _ => "text",
    }
}

fn extra_count(spec: &AdminInline, values: &Map<String, Value>, stored: usize) -> usize {
    let requested = values
        .get(&format!("extra_{}", spec.name))
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64().map(|value| value as u16),
            Value::String(text) => text.parse().ok(),
            _ => None,
        })
        .unwrap_or(spec.extra);
    usize::from(requested).min(usize::from(spec.max_count).saturating_sub(stored))
}

fn snapshot_fields(
    fields: &[ferrioxid_core::AdminFieldMeta],
    object: &Map<String, Value>,
) -> Map<String, Value> {
    let mut snapshot = Map::new();
    for field in fields {
        if field.hidden || field.sensitive || crate::registry::sensitive(field.name) {
            continue;
        }
        if let Some(value) = object.get(field.name) {
            snapshot.insert(field.name.to_owned(), Value::String(canonical(value)));
        }
    }
    snapshot
}

fn canonical(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

fn fingerprint(parent: &Map<String, Value>, children: &[(String, Map<String, Value>)]) -> String {
    let mut text = String::new();
    let mut names: Vec<&String> = parent.keys().collect();
    names.sort();
    for name in names {
        text.push_str(name);
        text.push('=');
        text.push_str(parent.get(name).and_then(Value::as_str).unwrap_or(""));
        text.push('\n');
    }
    let mut rows = children.to_vec();
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    for (id, fields) in rows {
        text.push_str(&id);
        text.push('\n');
        let mut names: Vec<&String> = fields.keys().collect();
        names.sort();
        for name in names {
            text.push_str(name);
            text.push('=');
            text.push_str(fields.get(name).and_then(Value::as_str).unwrap_or(""));
            text.push('\n');
        }
    }
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn summary_of(before: &Map<String, Value>, after: &Map<String, Value>) -> String {
    let mut lines = Vec::new();
    for (key, value) in after {
        if key.ends_with("_id") {
            continue;
        }
        let next = canonical(value);
        let previous = before.get(key).map(canonical).unwrap_or_default();
        if previous != next {
            lines.push(format!("{key}: {previous} → {next}"));
        }
    }
    lines.join("\n").chars().take(400).collect()
}

async fn audit_parent(
    parent: &dyn ManagedModel,
    tx: &DatabaseTransaction,
    user: &AuthUser,
    id: &Uuid,
    operation: Uuid,
    action: &str,
) -> Result<(), FerrioxidError> {
    audit::insert(
        tx,
        &Event {
            actor_id: user.id,
            model_key: parent.meta().key.to_owned(),
            record_id: id.to_string(),
            action: action.to_owned(),
            outcome: "ok".to_owned(),
            summary: String::new(),
            operation_id: Some(operation),
        },
    )
    .await
}

async fn write_child_audit(
    tx: &DatabaseTransaction,
    user: &AuthUser,
    model: &str,
    id: Uuid,
    action: &str,
    summary: &str,
    operation: Uuid,
) -> Result<(), FerrioxidError> {
    audit::insert(
        tx,
        &Event {
            actor_id: user.id,
            model_key: model.to_owned(),
            record_id: id.to_string(),
            action: action.to_owned(),
            outcome: "ok".to_owned(),
            summary: summary.chars().take(400).collect(),
            operation_id: Some(operation),
        },
    )
    .await
}

fn redisplay(
    parent: &Map<String, Value>,
    form: &HashMap<String, String>,
    code: &str,
) -> Map<String, Value> {
    let mut values = parent.clone();
    for (key, value) in form {
        if key.starts_with("inline_") || key.starts_with("extra_") || key == "basis" {
            values.insert(key.clone(), Value::String(value.clone()));
        }
    }
    values.insert(
        "__error".to_owned(),
        serde_json::json!({ "field": code.split_whitespace().next().unwrap_or(""), "text": code }),
    );
    values.insert(
        "basis".to_owned(),
        Value::String(form.get("basis").cloned().unwrap_or_default()),
    );
    values
}

fn draft_map(parent: &dyn ManagedModel, form: &HashMap<String, String>) -> Map<String, Value> {
    let mut values = Map::new();
    for field in parent.meta().fields {
        if field.hidden || field.readonly || field.sensitive || field.name == "id" {
            continue;
        }
        if let Some(value) = form.get(field.name) {
            if value.chars().count() <= 2_000 {
                values.insert(field.name.to_owned(), Value::String(value.clone()));
            }
        }
    }
    for (key, value) in form {
        if (key.starts_with("inline_") || key.starts_with("extra_") || key == "basis")
            && value.chars().count() <= 2_000
            && !key.contains("password")
        {
            values.insert(key.clone(), Value::String(value.clone()));
        }
    }
    values
}

fn label(spec: &AdminInline, fallback: &str) -> String {
    if spec.label.is_empty() {
        fallback.to_owned()
    } else {
        spec.label.to_owned()
    }
}

fn linked(parent: &dyn ManagedModel, spec: &AdminInline, child: &dyn ManagedModel) -> bool {
    child.columns().iter().any(|column| {
        column.name == spec.column && crate::registry::references_parent(column, parent)
    })
}
