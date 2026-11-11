//! Filtered writes, arithmetic updates, row locks and conflict handling.
//!
//! These statements are generated from the model column list. Application
//! code does not pass SQL text. Bulk writes do not call the single-row
//! `create` / `update` / `delete` path and therefore do not repeat its field
//! rules. A unique violation stays an error unless the insert explicitly
//! ignores that conflict. Overwriting on conflict is not offered.

use ferrioxid_core::{ColumnKind, ColumnSpec, FerrioxidError, FerrioxidModel, NumericBound};
use sea_orm::sea_query::{Alias, Expr, LockType, OnConflict, Query, SimpleExpr, Value};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement};
use std::marker::PhantomData;

use crate::query::{bind_for_column, Bound, Select};
use crate::sql::{self, is_safe_ident};

/// Rows changed by one insert, update or delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteResult {
    pub rows_affected: u64,
}

/// Rows locked by [`Select::lock_existing`].
///
/// `rows == 0` means no existing row matched. Nothing was reserved for a
/// later insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockResult {
    pub rows: u64,
}

enum Assignment {
    Set { column: &'static str, value: Bound },
    Null { column: &'static str },
    Add { column: &'static str, amount: i64 },
    Sub { column: &'static str, amount: i64 },
}

/// A filtered or explicitly unfiltered `UPDATE`.
#[must_use = "call execute to run the update"]
pub struct ColumnUpdate<M> {
    query: Select<M>,
    allow_all: bool,
    assignments: Vec<Assignment>,
}

impl<M> ColumnUpdate<M> {
    pub(crate) fn new(query: Select<M>, allow_all: bool) -> Self {
        Self {
            query,
            allow_all,
            assignments: Vec::new(),
        }
    }

    pub fn set(mut self, column: &'static str, value: impl Into<Bound>) -> Self {
        self.assignments.push(Assignment::Set {
            column,
            value: value.into(),
        });
        self
    }

    /// Sets a nullable column to SQL `NULL`.
    pub fn set_null(mut self, column: &'static str) -> Self {
        self.assignments.push(Assignment::Null { column });
        self
    }

    /// Adds `amount` in the database. `amount` must be greater than zero.
    ///
    /// The addition is one expression in the `UPDATE`. A result outside `i64`
    /// or the model's numeric bound matches no row instead of wrapping.
    pub fn increment(mut self, column: &'static str, amount: i64) -> Self {
        self.assignments.push(Assignment::Add { column, amount });
        self
    }

    /// Subtracts `amount` in the database. `amount` must be greater than zero.
    pub fn decrement(mut self, column: &'static str, amount: i64) -> Self {
        self.assignments.push(Assignment::Sub { column, amount });
        self
    }

    pub async fn execute<C>(self, db: &C) -> Result<WriteResult, FerrioxidError>
    where
        M: FerrioxidModel,
        C: ConnectionTrait,
    {
        if self.assignments.is_empty() {
            return Err(FerrioxidError::Validation(
                "update has no assignments".into(),
            ));
        }
        wait_for_sqlite(db).await?;
        let backend = db.get_database_backend();
        let filter = self.query.write_filter(backend)?;
        if !self.allow_all && filter.is_none() {
            return Err(FerrioxidError::Validation(
                "refusing an unfiltered update; call update_all to update every row".into(),
            ));
        }
        let columns = M::columns();
        let mut seen = Vec::new();
        let mut query = Query::update();
        query.table(Alias::new(M::descriptor().table));
        if let Some(filter) = filter {
            query.and_where(filter);
        }
        for assignment in &self.assignments {
            let name = assignment.column();
            if seen.contains(&name) {
                return Err(FerrioxidError::Validation(format!(
                    "column {name} is assigned more than once"
                )));
            }
            seen.push(name);
            apply_assignment::<M>(&mut query, columns, assignment, backend)?;
        }
        let result = db
            .execute({
                crate::query::note_statement();
                sql::statement(db, &query)?
            })
            .await
            .map_err(sql::map_write)?;
        Ok(WriteResult {
            rows_affected: result.rows_affected(),
        })
    }
}

/// A filtered or explicitly unfiltered `DELETE`.
#[must_use = "call execute to run the delete"]
pub struct FilteredDelete<M> {
    query: Select<M>,
    allow_all: bool,
}

impl<M> FilteredDelete<M> {
    pub(crate) fn new(query: Select<M>, allow_all: bool) -> Self {
        Self { query, allow_all }
    }

    pub async fn execute<C>(self, db: &C) -> Result<WriteResult, FerrioxidError>
    where
        M: FerrioxidModel,
        C: ConnectionTrait,
    {
        wait_for_sqlite(db).await?;
        let backend = db.get_database_backend();
        let filter = self.query.write_filter(backend)?;
        if !self.allow_all && filter.is_none() {
            return Err(FerrioxidError::Validation(
                "refusing an unfiltered delete; call delete_all to delete every row".into(),
            ));
        }
        let mut query = Query::delete();
        query.from_table(Alias::new(M::descriptor().table));
        if let Some(filter) = filter {
            query.and_where(filter);
        }
        let result = db
            .execute({
                crate::query::note_statement();
                sql::statement(db, &query)?
            })
            .await
            .map_err(sql::map_write)?;
        Ok(WriteResult {
            rows_affected: result.rows_affected(),
        })
    }
}

/// One typed insert. Conflict handling is opt-in and does not overwrite.
#[must_use = "call execute to run the insert"]
pub struct RowInsert<M> {
    values: Vec<(&'static str, Bound)>,
    conflict: Option<&'static [&'static str]>,
    _model: PhantomData<M>,
}

impl<M: FerrioxidModel> RowInsert<M> {
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            conflict: None,
            _model: PhantomData,
        }
    }

    pub fn value(mut self, column: &'static str, value: impl Into<Bound>) -> Self {
        self.values.push((column, value.into()));
        self
    }

    /// Ignores the insert when `columns` already exist.
    ///
    /// `columns` must be the primary key, a unique column, or a
    /// `unique_together` group. The stored row is left unchanged.
    /// `rows_affected` is `0` when the conflict happens.
    pub fn on_conflict_do_nothing(mut self, columns: &'static [&'static str]) -> Self {
        self.conflict = Some(columns);
        self
    }

    pub async fn execute<C>(self, db: &C) -> Result<WriteResult, FerrioxidError>
    where
        C: ConnectionTrait,
    {
        if self.values.is_empty() {
            return Err(FerrioxidError::Validation("insert has no values".into()));
        }
        wait_for_sqlite(db).await?;
        let columns = M::columns();
        let mut seen = Vec::new();
        let mut names = Vec::new();
        let mut bound = Vec::new();
        for (name, value) in &self.values {
            if seen.contains(name) {
                return Err(FerrioxidError::Validation(format!(
                    "column {name} is assigned more than once"
                )));
            }
            seen.push(*name);
            let column = column_spec(columns, name)?;
            names.push(*name);
            bound.push(bind_for_column(column, value, db.get_database_backend())?);
        }
        for column in columns {
            if column.nullable || column.default_bool.is_some() || seen.contains(&column.name) {
                continue;
            }
            return Err(FerrioxidError::Validation(format!(
                "column {} is required",
                column.name
            )));
        }
        if let Some(target) = self.conflict {
            require_unique_target::<M>(target)?;
        }
        let mut query = Query::insert();
        query
            .into_table(Alias::new(M::descriptor().table))
            .columns(names.into_iter().map(Alias::new))
            .values_panic(bound.into_iter().map(SimpleExpr::from));
        if let Some(target) = self.conflict {
            let idents: Vec<Alias> = target.iter().copied().map(Alias::new).collect();
            query.on_conflict(OnConflict::columns(idents).do_nothing().to_owned());
        }
        let result = db
            .execute({
                crate::query::note_statement();
                sql::statement(db, &query)?
            })
            .await
            .map_err(sql::map_write)?;
        Ok(WriteResult {
            rows_affected: result.rows_affected(),
        })
    }
}

pub(crate) async fn lock_existing<M: FerrioxidModel>(
    query: Select<M>,
    db: &DatabaseTransaction,
) -> Result<LockResult, FerrioxidError> {
    wait_for_sqlite(db).await?;
    let backend = db.get_database_backend();
    let Some(filter) = query.write_filter(backend)? else {
        return Err(FerrioxidError::Validation(
            "refusing to lock every row; add a filter or a scope".into(),
        ));
    };
    let table = M::descriptor().table;
    if !is_safe_ident(table) {
        return Err(FerrioxidError::Validation(format!(
            "table {table} is not available"
        )));
    }
    column_spec(M::columns(), "id")?;
    match backend {
        DatabaseBackend::Postgres => {
            let mut select = Query::select();
            select
                .column(Alias::new("id"))
                .from(Alias::new(table))
                .and_where(filter)
                .lock(LockType::Update);
            let rows = db
                .query_all({
                    crate::query::note_statement();
                    sql::statement(db, &select)?
                })
                .await
                .map_err(FerrioxidError::database)?;
            Ok(LockResult {
                rows: rows.len() as u64,
            })
        }
        DatabaseBackend::Sqlite => {
            // A preceding SELECT would take a shared lock. Two transactions
            // that then both try to write can deadlock and return SQLITE_BUSY
            // immediately. The write itself is the lock. Zero changed rows
            // still does not insert a missing record.
            let mut update = Query::update();
            update
                .table(Alias::new(table))
                .value(Alias::new("id"), Expr::col(Alias::new("id")))
                .and_where(filter);
            let result = db
                .execute({
                    crate::query::note_statement();
                    sql::statement(db, &update)?
                })
                .await
                .map_err(sql::map_write)?;
            Ok(LockResult {
                rows: result.rows_affected(),
            })
        }
        DatabaseBackend::MySql => Err(FerrioxidError::Validation("MySQL is not supported".into())),
    }
}

impl Assignment {
    fn column(&self) -> &'static str {
        match self {
            Self::Set { column, .. }
            | Self::Null { column }
            | Self::Add { column, .. }
            | Self::Sub { column, .. } => column,
        }
    }
}

