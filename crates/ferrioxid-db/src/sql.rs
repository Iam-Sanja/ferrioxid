//! SeaQuery-backed CRUD and DDL for SQLite and PostgreSQL.

use ferrioxid_core::{
    ColumnKind, ColumnSpec, DatabaseKind, FerrioxidError, FerrioxidModel, QueryScope,
    ReferentialAction, ScopeValue,
};
use sea_orm::sea_query::{
    Alias, ColumnDef, Expr, ForeignKey, ForeignKeyAction, Index, Order, PostgresQueryBuilder,
    Query, SqliteQueryBuilder, Table,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, QueryResult, Statement, Value,
};
use serde_json::{Map, Number, Value as Json};
use uuid::Uuid;

pub const LIST_LIMIT: usize = 100;

pub fn kind_of(backend: DatabaseBackend) -> Result<DatabaseKind, FerrioxidError> {
    match backend {
        DatabaseBackend::Sqlite => Ok(DatabaseKind::Sqlite),
        DatabaseBackend::Postgres => Ok(DatabaseKind::Postgres),
        DatabaseBackend::MySql => Err(FerrioxidError::database(UnsupportedBackend)),
    }
}

/// Spalte für DDL. CRUD und Migrationen nutzen dieselbe Typabbildung.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalColumn<'a> {
    pub name: &'a str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
}

impl<'a> From<&'a ColumnSpec> for PhysicalColumn<'a> {
    fn from(column: &'a ColumnSpec) -> Self {
        Self {
            name: column.name,
            kind: column.kind,
            nullable: column.nullable,
            primary_key: column.primary_key,
            default_bool: column.default_bool,
        }
    }
}

pub fn render_schema(kind: DatabaseKind, table: &str, columns: &[ColumnSpec]) -> String {
    let physical: Vec<PhysicalColumn<'_>> = columns.iter().map(PhysicalColumn::from).collect();
    inject_choice_checks(render_create_table(kind, table, &physical, true), columns)
}

pub fn render_create_table(
    kind: DatabaseKind,
    table: &str,
    columns: &[PhysicalColumn<'_>],
    if_not_exists: bool,
) -> String {
    let mut statement = Table::create();
    statement.table(Alias::new(table));
    if if_not_exists {
        statement.if_not_exists();
    }
    for column in columns {
        let mut definition = column_definition(kind, column);
        statement.col(&mut definition);
    }
    render_ddl(kind, statement)
}

pub struct ForeignKeyClause<'a> {
    pub name: &'a str,
    pub column: &'a str,
    pub referenced_table: &'a str,
    pub referenced_column: &'a str,
    pub on_delete: ReferentialAction,
}

