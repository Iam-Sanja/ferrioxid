//! SeaQuery-backed CRUD and DDL for SQLite and PostgreSQL.

use crate::driver::sea_query::{
    Alias, ColumnDef, Expr, ForeignKey, ForeignKeyAction, Func, Index, LikeExpr, Order,
    PostgresQueryBuilder, Query, SqliteQueryBuilder, Table,
};
use crate::driver::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, QueryResult, Statement, Value,
};
use fexu_core::{
    ColumnKind, ColumnSpec, DatabaseKind, FexuError, FexuModel, QueryScope, ReferentialAction,
    ScopeValue,
};
use serde_json::{Map, Number, Value as Json};
use uuid::Uuid;

pub const LIST_LIMIT: usize = 100;

pub fn kind_of(backend: DatabaseBackend) -> Result<DatabaseKind, FexuError> {
    match backend {
        DatabaseBackend::Sqlite => Ok(DatabaseKind::Sqlite),
        DatabaseBackend::Postgres => Ok(DatabaseKind::Postgres),
        DatabaseBackend::MySql => Err(FexuError::database(UnsupportedBackend)),
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
    let sql = inject_choice_checks(render_create_table(kind, table, &physical, true), columns);
    if kind != DatabaseKind::Sqlite {
        return sql;
    }
    let checks: Vec<String> = columns
        .iter()
        .filter(|column| column.kind == ColumnKind::Json)
        .map(|column| {
            format!(
                "CHECK (\"{}\" IS NULL OR json_valid(\"{}\"))",
                column.name, column.name
            )
        })
        .collect();
    with_checks(sql, &checks)
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
        (DatabaseKind::Postgres, ColumnKind::Json) => {
            definition.json_binary();
        }
        (DatabaseKind::Sqlite, ColumnKind::Json) => {
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
    statement: impl crate::driver::sea_query::SchemaStatementBuilder,
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
) -> Result<ValuePage, FexuError> {
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
        .map_err(FexuError::database)?;
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
) -> Result<AdminPage, FexuError> {
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
        return Err(FexuError::Validation(
            "page size must be between 1 and 100".into(),
        ));
    }
    let backend = db.get_database_backend();
    let mut count_query = Query::select();
    count_query
        .expr_as(
            Expr::col(Alias::new("id")).count(),
            Alias::new("scoped_count"),
        )
        .from(Alias::new(table));
    constrain(
        &mut count_query,
        backend,
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
        .map_err(FexuError::database)?
        .ok_or_else(|| {
            FexuError::database(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "count returned no row",
            ))
        })?;
    let total = count_row
        .try_get::<i64>("", "scoped_count")
        .or_else(|_| count_row.try_get::<i32>("", "scoped_count").map(i64::from))
        .map_err(FexuError::database)?;
    let mut query = Query::select();
    query
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .from(Alias::new(table));
    constrain(
        &mut query,
        backend,
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
        .map_err(FexuError::database)?;
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
) -> Result<Vec<(String, String)>, FexuError> {
    if !is_safe_ident(table) {
        return Err(FexuError::Validation(format!(
            "table {table} is not available"
        )));
    }
    ensure_column(columns, "id")?;
    ensure_column(columns, label)?;
    if columns.iter().any(|column| {
        column.name == label && !matches!(column.kind, ColumnKind::Text | ColumnKind::Choice)
    }) {
        return Err(FexuError::Validation(format!("column {label} is not text")));
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
            .map_err(FexuError::database)?;
        for row in rows {
            let id = row
                .try_get::<Option<String>>("", "id")
                .or_else(|_| {
                    row.try_get::<Option<Uuid>>("", "id")
                        .map(|value| value.map(|id| id.to_string()))
                })
                .map_err(FexuError::database)?
                .unwrap_or_default();
            let text = row
                .try_get::<Option<String>>("", label)
                .map_err(FexuError::database)?
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
) -> Result<Vec<(String, i64)>, FexuError> {
    if !matches!(width, 4 | 7 | 10) || !is_safe_ident(table) {
        return Err(FexuError::Validation("date bucket is invalid".into()));
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
        db.get_database_backend(),
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
        .map_err(FexuError::database)?;
    let mut buckets = Vec::new();
    for row in rows.into_iter().take(100) {
        let label = row
            .try_get::<String>("", "bucket")
            .map_err(FexuError::database)?;
        let count = row
            .try_get::<i64>("", "bucket_count")
            .or_else(|_| row.try_get::<i32>("", "bucket_count").map(i64::from))
            .map_err(FexuError::database)?;
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

#[allow(clippy::too_many_arguments)]
fn constrain(
    query: &mut crate::driver::sea_query::SelectStatement,
    backend: DatabaseBackend,
    columns: &[ColumnSpec],
    search_columns: &[&str],
    search: Option<&str>,
    equal_filters: &[(&str, Value)],
    bounds: &[(&str, bool, Value)],
    scope: &QueryScope,
) -> Result<(), FexuError> {
    if let Some(term) = search.filter(|term| !term.is_empty()) {
        let pattern = fold_case_insensitive_pattern(like_pattern(term), backend);
        let mut any = Vec::new();
        for column in search_columns {
            ensure_column(columns, column)?;
            let column_expr = Expr::col(Alias::new(*column));
            any.push(
                Expr::expr(Func::cust("LOWER").arg(column_expr))
                    .like(LikeExpr::new(pattern.clone()).escape('\\')),
            );
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

fn ensure_column(columns: &[ColumnSpec], name: &str) -> Result<(), FexuError> {
    if columns.iter().any(|column| column.name == name) && is_safe_ident(name) {
        Ok(())
    } else {
        Err(FexuError::Validation(format!(
            "column {name} is not available"
        )))
    }
}

pub(crate) fn scope_condition(
    columns: &[ColumnSpec],
    scope: &QueryScope,
) -> Result<Option<crate::driver::sea_query::SimpleExpr>, FexuError> {
    scope_condition_in(columns, scope, None)
}

pub(crate) fn scope_condition_in(
    columns: &[ColumnSpec],
    scope: &QueryScope,
    alias: Option<&str>,
) -> Result<Option<crate::driver::sea_query::SimpleExpr>, FexuError> {
    match scope {
        QueryScope::All => Ok(None),
        QueryScope::None => Ok(Some(Expr::val(0).eq(1))),
        QueryScope::Equals { column, value } => {
            ensure_column(columns, column)?;
            let column = qualified_col(alias, column);
            let expression = match value {
                ScopeValue::Text(text) => column.eq(text.clone()),
                ScopeValue::Uuid(id) => column.eq(*id),
                ScopeValue::BigInt(number) => column.eq(*number),
                ScopeValue::Bool(flag) => column.eq(*flag),
            };
            Ok(Some(expression))
        }
    }
}

fn qualified_col(alias: Option<&str>, name: &str) -> crate::driver::sea_query::SimpleExpr {
    match alias {
        Some(alias) => {
            crate::driver::sea_query::Expr::col((Alias::new(alias), Alias::new(name))).into()
        }
        None => crate::driver::sea_query::Expr::col(Alias::new(name)).into(),
    }
}

pub(crate) fn is_safe_ident(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic() || char == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

pub(crate) fn escape_like(term: &str) -> String {
    let mut escaped = String::new();
    for char in term.chars() {
        if matches!(char, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(char);
    }
    escaped
}

fn like_pattern(term: &str) -> String {
    format!("%{}%", escape_like(term))
}

/// Folds a LIKE pattern so it matches `LOWER(column)` on the active backend.
///
/// SQLite `LOWER` affects ASCII letters only, so the pattern uses
/// [`str::to_ascii_lowercase`]. PostgreSQL follows `LOWER` with locale-aware
/// Unicode rules, so the pattern uses [`str::to_lowercase`]. The same rules
/// are used by the model text filters. Neither backend promises identical
/// results for `München` and `MÜNCHEN`.
pub(crate) fn fold_case_insensitive_pattern(pattern: String, backend: DatabaseBackend) -> String {
    match backend {
        DatabaseBackend::Sqlite => pattern.to_ascii_lowercase(),
        DatabaseBackend::Postgres | DatabaseBackend::MySql => pattern.to_lowercase(),
    }
}

pub async fn get_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    scope: &QueryScope,
) -> Result<Option<Json>, FexuError> {
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
        .map_err(FexuError::database)?;
    row.map(|row| row_to_json(&row, columns)).transpose()
}

pub async fn insert_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    values: Vec<Value>,
) -> Result<Json, FexuError> {
    let mut prepared = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        let value = match columns.get(index) {
            Some(column) => json_parameter(column, value)?,
            None => value,
        };
        prepared.push(value);
    }
    let mut query = Query::insert();
    query
        .into_table(Alias::new(table))
        .columns(columns.iter().map(|column| Alias::new(column.name)))
        .values_panic(prepared.into_iter().map(Into::into))
        .returning_all();
    let row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(map_write)?
        .ok_or_else(|| FexuError::Validation("no row returned".into()))?;
    row_to_json(&row, columns)
}

pub async fn update_record(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    id: Uuid,
    changes: Vec<(&'static str, Value)>,
    scope: &QueryScope,
) -> Result<Json, FexuError> {
    if changes.is_empty() {
        return get_record(db, table, columns, id, scope)
            .await?
            .ok_or(FexuError::NotFound);
    }
    let mut prepared = Vec::with_capacity(changes.len());
    for (name, value) in changes {
        let value = match columns.iter().find(|column| column.name == name) {
            Some(column) => json_parameter(column, value)?,
            None => value,
        };
        prepared.push((name, value));
    }
    let mut query = Query::update();
    query
        .table(Alias::new(table))
        .and_where(Expr::col(Alias::new("id")).eq(id))
        .returning_all();
    if let Some(condition) = scope_condition(columns, scope)? {
        query.and_where(condition);
    }
    for (name, value) in prepared {
        query.value(Alias::new(name), value);
    }
    let row = db
        .query_one({
            crate::query::note_statement();
            statement(db, &query)?
        })
        .await
        .map_err(map_write)?
        .ok_or(FexuError::NotFound)?;
    row_to_json(&row, columns)
}

pub async fn delete_record(
    db: &impl ConnectionTrait,
    table: &str,
    id: Uuid,
    scope: &QueryScope,
    columns: &[ColumnSpec],
) -> Result<u64, FexuError> {
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
) -> Result<Vec<M>, FexuError>
where
    M: FexuModel + serde::de::DeserializeOwned,
{
    let columns = M::columns();
    if !columns.iter().any(|candidate| candidate.name == column) {
        return Err(FexuError::Validation(format!(
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
        .map_err(FexuError::database)?;
    rows.into_iter()
        .map(|row| {
            let json = row_to_json(&row, columns)?;
            serde_json::from_value(json).map_err(FexuError::database)
        })
        .collect()
}

fn json_parameter(column: &ColumnSpec, value: Value) -> Result<Value, FexuError> {
    if column.kind != ColumnKind::Json {
        return Ok(value);
    }
    match value {
        Value::Json(document) => Ok(Value::Json(document)),
        Value::String(None) => Ok(Value::Json(None)),
        Value::String(Some(text)) => {
            let canonical = fexu_core::canonical_json_text(text.as_str())
                .map_err(|_| FexuError::Validation("validation.json".into()))?;
            let parsed = serde_json::from_str::<Json>(&canonical)
                .map_err(|_| FexuError::Validation("validation.json".into()))?;
            Ok(Value::Json(Some(Box::new(parsed))))
        }
        _ => Err(FexuError::Validation("validation.json".into())),
    }
}

pub(crate) fn map_write(error: crate::driver::DbErr) -> FexuError {
    if let Some(mapped) = constraint_violation(&error) {
        return mapped;
    }
    FexuError::database(error)
}

fn constraint_violation(error: &crate::driver::DbErr) -> Option<FexuError> {
    // PostgreSQL 23001 and SQLite extended code 1811 are classified by the driver.
    match error.constraint() {
        Some(crate::driver::ConstraintKind::Unique) => {
            Some(FexuError::Validation("unique constraint violated".into()))
        }
        Some(crate::driver::ConstraintKind::ForeignKey) => Some(FexuError::Validation(
            "foreign key constraint violated".into(),
        )),
        None => None,
    }
}

pub(crate) fn statement(
    db: &impl ConnectionTrait,
    query: &impl crate::driver::sea_query::QueryStatementWriter,
) -> Result<Statement, FexuError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => return Err(FexuError::database(UnsupportedBackend)),
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

pub(crate) fn row_to_json(row: &QueryResult, columns: &[ColumnSpec]) -> Result<Json, FexuError> {
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
            ColumnKind::Json => json_document(row, column)?,
        };
        object.insert(column.name.to_owned(), value);
    }
    Ok(Json::Object(object))
}

fn json_document(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    if let Ok(value) = row.try_get::<Option<Json>>("", column.name) {
        return match value {
            Some(document) => Ok(Json::String(
                fexu_core::canonical_json_text(&document.to_string())
                    .map_err(|code| FexuError::Validation(code.into()))?,
            )),
            None => Ok(Json::Null),
        };
    }
    let text = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FexuError::database)?;
    Ok(match text {
        Some(text) => Json::String(
            fexu_core::canonical_json_text(&text)
                .map_err(|code| FexuError::Validation(code.into()))?,
        ),
        None => Json::Null,
    })
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

fn json_decimal(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    let value = row
        .try_get::<Option<Vec<u8>>>("", column.name)
        .map_err(FexuError::database)?;
    Ok(match value {
        Some(bytes) => {
            let decimal = fexu_core::decode_decimal(&bytes, column.scale).map_err(|error| {
                FexuError::Validation(format!("{} {}", column.name, error.message))
            })?;
            let text = fexu_core::format_decimal(decimal, column.scale).map_err(|error| {
                FexuError::Validation(format!("{} {}", column.name, error.message))
            })?;
            Json::String(text)
        }
        None => Json::Null,
    })
}

fn json_text(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FexuError::database)?;
    Ok(match value {
        Some(text) => Json::String(text),
        None => Json::Null,
    })
}

fn json_int(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    if let Ok(value) = row.try_get::<Option<i64>>("", column.name) {
        return Ok(match value {
            Some(number) => Json::Number(Number::from(number)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<i32>>("", column.name)
        .map_err(FexuError::database)?;
    Ok(match value {
        Some(number) => Json::Number(Number::from(number)),
        None => Json::Null,
    })
}

fn json_bool(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
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
        .map_err(FexuError::database)?;
    Ok(match value {
        Some(flag) => Json::Bool(flag != 0),
        None => Json::Null,
    })
}

fn json_temporal(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    if let Ok(value) = row.try_get::<Option<String>>("", column.name) {
        return Ok(match value {
            Some(text) => Json::String(canonical_temporal(&text)),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDate>>("", column.name) {
        return Ok(match value {
            Some(date) => Json::String(fexu_core::temporal::format_date(date)),
            None => Json::Null,
        });
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveTime>>("", column.name) {
        return Ok(match value {
            Some(time) => Json::String(fexu_core::temporal::format_time(time)),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", column.name)
        .map_err(FexuError::database)?;
    Ok(match value {
        Some(instant) => Json::String(fexu_core::temporal::format_datetime(instant)),
        None => Json::Null,
    })
}

fn canonical_temporal(text: &str) -> String {
    if let Ok(date) = fexu_core::parse_date(text) {
        return fexu_core::temporal::format_date(date);
    }
    if let Ok(time) = fexu_core::parse_time(text) {
        if text.len() <= 8 {
            return fexu_core::temporal::format_time(time);
        }
    }
    if let Ok(instant) = fexu_core::parse_datetime_utc(text) {
        return fexu_core::temporal::format_datetime(instant);
    }
    text.to_owned()
}

fn json_uuid(row: &QueryResult, column: &ColumnSpec) -> Result<Json, FexuError> {
    if let Ok(value) = row.try_get::<Option<Uuid>>("", column.name) {
        return Ok(match value {
            Some(id) => Json::String(id.to_string()),
            None => Json::Null,
        });
    }
    let value = row
        .try_get::<Option<String>>("", column.name)
        .map_err(FexuError::database)?;
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

#[cfg(test)]
mod tests {
    use crate::driver::sea_query::{Alias, PostgresQueryBuilder, Query};
    use crate::driver::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
    use fexu_core::{ColumnKind, ColumnSpec, QueryScope, ScopeValue};
    use uuid::Uuid;

    use super::{constrain, query_admin_page, AdminPageRequest};
    use crate::connect;

    fn column(name: &'static str, kind: ColumnKind, primary_key: bool) -> ColumnSpec {
        ColumnSpec {
            name,
            kind,
            nullable: false,
            primary_key,
            default_bool: None,
            unique: false,
            indexed: false,
            foreign_table: None,
            foreign_column: None,
            on_delete: None,
            precision: 0,
            scale: 0,
            choices: &[],
        }
    }

    fn columns() -> Vec<ColumnSpec> {
        vec![
            column("id", ColumnKind::Uuid, true),
            column("name", ColumnKind::Text, false),
            column("sku", ColumnKind::Text, false),
            column("owner_id", ColumnKind::Uuid, false),
        ]
    }

    fn owner_scope(owner: u128) -> QueryScope {
        QueryScope::Equals {
            column: "owner_id",
            value: ScopeValue::Uuid(Uuid::from_u128(owner)),
        }
    }

    #[test]
    fn postgres_admin_search_binds_the_pattern_without_a_question_mark() {
        let mut query = Query::select();
        query
            .column(Alias::new("id"))
            .from(Alias::new("fexu_admin_search"));
        constrain(
            &mut query,
            DatabaseBackend::Postgres,
            &columns(),
            &["name", "sku"],
            Some("Hemd%_\\"),
            &[],
            &[],
            &QueryScope::All,
        )
        .unwrap();
        let (sql, values) = query.build(PostgresQueryBuilder);
        assert!(sql.contains("$1"));
        assert!(sql.contains("$2"));
        assert!(!sql.contains('?'));
        assert!(sql.contains("ESCAPE"));
        assert!(!sql.contains("Hemd"));
        assert!(!sql.contains("DROP"));
        assert_eq!(values.0.len(), 2);
        for value in &values.0 {
            match value {
                Value::String(Some(pattern)) => {
                    assert_eq!(pattern.as_str(), "%hemd\\%\\_\\\\%");
                }
                other => panic!("expected a bound text pattern, got {other:?}"),
            }
        }
        let sqlite = Query::select()
            .column(Alias::new("id"))
            .from(Alias::new("fexu_admin_search"))
            .to_owned();
        let mut sqlite = sqlite;
        constrain(
            &mut sqlite,
            DatabaseBackend::Sqlite,
            &columns(),
            &["name"],
            Some("MÜNCHEN"),
            &[],
            &[],
            &QueryScope::All,
        )
        .unwrap();
        let (_, sqlite_values) = sqlite.build(crate::driver::sea_query::SqliteQueryBuilder);
        match &sqlite_values.0[0] {
            Value::String(Some(pattern)) => assert_eq!(pattern.as_str(), "%mÜnchen%"),
            other => panic!("expected sqlite ASCII folding, got {other:?}"),
        }
    }

    async fn create_search_table(db: &DatabaseConnection) {
        let sql = match db.get_database_backend() {
            DatabaseBackend::Sqlite => {
                "CREATE TABLE fexu_admin_search (id text PRIMARY KEY, name text NOT NULL, sku text NOT NULL, owner_id text NOT NULL)"
            }
            DatabaseBackend::Postgres => {
                "CREATE TABLE fexu_admin_search (id uuid PRIMARY KEY, name text NOT NULL, sku text NOT NULL, owner_id uuid NOT NULL)"
            }
            DatabaseBackend::MySql => panic!("mysql is not a test backend"),
        };
        db.execute(Statement::from_string(
            db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap();
    }

    async fn seed(db: &DatabaseConnection) {
        let rows = [
            (1_u128, "Hemd", "SKU-1", 1_u128),
            (2, "100%", "SKU-2", 1),
            (3, "a_b", "SKU-3", 2),
            (4, "a\\b", "SKU-4", 1),
            (5, "München", "city", 1),
            (6, "shirt-a", "S1", 1),
            (7, "shirt-b", "S2", 1),
            (8, "shirt-c", "S3", 1),
        ];
        for (id, name, sku, owner) in rows {
            crate::insert_record(
                db,
                "fexu_admin_search",
                &columns(),
                vec![
                    Value::Uuid(Some(Box::new(Uuid::from_u128(id)))),
                    Value::String(Some(Box::new(name.to_owned()))),
                    Value::String(Some(Box::new(sku.to_owned()))),
                    Value::Uuid(Some(Box::new(Uuid::from_u128(owner)))),
                ],
            )
            .await
            .unwrap();
        }
    }

    async fn search<'a>(
        db: &DatabaseConnection,
        term: Option<&'a str>,
        scope: &'a QueryScope,
        offset: u64,
        limit: u64,
    ) -> super::AdminPage {
        query_admin_page(
            db,
            "fexu_admin_search",
            &columns(),
            AdminPageRequest {
                search_columns: &["name", "sku"],
                search: term,
                sort_column: "name",
                descending: false,
                equal_filters: &[],
                bounds: &[],
                offset,
                limit,
                scope,
            },
        )
        .await
        .unwrap()
    }

    async fn exercise_admin_search(db: &DatabaseConnection) {
        let owner = owner_scope(1);
        let all = QueryScope::All;
        assert_eq!(search(db, Some("Hemd"), &all, 0, 10).await.total, 1);
        assert_eq!(search(db, Some("hemd"), &all, 0, 10).await.total, 1);
        assert_eq!(search(db, Some("%"), &all, 0, 10).await.total, 1);
        assert_eq!(search(db, Some("_"), &all, 0, 10).await.total, 1);
        assert_eq!(search(db, Some("\\"), &all, 0, 10).await.total, 1);
        assert_eq!(search(db, Some("SKU-1"), &all, 0, 10).await.total, 1);
        assert_eq!(
            search(
                db,
                Some("'); DROP TABLE fexu_admin_search; --"),
                &all,
                0,
                10
            )
            .await
            .total,
            0
        );
        let empty = search(db, Some(""), &owner, 0, 10).await;
        assert_eq!(empty.total, 7);
        assert_eq!(search(db, None, &QueryScope::None, 0, 10).await.total, 0);
        let shirts = search(db, Some("shirt"), &owner, 0, 2).await;
        assert_eq!(shirts.total, 3);
        assert_eq!(shirts.rows.len(), 2);
        assert!(shirts.has_more);
        let next = search(db, Some("shirt"), &owner, 2, 2).await;
        assert_eq!(next.total, 3);
        assert_eq!(next.rows.len(), 1);
        assert!(!next.has_more);
        let hidden = search(db, Some("a_b"), &owner, 0, 10).await;
        assert_eq!(hidden.total, 0);
        let visible = search(db, Some("a_b"), &all, 0, 10).await;
        assert_eq!(visible.total, 1);
        let folded = search(db, Some("MÜNCHEN"), &all, 0, 10).await.total;
        match db.get_database_backend() {
            DatabaseBackend::Sqlite => assert_eq!(folded, 0),
            DatabaseBackend::Postgres => assert_eq!(folded, 1),
            DatabaseBackend::MySql => panic!("mysql is not a test backend"),
        }
        let rejected = query_admin_page(
            db,
            "fexu_admin_search",
            &columns(),
            AdminPageRequest {
                search_columns: &["missing"],
                search: Some("Hemd"),
                sort_column: "name",
                descending: false,
                equal_filters: &[],
                bounds: &[],
                offset: 0,
                limit: 10,
                scope: &all,
            },
        )
        .await;
        assert!(rejected.is_err());
    }

    #[tokio::test]
    async fn sqlite_admin_search_matches_literals_scopes_and_pages() {
        let db = connect("sqlite::memory:").await.unwrap();
        create_search_table(&db).await;
        seed(&db).await;
        exercise_admin_search(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_admin_search_runtime() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = crate::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        let db = connect(disposable.url()).await.expect("postgres");
        create_search_table(&db).await;
        seed(&db).await;
        exercise_admin_search(&db).await;
        db.close().await.expect("close");
        disposable
            .drop_database()
            .await
            .expect("drop isolated database");
    }
}
