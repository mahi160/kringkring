use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

const MIGRATIONS: &[&str] = &[
    // Version 1 establishes the schema baseline; no application tables yet.
    include_str!("../migrations/0001_baseline.sql"),
    include_str!("../migrations/0002_users_and_sessions.sql"),
    include_str!("../migrations/0003_default_bengali.sql"),
    include_str!("../migrations/0004_user_activation.sql"),
    include_str!("../migrations/0005_catalog.sql"),
    include_str!("../migrations/0006_requests.sql"),
    include_str!("../migrations/0007_request_reasons.sql"),
    include_str!("../migrations/0008_staff_duty.sql"),
    include_str!("../migrations/0009_request_history.sql"),
    include_str!("../migrations/0010_push.sql"),
];

pub fn initialize(data_dir: &Path) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    fs::create_dir_all(data_dir)?;
    let path = data_dir.join("kringkring.sqlite3");
    let mut db = Connection::open(&path)?;
    let journal: String =
        db.pragma_update_and_check(None, "journal_mode", "wal", |row| row.get(0))?;
    if journal != "wal" {
        return Err("SQLite did not enable WAL mode".into());
    }

    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let version = usize::try_from(version)?;
    if version > MIGRATIONS.len() {
        return Err(format!("database schema version {version} is newer than this binary").into());
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i32::try_from(index + 1)?)?;
    }
    tx.commit()?;
    Ok(path)
}

pub fn check(path: &Path) -> rusqlite::Result<()> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn initializes_wal_and_does_not_write_on_restart() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("new-data-dir");
        let db = super::initialize(&data_dir).unwrap();
        let observer = Connection::open(db).unwrap();
        let version: i32 = observer
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let journal: String = observer
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 10);
        assert_eq!(journal, "wal");

        let data_version = || {
            observer
                .pragma_query_value(None, "data_version", |row| row.get::<_, i32>(0))
                .unwrap()
        };
        let before_restart = data_version();
        super::initialize(&data_dir).unwrap();
        assert_eq!(data_version(), before_restart);
    }
}