pub fn render_table_bundle(
    kind: DatabaseKind,
    table: &str,
    columns: &[PhysicalColumn<'_>],
    uniques: &[(&str, &[&str])],
    foreign_keys: &[ForeignKeyClause<'_>],
    indexes: &[(&str, &[&str])],
) -> Vec<String> {
    let mut statement = Table::create();
    statement.table(Alias::new(table));
    for column in columns {
        let mut definition = column_definition(kind, column);
        statement.col(&mut definition);
    }
    for foreign_key in foreign_keys {
        let mut constraint = ForeignKey::create();
        constraint
            .name(foreign_key.name)
            .from_tbl(Alias::new(table))
            .from_col(Alias::new(foreign_key.column))
            .to_tbl(Alias::new(foreign_key.referenced_table))
            .to_col(Alias::new(foreign_key.referenced_column))
            .on_delete(foreign_key_action(foreign_key.on_delete))
            .on_update(ForeignKeyAction::NoAction);
        statement.foreign_key(&mut constraint);
    }
    let mut statements = vec![render_ddl(kind, statement)];
    for (name, unique_columns) in uniques {
        statements.push(render_create_index(kind, name, table, unique_columns, true));
    }
    for (name, index_columns) in indexes {
        statements.push(render_create_index(kind, name, table, index_columns, false));
    }
    statements
}

pub fn render_create_index(
    kind: DatabaseKind,
    name: &str,
    table: &str,
    columns: &[&str],
    unique: bool,
) -> String {
    let mut index = Index::create();
    index.name(name).table(Alias::new(table));
    if unique {
        index.unique();
    }
    for column in columns {
        index.col(Alias::new(*column));
    }
    render_ddl(kind, index)
}

pub fn render_drop_index(kind: DatabaseKind, name: &str) -> String {
    let mut index = Index::drop();
    index.name(name);
    render_ddl(kind, index)
}

pub fn render_add_column_referencing(
    kind: DatabaseKind,
    table: &str,
    column: &PhysicalColumn<'_>,
    foreign_key: Option<&ForeignKeyClause<'_>>,
) -> String {
    let mut sql = render_add_column(kind, table, column);
    if let Some(foreign_key) = foreign_key {
        sql.push_str(&format!(
            " CONSTRAINT \"{}\" REFERENCES \"{}\" (\"{}\") ON DELETE {} ON UPDATE NO ACTION",
            foreign_key.name,
            foreign_key.referenced_table,
            foreign_key.referenced_column,
            on_delete_sql(foreign_key.on_delete)
        ));
    }
    sql
}

fn foreign_key_action(action: ReferentialAction) -> ForeignKeyAction {
    match action {
        ReferentialAction::Restrict => ForeignKeyAction::Restrict,
        ReferentialAction::Cascade => ForeignKeyAction::Cascade,
        ReferentialAction::SetNull => ForeignKeyAction::SetNull,
    }
}

fn on_delete_sql(action: ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Restrict => "RESTRICT",
        ReferentialAction::Cascade => "CASCADE",
        ReferentialAction::SetNull => "SET NULL",
    }
}

pub fn render_add_column(kind: DatabaseKind, table: &str, column: &PhysicalColumn<'_>) -> String {
    let mut statement = Table::alter();
    let mut definition = column_definition(kind, column);
    statement
        .table(Alias::new(table))
        .add_column(&mut definition);
    render_ddl(kind, statement)
}

pub fn render_drop_table(kind: DatabaseKind, table: &str) -> String {
    let mut statement = Table::drop();
    statement.table(Alias::new(table));
    render_ddl(kind, statement)
}

pub fn render_rename_table(kind: DatabaseKind, from: &str, to: &str) -> String {
    let _ = kind;
    format!("ALTER TABLE \"{from}\" RENAME TO \"{to}\"")
}

pub fn render_rename_column(kind: DatabaseKind, table: &str, from: &str, to: &str) -> String {
    let _ = kind;
    format!("ALTER TABLE \"{table}\" RENAME COLUMN \"{from}\" TO \"{to}\"")
}

pub fn render_drop_column(kind: DatabaseKind, table: &str, column: &str) -> String {
    let mut statement = Table::alter();
    statement
        .table(Alias::new(table))
        .drop_column(Alias::new(column));
    render_ddl(kind, statement)
}

fn column_definition(kind: DatabaseKind, column: &PhysicalColumn<'_>) -> ColumnDef {
    let mut definition = ColumnDef::new(Alias::new(column.name));
    match (kind, column.kind) {
        (DatabaseKind::Postgres, ColumnKind::Uuid) => {
            definition.uuid();
        }
        (DatabaseKind::Sqlite, ColumnKind::Uuid) => {
            definition.text();
        }
        (_, ColumnKind::Text) => {
            definition.text();
        }
        (_, ColumnKind::BigInt) => {
            definition.big_integer();
        }
        (DatabaseKind::Postgres, ColumnKind::Boolean) => {
            definition.boolean();
        }
        (DatabaseKind::Sqlite, ColumnKind::Boolean) => {
            definition.integer();
        }
        (DatabaseKind::Postgres, ColumnKind::Date) => {
            definition.date();
        }
        (DatabaseKind::Sqlite, ColumnKind::Date) => {
            definition.text();
        }
        (DatabaseKind::Postgres, ColumnKind::Time) => {
            definition.time();
        }
        (DatabaseKind::Sqlite, ColumnKind::Time) => {
            definition.text();
        }
        (DatabaseKind::Postgres, ColumnKind::DateTime) => {
            definition.timestamp_with_time_zone();
        }
        (DatabaseKind::Sqlite, ColumnKind::DateTime) => {
            definition.text();
        }
        (_, ColumnKind::Decimal) => {
            definition.blob();
        }
        (_, ColumnKind::Choice) => {
            definition.text();
        }
    }
    if !column.nullable {
        definition.not_null();
    }
    if column.primary_key {
        definition.primary_key();
    }
    if let Some(default) = column.default_bool {
        match kind {
            DatabaseKind::Postgres => {
                definition.default(default);
            }
            DatabaseKind::Sqlite => {
                definition.default(i32::from(default));
            }
        }
    }
    definition
}

fn render_ddl(
    kind: DatabaseKind,
    statement: impl sea_orm::sea_query::SchemaStatementBuilder,
) -> String {
    match kind {
        DatabaseKind::Postgres => statement.to_string(PostgresQueryBuilder),
        DatabaseKind::Sqlite => statement.to_string(SqliteQueryBuilder),
    }
}

pub struct ValuePage {
    pub rows: Vec<Json>,
    pub limit: usize,
    pub truncated: bool,
}

pub async fn list_records(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    scope: &QueryScope,
) -> Result<ValuePage, FerrioxidError> {
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table))
        .order_by(Alias::new("id"), Order::Asc)
        .limit((LIST_LIMIT + 1) as u64);
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    let rows = db
        .query_all({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(FerrioxidError::database)?;
    let truncated = rows.len() > LIST_LIMIT;
    let rows = rows
        .into_iter()
        .take(LIST_LIMIT)
        .map(|row| row_to_json(&row, columns))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValuePage {
        rows,
        limit: LIST_LIMIT,
        truncated,
    })
}