fn apply_assignment<M: FerrioxidModel>(
    query: &mut sea_orm::sea_query::UpdateStatement,
    columns: &[ColumnSpec],
    assignment: &Assignment,
    backend: DatabaseBackend,
) -> Result<(), FerrioxidError> {
    let column = column_spec(columns, assignment.column())?;
    if column.primary_key {
        return Err(FerrioxidError::Validation(format!(
            "column {} is the primary key",
            column.name
        )));
    }
    match assignment {
        Assignment::Set { value, .. } => {
            let bound = bind_for_column(column, value, backend)?;
            query.value(Alias::new(column.name), bound);
        }
        Assignment::Null { .. } => {
            if !column.nullable {
                return Err(FerrioxidError::Validation(format!(
                    "column {} cannot be null",
                    column.name
                )));
            }
            query.value(Alias::new(column.name), null_value(column.kind, backend));
        }
        Assignment::Add { amount, .. } | Assignment::Sub { amount, .. } => {
            if column.kind != ColumnKind::BigInt {
                return Err(FerrioxidError::Validation(format!(
                    "column {} does not accept arithmetic",
                    column.name
                )));
            }
            if *amount <= 0 {
                return Err(FerrioxidError::Validation(
                    "arithmetic amount must be greater than zero".into(),
                ));
            }
            let adding = matches!(assignment, Assignment::Add { .. });
            let window = arithmetic_window(adding, *amount, bound_for::<M>(column.name));
            query.and_where(Expr::col(Alias::new(column.name)).is_not_null());
            if window.impossible {
                query.and_where(Expr::val(0).eq(1));
            } else {
                if let Some(lower) = window.lower {
                    query.and_where(Expr::col(Alias::new(column.name)).gte(lower));
                }
                if let Some(upper) = window.upper {
                    query.and_where(Expr::col(Alias::new(column.name)).lte(upper));
                }
            }
            let expression = if adding {
                Expr::col(Alias::new(column.name)).add(*amount)
            } else {
                Expr::col(Alias::new(column.name)).sub(*amount)
            };
            query.value(Alias::new(column.name), expression);
        }
    }
    Ok(())
}

