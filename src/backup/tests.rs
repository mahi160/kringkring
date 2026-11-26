use rusqlite::{Connection, OpenFlags};

use super::{DAY, snapshot_due};

const FIRST: i64 = 1_735_696_800; // 2025-01-01 02:00 UTC

#[test]
fn catch_up_preserves_committed_wal_data_and_does_not_overwrite_the_same_night() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    let live = Connection::open(&db).unwrap();
    live.execute(
        "INSERT INTO locations (name_en, name_bn, sort_order) VALUES ('Before', '', 10)",
        [],
    )
    .unwrap();
    let backups = temp.path().join("backups");

    snapshot_due(&db, &backups, FIRST - 1).unwrap();
    assert!(backups.join("kringkring-2024-12-31.sqlite3").is_file());
    snapshot_due(&db, &backups, FIRST + 8 * 3600).unwrap();
    let path = backups.join("kringkring-2025-01-01.sqlite3");
    live.execute("UPDATE locations SET name_en = 'After'", [])
        .unwrap();
    snapshot_due(&db, &backups, FIRST + DAY - 1).unwrap();

    let snapshot = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let result: String = snapshot
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(result, "ok");
    let value: String = snapshot
        .query_row("SELECT name_en FROM locations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "Before");
    assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 2);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(backups).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn rotation_retains_seven_newest_complete_snapshots_and_leaves_other_files() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    let backups = temp.path().join("backups");
    for day in 0..9 {
        snapshot_due(&db, &backups, FIRST + day * DAY).unwrap();
        if day == 0 {
            std::fs::write(backups.join("manual.sqlite3"), b"unrelated").unwrap();
            std::fs::write(backups.join("kringkring-2025-02-31.sqlite3"), b"unrelated").unwrap();
        }
    }
    let mut names: Vec<_> = std::fs::read_dir(&backups)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    let mut expected: Vec<_> = (3..=9)
        .map(|day| format!("kringkring-2025-01-{day:02}.sqlite3"))
        .collect();
    expected.extend([
        "kringkring-2025-02-31.sqlite3".to_owned(),
        "manual.sqlite3".to_owned(),
    ]);
    assert_eq!(names, expected);
}