pub struct AdminPageRequest<'a> {
    pub search_columns: &'a [&'a str],
    pub search: Option<&'a str>,
    pub sort_column: &'a str,
    pub descending: bool,
    pub equal_filters: &'a [(&'a str, Value)],
    /// `(column, inclusive_lower, value)`. `inclusive_lower` false means `< value`.
    pub bounds: &'a [(&'a str, bool, Value)],
    pub offset: u64,
    pub limit: u64,
    pub scope: &'a QueryScope,
}

pub struct AdminPage {
    pub rows: Vec<Json>,
    pub limit: usize,
    pub has_more: bool,
    pub total: i64,
}

pub async fn query_admin_page(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    request: AdminPageRequest<'_>,
) -> Result<AdminPage, FerrioxidError> {
    ensure_column(columns, request.sort_column)?;
    for name in request.search_columns {
        ensure_column(columns, name)?;
    }
    for (name, _) in request.equal_filters {
        ensure_column(columns, name)?;
    }
    for (name, _, _) in request.bounds {
        ensure_column(columns, name)?;
    }
    if request.limit == 0 || request.limit > LIST_LIMIT as u64 {
        return Err(FerrioxidError::Validation(
            "page size must be between 1 and 100".into(),
        ));
    }
    let mut count_query = Query::select();
    count_query
        .expr_as(
            Expr::col(Alias::new("id")).count(),
            Alias::new("scoped_count"),
        )
        .from(Alias::new(table));
    constrain(
        &mut count_query,
        columns,
        request.search_columns,
        request.search,
        request.equal_filters,
        request.bounds,
        request.scope,
    )?;
    let count_row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &count_query)?
        })
        .await
        .map_err(FerrioxidError::database)?
        .ok_or_else(|| {
            FerrioxidError::database(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "count returned no row",
            ))
        })?;
    let total = count_row
        .try_get::<i64>("", "scoped_count")
        .or_else(|_| count_row.try_get::<i32>("", "scoped_count").map(i64::from))
        .map_err(FerrioxidError::database)?;
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table));
    constrain(
        &mut query,
        columns,
        request.search_columns,
        request.search,
        request.equal_filters,
        request.bounds,
        request.scope,
    )?;
    query.order_by(
        Alias::new(request.sort_column),
        if request.descending {
            Order::Desc
        } else {
            Order::Asc
        },
    );
    if request.sort_column != "id" {
        query.order_by(Alias::new("id"), Order::Asc);
    }
    query.limit(request.limit + 1).offset(request.offset);
    let rows = db
        .query_all({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(FerrioxidError::database)?;
    let has_more = rows.len() > request.limit as usize;
    let rows = rows
        .into_iter()
        .take(request.limit as usize)
        .map(|row| row_to_json(&row, columns))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdminPage {
        rows,
        limit: request.limit as usize,
        has_more,
        total,
    })
}