struct Window {
    lower: Option<i64>,
    upper: Option<i64>,
    impossible: bool,
}

fn arithmetic_window(adding: bool, amount: i64, bound: Option<&NumericBound>) -> Window {
    let mut window = if adding {
        match i64::MAX.checked_sub(amount) {
            Some(upper) => Window {
                lower: None,
                upper: Some(upper),
                impossible: false,
            },
            None => Window {
                lower: None,
                upper: None,
                impossible: true,
            },
        }
    } else {
        match i64::MIN.checked_add(amount) {
            Some(lower) => Window {
                lower: Some(lower),
                upper: None,
                impossible: false,
            },
            None => Window {
                lower: None,
                upper: None,
                impossible: true,
            },
        }
    };
    let Some(bound) = bound else {
        return window;
    };
    if window.impossible {
        return window;
    }
    if let Some(max) = bound.max {
        let cap = if adding {
            max.checked_sub(amount)
        } else {
            max.checked_add(amount)
        };
        match cap {
            Some(cap) => {
                window.upper = Some(window.upper.map(|current| current.min(cap)).unwrap_or(cap))
            }
            None if adding => window.impossible = true,
            None => {}
        }
    }
    if let Some(min) = bound.min {
        let floor = if adding {
            min.checked_sub(amount)
        } else {
            min.checked_add(amount)
        };
        match floor {
            Some(floor) => {
                window.lower = Some(
                    window
                        .lower
                        .map(|current| current.max(floor))
                        .unwrap_or(floor),
                )
            }
            None if !adding => window.impossible = true,
            None => {}
        }
    }
    window
}

