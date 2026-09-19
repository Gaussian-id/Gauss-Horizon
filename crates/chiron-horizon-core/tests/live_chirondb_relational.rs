//! Internal interoperability smoke test for ChironDB's PostgreSQL-wire
//! relational surface through Chiron Horizon's native PostgreSQL adapter.
//!
//! Run against a disposable local ChironDB instance:
//! `CHIRON_HORIZON_TEST_CHIRONDB_RELATIONAL_URL=postgresql://chiron@127.0.0.1:7403/gaussdb?sslmode=disable \
//!   cargo test -p chiron-horizon-core --no-default-features --features sqlite-bundled \
//!   --test live_chirondb_relational -- --ignored --nocapture`

use std::time::Duration;

use chiron_horizon_core::db::postgres;

#[tokio::test]
#[ignore = "requires CHIRON_HORIZON_TEST_CHIRONDB_RELATIONAL_URL pointing at a disposable ChironDB instance"]
async fn horizon_connects_queries_and_reads_chirondb_relational_metadata() {
    let url = chiron_horizon_core::legacy::var("CHIRON_HORIZON_TEST_CHIRONDB_RELATIONAL_URL")
        .expect("CHIRON_HORIZON_TEST_CHIRONDB_RELATIONAL_URL");
    let pool = postgres::connect(&url, Duration::from_secs(10)).await.expect("connect ChironDB PostgreSQL wire");

    let suffix = std::process::id();
    let parent = format!("horizon_chirondb_parent_{suffix}");
    let child = format!("horizon_chirondb_child_{suffix}");
    // Keep identifiers unquoted for the current ChironDB preview router. The
    // generated names contain only lowercase ASCII, underscores, and digits.
    let parent_ident = parent.clone();
    let child_ident = child.clone();

    let _ = postgres::execute_query(&pool, &format!("DROP TABLE IF EXISTS {child_ident} CASCADE")).await;
    let _ = postgres::execute_query(&pool, &format!("DROP TABLE IF EXISTS {parent_ident} CASCADE")).await;

    let setup = [
        format!("CREATE TABLE {parent_ident} (id BIGINT PRIMARY KEY, name TEXT NOT NULL)"),
        format!("CREATE TABLE {child_ident} (id BIGINT PRIMARY KEY, parent_id BIGINT NOT NULL, note TEXT)"),
        format!("INSERT INTO {parent_ident} (id, name) VALUES (1, 'Horizon')"),
        format!("INSERT INTO {child_ident} (id, parent_id, note) VALUES (10, 1, 'ChironDB relational')"),
    ];
    for statement in setup {
        postgres::execute_query(&pool, &statement)
            .await
            .unwrap_or_else(|error| panic!("execute ChironDB relational fixture statement `{statement}`: {error}"));
    }

    let result = postgres::execute_query_text_protocol(
        &pool,
        &format!("SELECT c.id, p.name, c.note FROM {child_ident} c INNER JOIN {parent_ident} p ON p.id = c.parent_id"),
        Some(100),
    )
    .await
    .expect("query ChironDB relational rows through Horizon");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][1], serde_json::json!("Horizon"));
    assert_eq!(result.rows[0][2], serde_json::json!("ChironDB relational"));

    let tables =
        postgres::list_chirondb_relational_tables(&pool, "public").await.expect("list ChironDB tables through Horizon");
    assert!(tables.iter().any(|table| table.name == parent));
    assert!(tables.iter().any(|table| table.name == child));

    let columns = postgres::get_chirondb_relational_columns(&pool, "public", &child)
        .await
        .expect("read ChironDB columns through Horizon");
    assert!(columns.iter().any(|column| column.name == "parent_id" && column.is_primary_key == false));
    assert!(columns.iter().any(|column| column.name == "id"));

    postgres::execute_query(&pool, &format!("DROP TABLE {child_ident} CASCADE"))
        .await
        .expect("drop ChironDB child fixture");
    postgres::execute_query(&pool, &format!("DROP TABLE {parent_ident} CASCADE"))
        .await
        .expect("drop ChironDB parent fixture");
}