/// Eine begrenzte Abfrage je 100 Bezeichner. Tabelle und Spalte müssen registrierte Identifier sein.
pub async fn query_labels(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    label: &str,
    ids: &[Uuid],
    scope: &QueryScope,
) -> Result<Vec<(String, String)>, FerrioxidError> {
    if !is_safe_ident(table) {
        return Err(FerrioxidError::Validation(format!(
            "table {table} is not available"
        )));
    }
    ensure_column(columns, "id")?;
    ensure_column(columns, label)?;
    if columns.iter().any(|column| {
        column.name == label && !matches!(column.kind, ColumnKind::Text | ColumnKind::Choice)
    }) {
        return Err(FerrioxidError::Validation(format!(
            "column {label} is not text"
        )));
    }
    let mut unique = Vec::new();
    for id in ids {
        if !unique.contains(id) {
            unique.push(*id);
        }
    }
    let mut found = Vec::new();
    for chunk in unique.chunks(100) {
        let mut query = Query::select();
        query
            .columns([Alias::new("id"), Alias::new(label)])
            .from(Alias::new(table))
            .limit(100);
        query.and_where(Expr::col(Alias::new("id")).is_in(chunk.iter().copied()));
        if let Some(condition) = scope_condition(columns, scope)? {
            query.and_where(condition);
        }
        let rows = db
            .query_all({
                crate::query::note_statement();
                statement(db, &query)?
            })
            .await
            .map_err(FerrioxidError::database)?;
        for row in rows {
            let id = row
                .try_get::<Option<String>>("", "id")
                .or_else(|_| {
                    row.try_get::<Option<Uuid>>("", "id")
                        .map(|value| value.map(|id| id.to_string()))
                })
                .map_err(FerrioxidError::database)?
                .unwrap_or_default();
            let text = row
                .try_get::<Option<String>>("", label)
                .map_err(FerrioxidError::database)?
                .unwrap_or_default();
            if !id.is_empty() && !text.trim().is_empty() {
                found.push((id, text));
            }
        }
    }
    Ok(found)
}

