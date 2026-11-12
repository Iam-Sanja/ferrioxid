//! Beziehungsauswahl und Mengenänderungen auf der bestehenden Abfrage-API.
//!
//! Fremdschlüssel und eindeutige Eins-zu-eins-Spalten nutzen dieselbe seitenweise
//! Suche. Many-to-Many schreibt nur die Verbindungstabelle und nur in einer
//! Transaktion. Ein Objekt-Scope bleibt ein Parameter des Aufrufers.

use ferrioxid_core::{ColumnKind, ColumnSpec, FerrioxidError, QueryScope};
use ferrioxid_db::{query_admin_page, AdminPageRequest, ManyToMany};
use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query, SqliteQueryBuilder};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait, Value,
};
use serde_json::Value as Json;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationShape {
    ForeignKey,
    OneToOne,
}

pub fn shape_of(column: &ColumnSpec) -> Option<RelationShape> {
    column.foreign_table?;
    Some(if column.unique {
        RelationShape::OneToOne
    } else {
        RelationShape::ForeignKey
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupPage {
    pub rows: Vec<Json>,
    pub total: i64,
    pub page: u32,
    pub page_size: u16,
    pub has_more: bool,
}

/// Sucht Anzeigewerte in `label`. `page_size` bleibt bei höchstens 100.
///
/// `scope` kommt aus der Objektregel. [`QueryScope::None`] liefert keine Zeile.
#[allow(clippy::too_many_arguments)]
pub async fn lookup_page(
    db: &impl ConnectionTrait,
    table: &str,
    columns: &[ColumnSpec],
    label: &str,
    search: Option<&str>,
    page: u32,
    page_size: u16,
    scope: &QueryScope,
) -> Result<LookupPage, FerrioxidError> {
    if !safe_ident(table) || !safe_ident(label) {
        return Err(FerrioxidError::Validation(
            "relation lookup is not available".into(),
        ));
    }
    let column = columns
        .iter()
        .find(|column| column.name == label)
        .ok_or_else(|| FerrioxidError::Validation("relation label is not a column".into()))?;
    if column.kind != ColumnKind::Text || column.primary_key {
        return Err(FerrioxidError::Validation(
            "relation label must be a text column".into(),
        ));
    }
    let page = page.clamp(1, 10_000);
    let page_size = page_size.clamp(1, 100);
    let term = search.map(str::trim).filter(|term| !term.is_empty());
    if term.is_some_and(|term| term.chars().count() > 200) {
        return Err(FerrioxidError::Validation(
            "relation search is too long".into(),
        ));
    }
    let search_columns: [&str; 1] = [label];
    let listed = query_admin_page(
        db,
        table,
        columns,
        AdminPageRequest {
            search_columns: if term.is_some() { &search_columns } else { &[] },
            search: term,
            sort_column: label,
            descending: false,
            equal_filters: &[],
            bounds: &[],
            offset: u64::from(page - 1) * u64::from(page_size),
            limit: u64::from(page_size),
            scope,
        },
    )
    .await?;
    Ok(LookupPage {
        rows: listed.rows,
        total: listed.total,
        page,
        page_size,
        has_more: listed.has_more,
    })
}

/// Lehnt eine Sammelauswahl ab, sobald eine Id außerhalb der sichtbaren Menge liegt.
pub fn bulk_targets(visible: &[Uuid], requested: &[Uuid]) -> Result<Vec<Uuid>, &'static str> {
    if requested.len() > 1000 {
        return Err("admin.bulk.size");
    }
    let mut chosen = Vec::with_capacity(requested.len());
    for id in requested {
        if chosen.contains(id) || !visible.contains(id) {
            return Err("admin.bulk.scope");
        }
        chosen.push(*id);
    }
    Ok(chosen)
}

/// Ersetzt die Kindzeilen eines Elternteils. Die vorherigen Verbindungen bleiben,
/// wenn ein Schreibvorgang fehlschlägt.
pub async fn replace_links(
    db: &DatabaseConnection,
    link: ManyToMany,
    parent: Uuid,
    children: &[Uuid],
) -> Result<(), FerrioxidError> {
    if !safe_ident(link.join_table)
        || !safe_ident(link.parent_column)
        || !safe_ident(link.child_column)
        || children.len() > 1000
    {
        return Err(FerrioxidError::Validation(
            "many-to-many link is not available".into(),
        ));
    }
    let mut unique = Vec::new();
    for child in children {
        if unique.contains(child) {
            return Err(FerrioxidError::Validation(
                "many-to-many child is duplicated".into(),
            ));
        }
        unique.push(*child);
    }
    let tx = db.begin().await.map_err(FerrioxidError::database)?;
    let mut delete = Query::delete();
    delete.from_table(Alias::new(link.join_table)).and_where(
        Expr::col(Alias::new(link.parent_column)).eq(Value::Uuid(Some(Box::new(parent)))),
    );
    tx.execute(statement(&tx, &delete)?)
        .await
        .map_err(FerrioxidError::database)?;
    for child in unique {
        let mut insert = Query::insert();
        insert
            .into_table(Alias::new(link.join_table))
            .columns([
                Alias::new(link.parent_column),
                Alias::new(link.child_column),
            ])
            .values_panic([
                Value::Uuid(Some(Box::new(parent))).into(),
                Value::Uuid(Some(Box::new(child))).into(),
            ]);
        tx.execute(statement(&tx, &insert)?)
            .await
            .map_err(FerrioxidError::database)?;
    }
    tx.commit().await.map_err(FerrioxidError::database)?;
    Ok(())
}

fn statement(
    db: &impl ConnectionTrait,
    query: &impl sea_orm::sea_query::QueryStatementWriter,
) -> Result<Statement, FerrioxidError> {
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
        DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
        DatabaseBackend::MySql => {
            return Err(FerrioxidError::Validation("MySQL is not supported".into()))
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values.0))
}

fn safe_ident(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 64
}

#[cfg(test)]
mod tests {
    use super::{bulk_targets, lookup_page, replace_links, shape_of, RelationShape};
    use ferrioxid_core::{ColumnKind, ColumnSpec, QueryScope, ReferentialAction, ScopeValue};
    use ferrioxid_db::{insert_record, ManyToMany};
    use sea_orm::{
        ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait, Value,
    };
    use uuid::Uuid;

    fn text_column(name: &'static str) -> ColumnSpec {
        ColumnSpec {
            name,
            kind: ColumnKind::Text,
            nullable: false,
            primary_key: false,
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
            ColumnSpec {
                name: "id",
                kind: ColumnKind::Uuid,
                nullable: false,
                primary_key: true,
                default_bool: None,
                unique: false,
                indexed: false,
                foreign_table: None,
                foreign_column: None,
                on_delete: None,
                precision: 0,
                scale: 0,
                choices: &[],
            },
            text_column("label"),
            text_column("owner"),
        ]
    }

    async fn create_lookup(db: &DatabaseConnection) {
        let sql = match db.get_database_backend() {
            DatabaseBackend::Postgres => {
                "CREATE TABLE lookup_targets (id uuid PRIMARY KEY, label text NOT NULL, owner text NOT NULL)"
            }
            DatabaseBackend::Sqlite => {
                "CREATE TABLE lookup_targets (id text PRIMARY KEY, label text NOT NULL, owner text NOT NULL)"
            }
            DatabaseBackend::MySql => panic!("mysql is not a test backend"),
        };
        db.execute(Statement::from_string(
            db.get_database_backend(),
            String::from(sql),
        ))
        .await
        .unwrap();
    }

    async fn insert_owned(db: &DatabaseConnection, index: u32, owner: &str) {
        insert_record(
            db,
            "lookup_targets",
            &columns(),
            vec![
                Value::Uuid(Some(Box::new(Uuid::from_u128(u128::from(index) + 1)))),
                Value::String(Some(Box::new(format!("row-{index:04}")))),
                Value::String(Some(Box::new(owner.to_owned()))),
            ],
        )
        .await
        .unwrap();
    }

    async fn exercise_lookup(db: &DatabaseConnection) {
        create_lookup(db).await;
        let cols = columns();
        let all = QueryScope::All;
        let empty = lookup_page(db, "lookup_targets", &cols, "label", None, 1, 100, &all)
            .await
            .unwrap();
        assert_eq!(empty.total, 0);
        assert!(empty.rows.is_empty());
        insert_owned(db, 0, "alpha").await;
        let one = lookup_page(db, "lookup_targets", &cols, "label", None, 1, 100, &all)
            .await
            .unwrap();
        assert_eq!(one.total, 1);
        assert!(!one.has_more);
        for index in 1..100 {
            insert_owned(db, index, "alpha").await;
        }
        let hundred = lookup_page(db, "lookup_targets", &cols, "label", None, 1, 100, &all)
            .await
            .unwrap();
        assert_eq!(hundred.total, 100);
        assert_eq!(hundred.rows.len(), 100);
        assert!(!hundred.has_more);
        for index in 100..1000 {
            insert_owned(db, index, "alpha").await;
        }
        let last = lookup_page(db, "lookup_targets", &cols, "label", None, 10, 100, &all)
            .await
            .unwrap();
        assert_eq!(last.total, 1000);
        assert_eq!(last.rows.len(), 100);
        assert!(!last.has_more);
        let beyond = lookup_page(db, "lookup_targets", &cols, "label", None, 11, 100, &all)
            .await
            .unwrap();
        assert!(beyond.rows.is_empty());
        let found = lookup_page(
            db,
            "lookup_targets",
            &cols,
            "label",
            Some("row-0999"),
            1,
            25,
            &all,
        )
        .await
        .unwrap();
        assert_eq!(found.total, 1);
        let hidden = lookup_page(
            db,
            "lookup_targets",
            &cols,
            "label",
            None,
            1,
            100,
            &QueryScope::Equals {
                column: "owner",
                value: ScopeValue::Text("beta".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(hidden.total, 0);
        let none = lookup_page(
            db,
            "lookup_targets",
            &cols,
            "label",
            None,
            1,
            100,
            &QueryScope::None,
        )
        .await
        .unwrap();
        assert_eq!(none.total, 0);
        assert!(none.rows.is_empty());
    }

    #[tokio::test]
    async fn sqlite_lookup_covers_zero_one_hundred_and_thousand_rows() {
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        exercise_lookup(&db).await;
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_lookup_covers_zero_one_hundred_and_thousand_rows() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let disposable = ferrioxid_db::DisposablePostgres::create(&admin)
            .await
            .expect("isolated postgres");
        let db = ferrioxid_db::connect(disposable.url())
            .await
            .expect("postgres");
        exercise_lookup(&db).await;
        db.close().await.expect("close");
        disposable.drop_database().await.expect("drop");
    }

    #[test]
    fn shapes_and_bulk_selection_stay_inside_the_visible_set() {
        let foreign = ColumnSpec {
            name: "category_id",
            kind: ColumnKind::Uuid,
            nullable: true,
            primary_key: false,
            default_bool: None,
            unique: false,
            indexed: true,
            foreign_table: Some("categories"),
            foreign_column: Some("id"),
            on_delete: Some(ReferentialAction::Restrict),
            precision: 0,
            scale: 0,
            choices: &[],
        };
        let mut unique = foreign;
        unique.unique = true;
        assert_eq!(shape_of(&foreign), Some(RelationShape::ForeignKey));
        assert_eq!(shape_of(&unique), Some(RelationShape::OneToOne));
        assert_eq!(shape_of(&text_column("label")), None);
        let visible = [Uuid::from_u128(1), Uuid::from_u128(2)];
        assert!(bulk_targets(&visible, &[]).unwrap().is_empty());
        assert_eq!(bulk_targets(&visible, &visible).unwrap().len(), 2);
        assert_eq!(
            bulk_targets(&visible, &[Uuid::from_u128(3)]).unwrap_err(),
            "admin.bulk.scope"
        );
        assert_eq!(
            bulk_targets(&visible, &[visible[0], visible[0]]).unwrap_err(),
            "admin.bulk.scope"
        );
    }

    #[tokio::test]
    async fn many_to_many_replacement_rolls_back_and_one_to_one_stays_unique() {
        let db = ferrioxid_db::connect("sqlite::memory:").await.unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            String::from("CREATE TABLE note_tags (note_id text NOT NULL, tag_id text NOT NULL, UNIQUE(note_id, tag_id))"),
        ))
        .await
        .unwrap();
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            String::from(
                "CREATE TABLE profiles (id text PRIMARY KEY, owner_id text NOT NULL UNIQUE)",
            ),
        ))
        .await
        .unwrap();
        let parent = Uuid::from_u128(7);
        let children = [Uuid::from_u128(1), Uuid::from_u128(2)];
        replace_links(
            &db,
            ManyToMany {
                join_table: "note_tags",
                parent_column: "note_id",
                child_column: "tag_id",
            },
            parent,
            &children,
        )
        .await
        .unwrap();
        replace_links(
            &db,
            ManyToMany {
                join_table: "note_tags",
                parent_column: "note_id",
                child_column: "tag_id",
            },
            parent,
            &[children[0]],
        )
        .await
        .unwrap();
        let count = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                String::from("SELECT COUNT(*) AS total FROM note_tags"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(count.try_get::<i64>("", "total").unwrap(), 1);
        let tx = db.begin().await.unwrap();
        tx.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "INSERT INTO note_tags (note_id, tag_id) VALUES ('{}', '{}')",
                parent,
                Uuid::from_u128(9)
            ),
        ))
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        let after = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                String::from("SELECT COUNT(*) AS total FROM note_tags"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.try_get::<i64>("", "total").unwrap(), 1);
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            String::from("INSERT INTO profiles (id, owner_id) VALUES ('p1', 'owner')"),
        ))
        .await
        .unwrap();
        let duplicate = db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                String::from("INSERT INTO profiles (id, owner_id) VALUES ('p2', 'owner')"),
            ))
            .await;
        assert!(duplicate.is_err());
    }
}
