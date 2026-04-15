use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::SqlitePool;
use tax_db_macros::Entity;

// ── encode / decode helpers ───────────────────────────────────────────

fn decimal_as_f64(d: &Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

fn decimal_from_sql(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> Result<Decimal, sqlx::Error> {
    let val: f64 = sqlx::Row::try_get(row, col)?;
    Decimal::try_from(val).map_err(|e| sqlx::Error::ColumnDecode {
        index: col.to_string(),
        source: Box::new(e),
    })
}

// ── test structs ──────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Entity)]
#[entity(table = "widgets")]
#[allow(dead_code)]
struct Widget {
    #[entity(pk)]
    id: i64,
    name: String,
    #[entity(skip)]
    computed: String,
}

#[derive(Debug, PartialEq, Entity)]
#[entity(table = "accounts")]
struct Account {
    #[entity(pk)]
    id: i64,
    #[entity(
        encode_with = "decimal_as_f64",
        decode_with = "decimal_from_sql"
    )]
    balance: Decimal,
}

/// Composite PK, no custom codec.
#[derive(Debug, PartialEq, Entity)]
#[entity(table = "line_items")]
struct LineItem {
    #[entity(pk)]
    order_id: i64,
    #[entity(pk)]
    item_id: i64,
    description: String,
}

/// No pk — only insert + list are generated.
#[derive(Debug, PartialEq, Entity)]
struct EventLog {
    ts: i64,
    message: String,
}

// ── SQL constant assertions ───────────────────────────────────────────

#[test]
fn widget_sql() {
    assert_eq!(Widget::TABLE, "widgets");
    assert_eq!(
        Widget::INSERT_SQL,
        "INSERT INTO widgets (id, name) VALUES (?, ?)"
    );
    assert_eq!(Widget::SELECT_ALL_SQL, "SELECT id, name FROM widgets");
    assert_eq!(
        Widget::GET_SQL,
        "SELECT id, name FROM widgets WHERE id = ?"
    );
    assert_eq!(Widget::DELETE_SQL, "DELETE FROM widgets WHERE id = ?");
}

#[test]
fn composite_pk_sql() {
    assert_eq!(
        LineItem::GET_SQL,
        "SELECT order_id, item_id, description FROM line_items WHERE order_id = ? AND item_id = ?"
    );
    assert_eq!(
        LineItem::DELETE_SQL,
        "DELETE FROM line_items WHERE order_id = ? AND item_id = ?"
    );
}

#[test]
fn default_table_name() {
    assert_eq!(EventLog::TABLE, "event_logs");
    assert_eq!(
        EventLog::SELECT_ALL_SQL,
        "SELECT ts, message FROM event_logs"
    );
}

// ── round-trip tests ──────────────────────────────────────────────────

#[tokio::test]
async fn insert_find_delete_round_trip() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let w = Widget {
        id: 1,
        name: "gear".into(),
        computed: "ignored".into(),
    };
    w.insert(&pool).await.unwrap();

    let found = Widget::find(&pool, &1).await.unwrap();
    assert_eq!(
        found,
        Some(Widget {
            id: 1,
            name: "gear".into(),
            computed: String::new(), // Default for skipped field
        })
    );

    w.delete(&pool).await.unwrap();

    let gone = Widget::find(&pool, &1).await.unwrap();
    assert_eq!(gone, None);
}

#[tokio::test]
async fn find_returns_none_for_missing_row() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(Widget::find(&pool, &999).await.unwrap(), None);
}

#[tokio::test]
async fn list_returns_all_rows() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let a = Widget { id: 1, name: "a".into(), computed: String::new() };
    let b = Widget { id: 2, name: "b".into(), computed: String::new() };
    a.insert(&pool).await.unwrap();
    b.insert(&pool).await.unwrap();

    let mut all = Widget::list(&pool).await.unwrap();
    all.sort_by_key(|w| w.id);
    assert_eq!(all, vec![a, b]);
}

#[tokio::test]
async fn list_empty_table() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE event_logs (ts INTEGER NOT NULL, message TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let rows = EventLog::list(&pool).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn delete_by_pk_static() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let w = Widget { id: 1, name: "x".into(), computed: String::new() };
    w.insert(&pool).await.unwrap();

    let result = Widget::delete_by_pk(&pool, &1).await.unwrap();
    assert_eq!(result.rows_affected(), 1);

    let result = Widget::delete_by_pk(&pool, &1).await.unwrap();
    assert_eq!(result.rows_affected(), 0);
}

#[tokio::test]
async fn composite_pk_round_trip() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE line_items (
             order_id INTEGER NOT NULL,
             item_id  INTEGER NOT NULL,
             description TEXT NOT NULL,
             PRIMARY KEY (order_id, item_id)
         )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let li = LineItem {
        order_id: 10,
        item_id: 3,
        description: "bolt".into(),
    };
    li.insert(&pool).await.unwrap();

    let found = LineItem::find(&pool, &10, &3).await.unwrap();
    assert_eq!(found, Some(li));

    LineItem::delete_by_pk(&pool, &10, &3).await.unwrap();
    assert_eq!(LineItem::find(&pool, &10, &3).await.unwrap(), None);
}

#[tokio::test]
async fn encode_decode_decimal_round_trip() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE accounts (
             id      INTEGER PRIMARY KEY,
             balance REAL NOT NULL
         )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let a = Account {
        id: 1,
        balance: Decimal::new(12345, 2), // 123.45
    };
    a.insert(&pool).await.unwrap();

    let found = Account::find(&pool, &1).await.unwrap().unwrap();
    assert_eq!(found.balance, Decimal::new(12345, 2));

    let all = Account::list(&pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].balance, Decimal::new(12345, 2));
}

#[tokio::test]
async fn insert_inside_transaction() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE accounts (id INTEGER PRIMARY KEY, balance REAL NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let a = Account {
        id: 7,
        balance: Decimal::new(999, 1),
    };
    a.insert(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let found = Account::find(&pool, &7).await.unwrap().unwrap();
    assert_eq!(found.balance, Decimal::new(999, 1));
}
