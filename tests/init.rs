use std::process::Command;

#[test]
fn init_stores_an_argon2id_hash_and_refuses_a_second_admin() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    let init = |username| {
        Command::new(env!("CARGO_BIN_EXE_kringkring"))
            .arg("init")
            .arg("--data-dir")
            .arg(&data_dir)
            .args(["--username", username, "--pin", "0123"])
            .output()
            .unwrap()
    };
    let first = init("operator");
    assert!(first.status.success());
    let second = init("second");
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("an admin already exists"));
    for output in [first, second] {
        assert!(!String::from_utf8_lossy(&output.stdout).contains("0123"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("0123"));
    }

    let db = rusqlite::Connection::open(data_dir.join("kringkring.sqlite3")).unwrap();
    let (hash, role, admin, language): (String, String, bool, String) = db.query_row(
        "SELECT pin_hash, role, is_admin, default_language FROM users WHERE username = 'operator'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert_eq!(role, "employee");
    assert!(admin);
    assert_eq!(language, "bn");
}