fn bound_for<M: FerrioxidModel>(column: &str) -> Option<&'static NumericBound> {
    M::numeric_bounds()
        .iter()
        .find(|bound| bound.column == column)
}

fn null_value(kind: ColumnKind, backend: DatabaseBackend) -> Value {
    match (backend, kind) {
        (_, ColumnKind::Uuid) => Value::Uuid(None),
        (_, ColumnKind::BigInt) => Value::BigInt(None),
        (_, ColumnKind::Boolean) => Value::Bool(None),
        (_, ColumnKind::Decimal) => Value::Bytes(None),
        (DatabaseBackend::Postgres, ColumnKind::Date) => Value::ChronoDate(None),
        (DatabaseBackend::Postgres, ColumnKind::Time) => Value::ChronoTime(None),
        (DatabaseBackend::Postgres, ColumnKind::DateTime) => Value::ChronoDateTimeUtc(None),
        _ => Value::String(None),
    }
}

fn column_spec<'a>(
    columns: &'a [ColumnSpec],
    name: &str,
) -> Result<&'a ColumnSpec, FerrioxidError> {
    columns
        .iter()
        .find(|column| column.name == name && is_safe_ident(name))
        .ok_or_else(|| FerrioxidError::Validation(format!("column {name} is not available")))
}

fn require_unique_target<M: FerrioxidModel>(target: &[&str]) -> Result<(), FerrioxidError> {
    if target.is_empty() {
        return Err(FerrioxidError::Validation(
            "conflict target needs at least one column".into(),
        ));
    }
    for name in target {
        column_spec(M::columns(), name)?;
    }
    if target.len() == 1 {
        let column = column_spec(M::columns(), target[0])?;
        if column.primary_key || column.unique {
            return Ok(());
        }
    }
    let mut wanted: Vec<&str> = target.to_vec();
    wanted.sort_unstable();
    for group in M::constraints().unique_groups {
        let mut have: Vec<&str> = (*group).to_vec();
        have.sort_unstable();
        if have == wanted {
            return Ok(());
        }
    }
    Err(FerrioxidError::Validation(
        "conflict target is not a unique constraint".into(),
    ))
}

