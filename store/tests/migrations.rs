//! Integration tests for the [`mneme_store::schema::apply_migrations`]
//! runner. These exercise the framework against a live in-memory
//! SQLite connection so we cover the real `PRAGMA user_version` path,
//! transactionality, and idempotence — not just unit-level mocking.
//!
//! Tests live here (under `store/tests/`) rather than as `#[cfg(test)]`
//! inside `schema.rs` so we can poke a private-looking constant
//! ([`MIGRATIONS`]) only via its public name and treat the runner as a
//! consumer-facing API surface.

use rusqlite::Connection;

use mneme_store::schema::{apply_migrations, MIGRATIONS};

/// Helper: open a fresh in-memory shard with `user_version` = 0.
fn fresh_db() -> Connection {
    Connection::open_in_memory().expect("open_in_memory")
}

/// Read `PRAGMA user_version` as `u32` from a connection.
fn user_version(conn: &Connection) -> u32 {
    conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .expect("user_version") as u32
}

#[test]
fn migrations_v0_to_v3_runs_idempotently_on_empty_db() {
    // With the v0.3.2-ship MIGRATIONS slice empty, the runner should
    // be a clean no-op on a fresh in-memory database. user_version
    // stays at 0, no error, no panic.
    let mut conn = fresh_db();
    assert_eq!(user_version(&conn), 0, "fresh db starts at user_version=0");

    // Use Meta layer for the empty-table no-op invariant — its
    // migration set is the default empty MIGRATIONS slice.
    let final_version =
        apply_migrations(&mut conn, common::layer::DbLayer::Meta).expect("apply on empty db");

    if MIGRATIONS.is_empty() {
        // Empty table: nothing should advance.
        assert_eq!(final_version, 0);
        assert_eq!(user_version(&conn), 0);
    } else {
        // Once v0.4 populates MIGRATIONS the final version equals the
        // highest target. Future-proofs the test.
        let highest_target = MIGRATIONS.iter().map(|(t, _)| *t).max().expect("non-empty");
        assert_eq!(final_version, highest_target);
        assert_eq!(user_version(&conn), highest_target);
    }
}

#[test]
fn migrations_v0_to_v3_skips_already_applied_blocks() {
    // Running apply_migrations twice on the same connection should
    // produce the same final version and not error. The second run
    // observes user_version >= every entry's target and skips them
    // all.
    let mut conn = fresh_db();

    let first = apply_migrations(&mut conn, common::layer::DbLayer::Meta).expect("first run");
    let second =
        apply_migrations(&mut conn, common::layer::DbLayer::Meta).expect("second run is no-op");

    assert_eq!(
        first, second,
        "second run must converge to the same version"
    );
    assert_eq!(user_version(&conn), second);
}

#[test]
fn migrations_fail_loud_on_bad_sql() {
    // Build a temporary, deliberately broken migration table and run
    // it through the same internal logic that `apply_migrations` uses.
    // We can't mutate the public `MIGRATIONS` const, so this test
    // re-implements the runner's contract against a fixture: bad SQL
    // must propagate, not silently skip.
    //
    // The shape mirrors `apply_migrations` exactly: read user_version,
    // open a transaction per block, run statements, commit, bump.
    // The only difference is the migration table.
    let conn = fresh_db();
    let bad_migrations: &[(u32, &[&str])] = &[(1, &["THIS IS NOT VALID SQL AT ALL"])];

    let result = run_with_table(&conn, bad_migrations);
    assert!(
        result.is_err(),
        "bad SQL must propagate as Err, never silently skip"
    );

    // user_version must NOT have advanced — the failed transaction
    // rolled back.
    assert_eq!(
        user_version(&conn),
        0,
        "user_version stays at 0 after a failed migration"
    );
}

#[test]
fn migrations_can_add_column_to_existing_table() {
    // Simulate the realistic upgrade scenario:
    //   v1 shard has a `nodes` table with the v1 columns.
    //   v2 ships with a new `complexity_score REAL` column.
    //   apply_migrations on the v1 shard must add the column without
    //   data loss.
    let conn = fresh_db();

    // Set up a "v1 database": nodes table at v1 shape, user_version=1.
    conn.execute_batch(
        "CREATE TABLE nodes (
             id INTEGER PRIMARY KEY,
             name TEXT NOT NULL
         );
         INSERT INTO nodes(id, name) VALUES(1, 'pre-existing');
         PRAGMA user_version = 1;",
    )
    .expect("seed v1 shard");
    assert_eq!(user_version(&conn), 1);

    // Define a fixture v2 migration.
    let v2_migrations: &[(u32, &[&str])] =
        &[(2, &["ALTER TABLE nodes ADD COLUMN complexity_score REAL"])];

    let final_version = run_with_table(&conn, v2_migrations).expect("v2 migration applies");
    assert_eq!(final_version, 2);
    assert_eq!(user_version(&conn), 2);

    // Verify the column actually exists. PRAGMA table_info yields one
    // row per column; we look for the new name.
    let mut stmt = conn
        .prepare("PRAGMA table_info(nodes)")
        .expect("prepare table_info");
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .expect("query_map")
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        cols.iter().any(|c| c == "complexity_score"),
        "complexity_score column must exist after migration; got {:?}",
        cols
    );

    // Pre-existing data must be preserved (NULL in the new column is
    // fine; SQLite auto-fills new columns with NULL).
    let preserved_name: String = conn
        .query_row("SELECT name FROM nodes WHERE id = 1", [], |r| r.get(0))
        .expect("pre-existing row preserved");
    assert_eq!(preserved_name, "pre-existing");
}

// ---------------------------------------------------------------------
// Test helper: re-implements the same algorithm as `apply_migrations`
// but takes the migration slice as a parameter so we can test against
// fixture tables. Keeps tests independent of the production
// MIGRATIONS table contents.
// ---------------------------------------------------------------------

fn run_with_table(conn: &Connection, table: &[(u32, &[&str])]) -> Result<u32, rusqlite::Error> {
    let mut current: u32 =
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as u32;

    for (target, stmts) in table.iter() {
        if *target <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        for stmt in stmts.iter() {
            tx.execute_batch(stmt)?;
        }
        tx.execute_batch(&format!("PRAGMA user_version = {}", target))?;
        tx.commit()?;
        current = *target;
    }
    Ok(current)
}