#[allow(clippy::too_many_arguments)]
pub async fn date_buckets(
    db: &DatabaseConnection,
    table: &str,
    columns: &[ColumnSpec],
    column: &str,
    width: u8,
    search_columns: &[&str],
    search: Option<&str>,
    equal_filters: &[(&str, Value)],
    bounds: &[(&str, bool, Value)],
    scope: &QueryScope,
) -> Result<Vec<(String, i64)>, FerrioxidError> {
    if !matches!(width, 4 | 7 | 10) || !is_safe_ident(table) {
        return Err(FerrioxidError::Validation("date bucket is invalid".into()));
    }
    ensure_column(columns, column)?;
    let kind = columns
        .iter()
        .find(|item| item.name == column)
        .map(|item| item.kind)
        .unwrap_or(ColumnKind::Text);
    let expression = bucket_expression(db.get_database_backend(), kind, column, width);
    let mut query = Query::select();
    query
        .expr_as(Expr::cust(&expression), Alias::new("bucket"))
        .expr_as(
            Expr::col(Alias::new("id")).count(),
            Alias::new("bucket_count"),
        )
        .from(Alias::new(table));
    constrain(
        &mut query,
        columns,
        search_columns,
        search,
        equal_filters,
        bounds,
        scope,
    )?;
    query.add_group_by([Expr::cust(&expression)]);
    query.order_by_expr(Expr::cust("bucket"), Order::Asc);
    query.limit(101);
    let rows = db
        .query_all({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(FerrioxidError::database)?;
    let mut buckets = Vec::new();
    for row in rows.into_iter().take(100) {
        let label = row
            .try_get::<String>("", "bucket")
            .map_err(FerrioxidError::database)?;
        let count = row
            .try_get::<i64>("", "bucket_count")
            .or_else(|_| row.try_get::<i32>("", "bucket_count").map(i64::from))
            .map_err(FerrioxidError::database)?;
        buckets.push((label, count));
    }
    Ok(buckets)
}

fn bucket_expression(
    backend: DatabaseBackend,
    kind: ColumnKind,
    column: &str,
    width: u8,
) -> String {
    let format = match width {
        4 => "YYYY",
        7 => "YYYY-MM",
        _ => "YYYY-MM-DD",
    };
    match (backend, kind) {
        (DatabaseBackend::Postgres, ColumnKind::DateTime) => {
            format!("to_char(\"{column}\" AT TIME ZONE 'UTC', '{format}')")
        }
        (DatabaseBackend::Postgres, _) => format!("to_char(\"{column}\", '{format}')"),
        _ => format!("substr(\"{column}\", 1, {width})"),
    }
}

fn constrain(
    query: &mut sea_orm::sea_query::SelectStatement,
    columns: &[ColumnSpec],
    search_columns: &[&str],
    search: Option<&str>,
    equal_filters: &[(&str, Value)],
    bounds: &[(&str, bool, Value)],
    scope: &QueryScope,
) -> Result<(), FerrioxidError> {
    if let Some(term) = search.filter(|term| !term.is_empty()) {
        let pattern = like_pattern(term);
        let mut any = Vec::new();
        for column in search_columns {
            ensure_column(columns, column)?;
            any.push(Expr::cust_with_values(
                format!("LOWER(\"{column}\") LIKE LOWER(?) ESCAPE '\\'"),
                [Value::from(pattern.clone())],
            ));
        }
        if let Some(first) = any.into_iter().reduce(|left, right| left.or(right)) {
            query.cond_where(first);
        }
    }
    for (column, value) in equal_filters {
        query.and_where(Expr::col(Alias::new(*column)).eq(value.clone()));
    }
    for (column, lower, value) in bounds {
        let column = Expr::col(Alias::new(*column));
        if *lower {
            query.and_where(column.gte(value.clone()));
        } else {
            query.and_where(column.lt(value.clone()));
        }
    }
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    Ok(())
}

fn ensure_column(columns: &[ColumnSpec], name: &str) -> Result<(), FerrioxidError> {
    if columns.iter().any(|column| column.name == name) && is_safe_ident(name) {
        Ok(())
    } else {
        Err(FerrioxidError::Validation(format!(
            "column {name} is not available"
        )))
    }
}

pub(crate) fn scope_condition(
    columns: &[ColumnSpec],
    scope: &QueryScope,
) -> Result<Option<sea_orm::sea_query::SimpleExpr>, FerrioxidError> {
    match scope {
        QueryScope::All => Ok(None),
        QueryScope::None => Ok(Some(Expr::val(0).eq(1))),
        QueryScope::Equals { column, value } => {
            ensure_column(columns, column)?;
            let expression = match value {
                ScopeValue::Text(text) => Expr::col(Alias::new(*column)).eq(text.clone()),
                ScopeValue::Uuid(id) => Expr::col(Alias::new(*column)).eq(*id),
                ScopeValue::BigInt(number) => Expr::col(Alias::new(*column)).eq(*number),
                ScopeValue::Bool(flag) => Expr::col(Alias::new(*column)).eq(*flag),
            };
            Ok(Some(expression))
        }
    }
}

pub(crate) fn is_safe_ident(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic() || char == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn like_pattern(term: &str) -> String {
    let mut escaped = String::new();
    for char in term.chars() {
        if matches!(char, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(char);
    }
    format!("%{escaped}%")
}

pub async fn get_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    scope: &QueryScope,
) -> Result<Option<Json>, FerrioxidError> {
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id));
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    let row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(FerrioxidError::database)?;
    row.map(|row| row_to_json(&row, columns)).transpose()
}

pub async fn insert_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    values: Vec<Value>,
) -> Result<Json, FerrioxidError> {
    let mut query = Query::insert();
    query
        .into_table(Alias::new(table))
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .values_panic(values.into_iter().map(Into::into))
        .returning_all();
    let row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(map_write)?
        .ok_or_else(|| FerrioxidError::Validation("no row returned".into()))?;
    row_to_json(&row, columns)
}