async fn wait_for_sqlite(db: &impl ConnectionTrait) -> Result<(), FerrioxidError> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return Ok(());
    }
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA busy_timeout = 5000".to_owned(),
    ))
    .await
    .map_err(FerrioxidError::database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ferrioxid_core::{
        ColumnKind, ColumnSpec, DatabaseKind, FerrioxidModel, ModelDescriptor, NumericBound,
        QueryScope, ReferentialAction, ScopeValue, TableConstraints,
    };
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
    use serde::Deserialize;
    use uuid::Uuid;

    use super::*;
    use crate::sql::{render_table_bundle, ForeignKeyClause, PhysicalColumn};
    use crate::{col, connect, kind_of, ModelQuery};

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn column(
        name: &'static str,
        kind: ColumnKind,
        nullable: bool,
        primary_key: bool,
    ) -> ColumnSpec {
        ColumnSpec {
            name,
            kind,
            nullable,
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

    #[track_caller]
    fn validation(error: FerrioxidError) -> String {
        match error {
            FerrioxidError::Validation(text) => text,
            other => panic!("expected validation, got {other:?}"),
        }
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Owner {
        id: Uuid,
        name: String,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Product {
        id: Uuid,
        name: String,
        stock: i64,
        active: bool,
        price_cents: i64,
        owner_id: Uuid,
        adjustment: Option<i64>,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Cart {
        id: Uuid,
        owner_id: Uuid,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Line {
        id: Uuid,
        cart_id: Uuid,
        owner_id: Uuid,
        sku: String,
        moved: bool,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
    struct Address {
        id: Uuid,
        owner_id: Uuid,
        label: String,
    }

    fn descriptor(name: &'static str, table: &'static str) -> ModelDescriptor {
        ModelDescriptor {
            name,
            table,
            api_path: "",
            allow_anonymous: false,
            permission: None,
            requires_object_policy: false,
        }
    }

    impl FerrioxidModel for Owner {
        fn descriptor() -> &'static ModelDescriptor {
            static VALUE: OnceLock<ModelDescriptor> = OnceLock::new();
            VALUE.get_or_init(|| descriptor("Owner", "ferrioxid_write_owners"))
        }
        fn columns() -> &'static [ColumnSpec] {
            static VALUE: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            VALUE.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("name", ColumnKind::Text, false, false),
                ]
            })
        }
        fn schema_sql(kind: DatabaseKind) -> String {
            render_table_bundle(
                kind,
                Self::descriptor().table,
                &physical(Self::columns()),
                &[],
                &[],
                &[],
            )
            .join(";\n")
        }
    }

    impl FerrioxidModel for Product {
        fn descriptor() -> &'static ModelDescriptor {
            static VALUE: OnceLock<ModelDescriptor> = OnceLock::new();
            VALUE.get_or_init(|| descriptor("Product", "ferrioxid_write_products"))
        }
        fn columns() -> &'static [ColumnSpec] {
            static VALUE: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            VALUE.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("name", ColumnKind::Text, false, false),
                    column("stock", ColumnKind::BigInt, false, false),
                    column("active", ColumnKind::Boolean, false, false),
                    column("price_cents", ColumnKind::BigInt, false, false),
                    column("owner_id", ColumnKind::Uuid, false, false),
                    column("adjustment", ColumnKind::BigInt, true, false),
                ]
            })
        }
        fn numeric_bounds() -> &'static [NumericBound] {
            static VALUE: &[NumericBound] = &[NumericBound {
                column: "stock",
                min: None,
                max: Some(1_000_000),
            }];
            VALUE
        }
        fn schema_sql(kind: DatabaseKind) -> String {
            render_table_bundle(
                kind,
                Self::descriptor().table,
                &physical(Self::columns()),
                &[],
                &[],
                &[],
            )
            .join(";\n")
        }
    }

    impl FerrioxidModel for Cart {
        fn descriptor() -> &'static ModelDescriptor {
            static VALUE: OnceLock<ModelDescriptor> = OnceLock::new();
            VALUE.get_or_init(|| descriptor("Cart", "ferrioxid_write_carts"))
        }
        fn columns() -> &'static [ColumnSpec] {
            static VALUE: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            VALUE.get_or_init(|| {
                let mut owner = column("owner_id", ColumnKind::Uuid, false, false);
                owner.unique = true;
                vec![column("id", ColumnKind::Uuid, false, true), owner]
            })
        }
        fn constraints() -> &'static TableConstraints {
            static VALUE: TableConstraints = TableConstraints {
                unique_groups: &[&["owner_id"]],
                index_groups: &[],
            };
            &VALUE
        }
        fn schema_sql(kind: DatabaseKind) -> String {
            render_table_bundle(
                kind,
                Self::descriptor().table,
                &physical(Self::columns()),
                &[("uq_ferrioxid_write_carts_owner", &["owner_id"])],
                &[],
                &[],
            )
            .join(";\n")
        }
    }

    impl FerrioxidModel for Line {
        fn descriptor() -> &'static ModelDescriptor {
            static VALUE: OnceLock<ModelDescriptor> = OnceLock::new();
            VALUE.get_or_init(|| descriptor("Line", "ferrioxid_write_lines"))
        }
        fn columns() -> &'static [ColumnSpec] {
            static VALUE: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            VALUE.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("cart_id", ColumnKind::Uuid, false, false),
                    column("owner_id", ColumnKind::Uuid, false, false),
                    column("sku", ColumnKind::Text, false, false),
                    column("moved", ColumnKind::Boolean, false, false),
                ]
            })
        }
        fn schema_sql(kind: DatabaseKind) -> String {
            render_table_bundle(
                kind,
                Self::descriptor().table,
                &physical(Self::columns()),
                &[],
                &[ForeignKeyClause {
                    name: "fk_ferrioxid_write_lines_cart",
                    column: "cart_id",
                    referenced_table: "ferrioxid_write_carts",
                    referenced_column: "id",
                    on_delete: ReferentialAction::Restrict,
                }],
                &[],
            )
            .join(";\n")
        }
    }

    impl FerrioxidModel for Address {
        fn descriptor() -> &'static ModelDescriptor {
            static VALUE: OnceLock<ModelDescriptor> = OnceLock::new();
            VALUE.get_or_init(|| descriptor("Address", "ferrioxid_write_addresses"))
        }
        fn columns() -> &'static [ColumnSpec] {
            static VALUE: OnceLock<Vec<ColumnSpec>> = OnceLock::new();
            VALUE.get_or_init(|| {
                vec![
                    column("id", ColumnKind::Uuid, false, true),
                    column("owner_id", ColumnKind::Uuid, false, false),
                    column("label", ColumnKind::Text, false, false),
                ]
            })
        }
        fn schema_sql(kind: DatabaseKind) -> String {
            render_table_bundle(
                kind,
                Self::descriptor().table,
                &physical(Self::columns()),
                &[],
                &[ForeignKeyClause {
                    name: "fk_ferrioxid_write_addresses_owner",
                    column: "owner_id",
                    referenced_table: "ferrioxid_write_owners",
                    referenced_column: "id",
                    on_delete: ReferentialAction::Restrict,
                }],
                &[],
            )
            .join(";\n")
        }
    }

    fn physical(columns: &[ColumnSpec]) -> Vec<PhysicalColumn<'_>> {
        columns.iter().map(PhysicalColumn::from).collect()
    }

    async fn apply(db: &DatabaseConnection, sql: &str) {
        for statement in sql.split(";\n") {
            let statement = statement.trim();
            if statement.is_empty() {
                continue;
            }
            db.execute(Statement::from_string(
                db.get_database_backend(),
                statement.to_owned(),
            ))
            .await
            .unwrap();
        }
    }

    async fn install(db: &DatabaseConnection) {
        let kind = kind_of(db.get_database_backend()).unwrap();
        apply(db, &Owner::schema_sql(kind)).await;
        apply(db, &Product::schema_sql(kind)).await;
        apply(db, &Cart::schema_sql(kind)).await;
        apply(db, &Line::schema_sql(kind)).await;
        apply(db, &Address::schema_sql(kind)).await;
    }

    async fn seed(db: &DatabaseConnection) {
        for (owner, name) in [(1_u128, "Ada"), (2, "Bea")] {
            Owner::insert()
                .value("id", id(owner))
                .value("name", name)
                .execute(db)
                .await
                .unwrap();
        }
        insert_product(db, 10, 1, 1, 500, None).await;
        insert_product(db, 11, 4, 1, 500, None).await;
        insert_product(db, 12, 1, 2, 500, None).await;
        for (cart, owner) in [(20_u128, 1_u128), (21, 2)] {
            Cart::insert()
                .value("id", id(cart))
                .value("owner_id", id(owner))
                .execute(db)
                .await
                .unwrap();
        }
        for line in 1..=101 {
            Line::insert()
                .value("id", id(1000 + line))
                .value("cart_id", id(20))
                .value("owner_id", id(1))
                .value("sku", format!("sku-{line}"))
                .value("moved", false)
                .execute(db)
                .await
                .unwrap();
        }
        Line::insert()
            .value("id", id(2000))
            .value("cart_id", id(21))
            .value("owner_id", id(2))
            .value("sku", "foreign")
            .value("moved", false)
            .execute(db)
            .await
            .unwrap();
        for number in 1..=19 {
            Address::insert()
                .value("id", id(3000 + number))
                .value("owner_id", id(1))
                .value("label", format!("address-{number}"))
                .execute(db)
                .await
                .unwrap();
        }
    }

    async fn insert_product(
        db: &DatabaseConnection,
        product: u128,
        stock: i64,
        owner: u128,
        price: i64,
        adjustment: Option<i64>,
    ) {
        let mut insert = Product::insert()
            .value("id", id(product))
            .value("name", format!("product-{product}"))
            .value("stock", stock)
            .value("active", true)
            .value("price_cents", price)
            .value("owner_id", id(owner));
        if let Some(adjustment) = adjustment {
            insert = insert.value("adjustment", adjustment);
        }
        insert.execute(db).await.unwrap();
    }

    async fn one_product(db: &impl ConnectionTrait, product: u128) -> Product {
        Product::query()
            .filter(col("id").eq(id(product)))
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    async fn take_one(
        db: &impl ConnectionTrait,
        product: u128,
        price: i64,
    ) -> Result<WriteResult, FerrioxidError> {
        Product::query()
            .filter(col("id").eq(id(product)))
            .filter(col("active").eq(true))
            .filter(col("stock").gte(1_i64))
            .filter(col("price_cents").eq(price))
            .update_matching()
            .decrement("stock", 1)
            .execute(db)
            .await
    }

    async fn cases(url: &str) {
        let db = connect(url).await.unwrap();
        install(&db).await;
        seed(&db).await;

        let refused = Product::query()
            .update_matching()
            .set("stock", 0_i64)
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(refused).contains("unfiltered"));
        let refused = Line::query()
            .delete_matching()
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(refused).contains("unfiltered"));
        let injected = Product::query()
            .filter(col("id").eq(id(10)))
            .update_matching()
            .set("stock; drop table ferrioxid_write_products", 0_i64)
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(injected).contains("not available"));
        assert_eq!(one_product(&db, 10).await.stock, 1);

        let hidden = Product::query()
            .scope(QueryScope::Equals {
                column: "owner_id",
                value: ScopeValue::Uuid(id(1)),
            })
            .filter(col("id").eq(id(12)))
            .update_matching()
            .set("name", "hidden")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(hidden.rows_affected, 0);
        assert_eq!(one_product(&db, 12).await.name, "product-12");
        let none = Product::query()
            .scope(QueryScope::None)
            .update_all()
            .set("name", "nobody")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(none.rows_affected, 0);

        let negative = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .set("stock", -2_i64)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(negative.rows_affected, 1);
        let lowered = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .decrement("stock", 1)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(lowered.rows_affected, 1);
        assert_eq!(one_product(&db, 11).await.stock, -3);
        Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .set("stock", i64::MIN)
            .execute(&db)
            .await
            .unwrap();
        let underflow = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .decrement("stock", 1)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(underflow.rows_affected, 0);
        assert_eq!(one_product(&db, 11).await.stock, i64::MIN);
        Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .set("stock", 1_000_000_i64)
            .execute(&db)
            .await
            .unwrap();
        let overflow = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .increment("stock", 1)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(overflow.rows_affected, 0);
        assert_eq!(one_product(&db, 11).await.stock, 1_000_000);
        let bad_amount = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .decrement("stock", 0)
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(bad_amount).contains("greater than zero"));
        let bad_type = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .increment("name", 1)
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(bad_type).contains("arithmetic"));
        Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .set("stock", 4_i64)
            .set_null("adjustment")
            .execute(&db)
            .await
            .unwrap();
        let skipped_null = Product::query()
            .filter(col("id").eq(id(11)))
            .update_matching()
            .decrement("adjustment", 1)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(skipped_null.rows_affected, 0);
        assert_eq!(one_product(&db, 11).await.adjustment, None);

        Product::query()
            .filter(col("id").eq(id(10)))
            .update_matching()
            .set("price_cents", 600_i64)
            .set("stock", 5_i64)
            .execute(&db)
            .await
            .unwrap();
        let stale = take_one(&db, 10, 500).await.unwrap();
        assert_eq!(stale.rows_affected, 0);
        assert_eq!(one_product(&db, 10).await.stock, 5);
        let fresh = take_one(&db, 10, 600).await.unwrap();
        assert_eq!(fresh.rows_affected, 1);
        assert_eq!(one_product(&db, 10).await.stock, 4);

        let tx = db.begin().await.unwrap();
        let moved = Line::query()
            .filter(col("owner_id").eq(id(1)))
            .update_matching()
            .set("moved", true)
            .execute(&tx)
            .await
            .unwrap();
        assert_eq!(moved.rows_affected, 101);
        let conflict = Cart::insert()
            .value("id", id(22))
            .value("owner_id", id(1))
            .execute(&tx)
            .await
            .unwrap_err();
        assert!(validation(conflict).contains("unique"));
        tx.rollback().await.unwrap();
        assert_eq!(
            Line::query()
                .filter(col("owner_id").eq(id(1)))
                .filter(col("moved").eq(true))
                .count(&db)
                .await
                .unwrap(),
            0
        );

        let tx = db.begin().await.unwrap();
        Line::query()
            .filter(col("owner_id").eq(id(1)))
            .update_matching()
            .set("sku", "kept")
            .execute(&tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            Line::query()
                .filter(col("sku").eq("kept"))
                .count(&db)
                .await
                .unwrap(),
            101
        );

        Product::query()
            .filter(col("id").eq(id(10)))
            .update_matching()
            .set("stock", 1_i64)
            .execute(&db)
            .await
            .unwrap();
        let left = connect(url).await.unwrap();
        let right = connect(url).await.unwrap();
        let (first, second) = tokio::join!(take_one(&left, 10, 600), take_one(&right, 10, 600));
        let affected = first.unwrap().rows_affected + second.unwrap().rows_affected;
        assert_eq!(affected, 1);
        assert_eq!(one_product(&db, 10).await.stock, 0);
        left.close().await.unwrap();
        right.close().await.unwrap();

        let removed = Line::query()
            .scope(QueryScope::Equals {
                column: "owner_id",
                value: ScopeValue::Uuid(id(1)),
            })
            .delete_matching()
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(removed.rows_affected, 101);
        assert_eq!(Line::query().count(&db).await.unwrap(), 1);
        assert_eq!(
            Line::query()
                .filter(col("owner_id").eq(id(2)))
                .count(&db)
                .await
                .unwrap(),
            1
        );

        Owner::insert()
            .value("id", id(3))
            .value("name", "Cara")
            .execute(&db)
            .await
            .unwrap();
        let left = connect(url).await.unwrap();
        let right = connect(url).await.unwrap();
        let (first, second) = tokio::join!(
            Cart::insert()
                .value("id", Uuid::new_v4())
                .value("owner_id", id(3))
                .execute(&left),
            Cart::insert()
                .value("id", Uuid::new_v4())
                .value("owner_id", id(3))
                .execute(&right),
        );
        match (first, second) {
            (Ok(row), Err(error)) | (Err(error), Ok(row)) => {
                assert_eq!(row.rows_affected, 1);
                assert!(validation(error).contains("unique"));
            }
            other => panic!("expected one insert and one unique conflict, got {other:?}"),
        }
        assert_eq!(
            Cart::query()
                .filter(col("owner_id").eq(id(3)))
                .count(&db)
                .await
                .unwrap(),
            1
        );
        left.close().await.unwrap();
        right.close().await.unwrap();

        let ignored = Cart::insert()
            .value("id", id(23))
            .value("owner_id", id(2))
            .on_conflict_do_nothing(&["owner_id"])
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(ignored.rows_affected, 0);
        assert_eq!(Cart::query().count(&db).await.unwrap(), 3);
        let missing = Owner::query()
            .filter(col("id").eq(id(99)))
            .lock_existing(&db.begin().await.unwrap())
            .await
            .unwrap();
        assert_eq!(missing.rows, 0);

        let left = connect(url).await.unwrap();
        let right = connect(url).await.unwrap();
        let (first, second) = tokio::join!(add_address(&left), add_address(&right));
        assert_eq!(first + second, 1);
        assert_eq!(
            Address::query()
                .filter(col("owner_id").eq(id(1)))
                .count(&db)
                .await
                .unwrap(),
            20
        );
        left.close().await.unwrap();
        right.close().await.unwrap();

        let blocked = Owner::query()
            .filter(col("id").eq(id(1)))
            .delete_matching()
            .execute(&db)
            .await
            .unwrap_err();
        assert!(validation(blocked).contains("foreign key"));
        assert_eq!(Owner::query().count(&db).await.unwrap(), 3);
        db.close().await.unwrap();
    }

    async fn add_address(db: &DatabaseConnection) -> u64 {
        let tx = db.begin().await.unwrap();
        let locked = Owner::query()
            .filter(col("id").eq(id(1)))
            .lock_existing(&tx)
            .await
            .unwrap();
        assert_eq!(locked.rows, 1);
        let existing = Address::query()
            .filter(col("owner_id").eq(id(1)))
            .count(&tx)
            .await
            .unwrap();
        let inserted = if existing < 20 {
            Address::insert()
                .value("id", Uuid::new_v4())
                .value("owner_id", id(1))
                .value("label", "extra")
                .execute(&tx)
                .await
                .unwrap()
                .rows_affected
        } else {
            0
        };
        tx.commit().await.unwrap();
        inserted
    }

    #[tokio::test]
    async fn sqlite_shop_cases() {
        let path = std::env::temp_dir().join(format!(
            "ferrioxid-write-{}.sqlite",
            Uuid::new_v4().simple()
        ));
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.display().to_string().replace('\\', "/")
        );
        cases(&url).await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_shop_cases() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = crate::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        cases(disposable.url()).await;
        disposable
            .drop_database()
            .await
            .expect("drop isolated database");
    }
}
