use rusqlite::Connection;

/// The maximum Magellan schema version that llmgrep v3.1.3 is known to support.
/// Magellan v11 is supported (4D coordinates, cfg_edges table, xxHash64 hashes).
/// If the database has a newer version, warn the user.
pub const SUPPORTED_MAGELLAN_SCHEMA_VERSION: i64 = 11;

/// Check the Magellan database schema version.
///
/// Returns Ok(()) if the schema version is <= SUPPORTED_MAGELLAN_SCHEMA_VERSION.
/// Returns Err with a descriptive message if the version is newer.
///
/// If the magellan_meta table doesn't exist (very old database), returns Ok(())
/// but logs a warning — the query will likely fail later anyway.
pub fn check_schema_version(conn: &Connection) -> Result<(), String> {
    let version: Option<i64> = match conn.query_row(
        "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
        [],
        |row| row.get(0),
    ) {
        Ok(v) => Some(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if msg.contains("no such table") => {
            eprintln!(
                "Warning: Could not determine Magellan schema version (magellan_meta table missing). \
                 Queries may fail if the database was created by an incompatible Magellan version."
            );
            return Ok(());
        }
        Err(e) => return Err(format!("Failed to read schema version: {}", e)),
    };

    match version {
        Some(v) if v > SUPPORTED_MAGELLAN_SCHEMA_VERSION => Err(format!(
            "Magellan database schema version {} is newer than the maximum supported version {}. \
             Please upgrade llmgrep or re-index with a compatible Magellan version.",
            v, SUPPORTED_MAGELLAN_SCHEMA_VERSION
        )),
        Some(v) if v < 7 => Err(format!(
            "Magellan database schema version {} is too old (minimum supported: 7). \
             Please re-index with 'magellan watch'.",
            v
        )),
        Some(v) => {
            eprintln!("Info: Magellan schema version {} (supported)", v);
            Ok(())
        }
        None => {
            eprintln!(
                "Warning: magellan_meta table exists but has no row with id=1. \
                 Schema version unknown."
            );
            Ok(())
        }
    }
}

/// Check if coverage tables exist in the database.
///
/// Returns true if `cfg_block_coverage`, `cfg_edge_coverage`, and `cfg_coverage_meta`
/// all exist. Returns false if any are missing.
pub fn check_coverage_tables_exist(conn: &Connection) -> bool {
    let tables = [
        "cfg_block_coverage",
        "cfg_edge_coverage",
        "cfg_coverage_meta",
    ];
    for table in &tables {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1 LIMIT 1",
                [table],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !exists {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_db_with_version(version: i64) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, 3, 0)",
            [version],
        ).unwrap();
        conn
    }

    #[test]
    fn test_check_supported_version() {
        let conn = create_test_db_with_version(10);
        assert!(check_schema_version(&conn).is_ok());
    }

    #[test]
    fn test_check_current_version() {
        let conn = create_test_db_with_version(11);
        assert!(check_schema_version(&conn).is_ok());
    }

    #[test]
    fn test_check_future_version_fails() {
        let conn = create_test_db_with_version(99);
        let result = check_schema_version(&conn);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("99"),
            "Error should mention version 99: {}",
            err
        );
        assert!(
            err.contains("upgrade llmgrep"),
            "Error should suggest upgrading: {}",
            err
        );
    }

    #[test]
    fn test_check_old_version_fails() {
        let conn = create_test_db_with_version(5);
        let result = check_schema_version(&conn);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("5"), "Error should mention version 5: {}", err);
    }

    #[test]
    fn test_check_missing_meta_table_warns() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(check_schema_version(&conn).is_ok());
    }
}