pub async fn update_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    changes: Vec<(&'static str, Value)>,
    scope: &QueryScope,
) -> Result<Json, FerrioxidError> {
    if changes.is_empty() {
        return get_record(db, table, columns, id, scope)
            .await?
            .ok_or(FerrioxidError::NotFound);
    }
    let mut query = Query::update();
    query
        .table(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id))
        .returning_all();
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    for (name, value) in changes {
        query.value(Alias::new(name), value);
    }
    let row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(map_write)?
        .ok_or(FerrioxidError::NotFound)?;
    row_to_json(&row, columns)
}

pub async fn delete_record(
    db: &impl ConnectionTrait,
    table: &str,
    id: Uuid,
    scope: &QueryScope,
    columns: &[ColumnSpec],
) -> Result<u64, FerrioxidError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id));
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    let result = db
        .execute({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(map_write)?;
    Ok(result.rows_affected())
}

pub async fn find_by_column<M>(
    db: &impl ConnectionTrait,
    column: &str,
    value: Value,
) -> Result<Vec<M>, FerrioxidError>
where
    M: FerrioxidModel + serde::de::DeserializeOwned,
{
    let columns = M::columns();
    if !columns.iter().any(|candidate| candidate.name == column) {
        return Err(FerrioxidError::Validation(format!(
            "column {column} is not part of {}",
            M::descriptor().table
        )));
    }
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|candidate| Alias::new(candidate.name)))
        .from(Alias::new(M::descriptor().table))
        .and_where(Expr::col(Alias::new(column)).eq(value))
        .order_by(Alias::new("id"), Order::Asc);
    let rows = db
        .query_all({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(FerrioxidError::database)?;
    rows.into_iter()
        .map(|row| {
            let json = row_to_json(&row, columns)?;
            serde_json::from_value(json).map_err(FerrioxidError::database)
        })
        .collect()
}

fn map_write(error: sea_orm::DbErr) -> FerrioxidError {
    if let Some(mapped) = constraint_violation(&error) {
        return mapped;
    }
    FerrioxidError::database(error)
}

fn constraint_violation(error: &sea_orm::DbErr) -> Option<FerrioxidError> {
    let database = sqlite_or_postgres_database(error)?;
    match database.kind() {
        sea_orm::sqlx::error::ErrorKind::UniqueViolation => Some(FerrioxidError::Validation(
            "unique constraint violated".into(),
        )),
        sea_orm::sqlx::error::ErrorKind::ForeignKeyViolation => Some(FerrioxidError::Validation(
            "foreign key constraint violated".into(),
        )),
        // SQLite setzt ON DELETE/UPDATE RESTRICT als internes RAISE um.
        // Der erweiterte Code ist dann 1811 (SQLITE_CONSTRAINT_TRIGGER) und nicht 787.
        // Die Meldung ist der feste Text dieser Implementierung.
        sea_orm::sqlx::error::ErrorKind::Other
            if database.code().as_deref() == Some("1811")
                && database.message() == "FOREIGN KEY constraint failed" =>
        {
            Some(FerrioxidError::Validation(
                "foreign key constraint violated".into(),
            ))
        }
        _ => None,
    }
}

fn sqlite_or_postgres_database(
    error: &sea_orm::DbErr,
) -> Option<&dyn sea_orm::sqlx::error::DatabaseError> {
    let runtime = match error {
        sea_orm::DbErr::Exec(runtime) | sea_orm::DbErr::Query(runtime) => runtime,
        _ => return None,
    };
    let sea_orm::RuntimeErr::SqlxError(sqlx_error) = runtime else {
        return None;
    };
    let sea_orm::sqlx::Error::Database(database) = sqlx_error else {
        return None;
    };
    Some(database.as_ref())
}

pub(crate) fn statement(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Statement, FerrioxidError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => return Err(FerrioxidError::database(UnsupportedBackend)),
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

pub(crate) fn row_to_json(
    row: &QueryResult,
    columns: &[ColumnSpec],
) -> Result<Json, FerrioxidError> {
    let mut object = Map::new();
    for column in columns {
        let value = match column.kind {
            ColumnKind::Text => json_text(row, column)?,
            ColumnKind::BigInt => json_int(row, column)?,
            ColumnKind::Boolean => json_bool(row, column)?,
            ColumnKind::Uuid => json_uuid(row, column)?,
            ColumnKind::Date | ColumnKind::Time | ColumnKind::DateTime => {
                json_temporal(row, column)?
            }
            ColumnKind::Choice => json_text(row, column)?,
            ColumnKind::Decimal => json_decimal(row, column)?,
        };
        object.insert(column.name.to_owned(), value);
    }
    Ok(Json::Object(object))
}

fn inject_choice_checks(sql: String, columns: &[ColumnSpec]) -> String {
    let checks: Vec<String> = columns
        .iter()
        .filter(|column| column.kind == ColumnKind::Choice && !column.choices.is_empty())
        .map(|column| {
            let list = column
                .choices
                .iter()
                .map(|choice| format!("'{}'", choice.value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("CHECK (\"{}\" IN ({list}))", column.name)
        })
        .collect();
    with_checks(sql, &checks)
}

pub fn with_checks(sql: String, checks: &[String]) -> String {
    if checks.is_empty() {
        return sql;
    }
    let Some((body, tail)) = sql.rsplit_once(')') else {
        return sql;
    };
    format!("{body}, {}){tail}", checks.join(", "))
}

fn json_decimal(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    let value = row
        .try_get::<Option<Vec<u8>>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(bytes) => {
            let decimal =
                ferrioxid_core::decode_decimal(&bytes, column.scale).map_err(|error| {
                    FerrioxidError::Validation(format!("{} {}", column.name, error.message))
                })?;
            let text = ferrioxid_core::format_decimal(decimal, column.scale).map_err(|error| {
                FerrioxidError::Validation(format!("{} {}", column.name, error.message))
            })?;
            Json::String(text)
        }
        None => Json::Null,
    })
}

fn json_text(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(text) => Json::String(text),
        None => Json::Null,
    })
}

