use rusqlite::params;

use super::{RETENTION, prune};

#[test]
fn daily_prune_respects_cutoff_and_removes_dependent_modifiers_only_for_old_requests() {
    let temp = tempfile::tempdir().unwrap();
    let path = crate::db::initialize(temp.path()).unwrap();
    let mut db = crate::users::connect(&path).unwrap();
    db.execute_batch(
        "INSERT INTO users (id, username, pin_hash, role) VALUES (1, 'Employee', '', 'employee');
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', '', 10);",
    )
    .unwrap();
    let now = 2_000_000_000;
    for (id, created) in [
        (1, now - RETENTION - 1),
        (2, now - RETENTION),
        (3, now - RETENTION + 1),
    ] {
        db.execute("INSERT INTO requests (id, employee_id, item_id, location_id, created_at) VALUES (?1,1,1,1,?2)",params![id,created]).unwrap();
        db.execute(
            "INSERT INTO request_modifiers (request_id, modifier_id) VALUES (?1,1)",
            [id],
        )
        .unwrap();
    }
    assert_eq!(prune(&mut db, now).unwrap(), 1);
    for query in [
        "SELECT id FROM requests ORDER BY id",
        "SELECT request_id FROM request_modifiers ORDER BY request_id",
    ] {
        let ids = db
            .prepare(query)
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(ids, [2, 3]);
    }
    assert_eq!(prune(&mut db, now).unwrap(), 0);

    assert_eq!(prune(&mut db, now + RETENTION).unwrap(), 2);
    crate::users::init_admin(&path, "operator", "0123").unwrap();
    db.execute(
        "INSERT INTO users (username, role, pin_hash) SELECT 'staff', 'staff', pin_hash FROM users WHERE username = 'operator'",
        [],
    ).unwrap();
    crate::users::login(&path, "staff", "0123")
        .unwrap()
        .unwrap();
    let session = crate::users::login(&path, "operator", "0123")
        .unwrap()
        .unwrap();
    crate::orders::send(
        &path,
        crate::users::token_hash(&session.token).unwrap(),
        &crate::orders::Draft {
            item_id: Some(1),
            custom_text: None,
            location_id: Some(1),
            modifiers: vec![],
        },
    )
    .ok()
    .unwrap();
    let new_id: i64 = db
        .query_row("SELECT id FROM requests", [], |row| row.get(0))
        .unwrap();
    assert_eq!(new_id, 4, "an old action URL must not match a new request");
}