fn json_int(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column.name) {
        return Ok(match value {
            Some(number) => Json::Number(Number::from(number)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<i32>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(number) => Json::Number(Number::from(number)),
        None => Json::Null,
    })
}

fn json_bool(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    if let Ok(value) = row.try_get::<Option<bool>>("", column.name) {
        return Ok(match value {
            Some(flag) => Json::Bool(flag),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<i64>>("", column.name) {
        return Ok(match value {
            Some(flag) => Json::Bool(flag != 0),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<i32>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(flag) => Json::Bool(flag != 0),
        None => Json::Null,
    })
}

fn json_temporal(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    if let Ok(value) = row.try_get::<Option<String>>("", column.name) {
        return Ok(match value {
            Some(text) => Json::String(canonical_temporal(&text)),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDate>>("", column.name) {
        return Ok(match value {
            Some(date) => Json::String(ferrioxid_core::temporal::format_date(date)),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveTime>>("", column.name) {
        return Ok(match value {
            Some(time) => Json::String(ferrioxid_core::temporal::format_time(time)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(instant) => Json::String(ferrioxid_core::temporal::format_datetime(instant)),
        None => Json::Null,
    })
}

fn canonical_temporal(text: &str) -> String {
    if let Ok(date) = ferrioxid_core::parse_date(text) {
        return ferrioxid_core::temporal::format_date(date);
    }
    if let Ok(time) = ferrioxid_core::parse_time(text) {
        if text.len() <= 8 {
            return ferrioxid_core::temporal::format_time(time);
        }
    }
    if let Ok(instant) = ferrioxid_core::parse_datetime_utc(text) {
        return ferrioxid_core::temporal::format_datetime(instant);
    }
    text.to_owned()
}

fn json_uuid(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FerrioxidError> {
    if let Ok(value) = row.try_get::<Option<Uuid>>("", column.name) {
        return Ok(match value {
            Some(id) => Json::String(id.to_string()),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FerrioxidError::database)?;
    Ok(match value {
        Some(text) => Json::String(text),
        None => Json::Null,
    })
}

#[derive(Debug)]
struct UnsupportedBackend;

impl std::fmt::Display for UnsupportedBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("only sqlite and postgres are supported")
    }
}

impl std::error::Error for UnsupportedBackend {}
