use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use kringkring::i18n::Language;
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair};

use super::{Keys, Outcome, Transport, Vapid, dispatch, subscribe, take};
use crate::{
    orders::{self, Draft, Reason, StaffAction},
    users::{TokenHash, connect},
};

const EMPLOYEE: TokenHash = [1; 32];
const RINA: TokenHash = [2; 32];
const HASAN: TokenHash = [3; 32];
const SIGNED_OUT: TokenHash = [4; 32];

fn keys() -> Keys {
    let public = ES256KeyPair::generate()
        .public_key()
        .public_key()
        .to_bytes_uncompressed();
    Keys {
        p256dh: URL_SAFE_NO_PAD.encode(public),
        auth: URL_SAFE_NO_PAD.encode([7; 16]),
    }
}

fn endpoint(name: &str) -> String {
    format!("https://fcm.googleapis.com/fcm/send/{name}")
}

/// Employee (BN) orders; Rina (BN user, EN device, on duty), Hasan (BN, paused) and a
/// signed-out staff device are subscribed.
fn setup() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "employee", "0123").unwrap();
    let conn = connect(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, role, pin_hash, default_language) VALUES
             (2, 'Rina', 'staff', '', 'bn'), (3, 'Hasan', 'staff', '', 'bn'), (4, 'Gone', 'staff', '', 'en');
         UPDATE users SET staff_paused = 1 WHERE id = 3;
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', 'রান্নাঘর', 10);",
    )
    .unwrap();
    for (token, user) in [(EMPLOYEE, 1), (RINA, 2), (HASAN, 3), (SIGNED_OUT, 4)] {
        conn.execute(
            "INSERT INTO sessions (token_hash, user_id) VALUES (?1, ?2)",
            rusqlite::params![token, user],
        )
        .unwrap();
    }
    for (token, name, lang) in [
        (EMPLOYEE, "employee", Language::Bn),
        (RINA, "rina", Language::En),
        (HASAN, "hasan", Language::Bn),
        (SIGNED_OUT, "gone", Language::En),
    ] {
        assert!(subscribe(&db, token, &endpoint(name), &keys(), lang).is_ok());
    }
    conn.execute("DELETE FROM sessions WHERE token_hash = ?1", [SIGNED_OUT])
        .unwrap();
    (temp, db)
}

fn order(db: &Path) {
    let draft = Draft {
        item_id: Some(1),
        custom_text: None,
        location_id: Some(1),
        modifiers: Vec::new(),
    };
    assert!(orders::send(db, EMPLOYEE, &draft).is_ok());
}

fn no_reason() -> Reason {
    Reason {
        preset_id: None,
        other: None,
    }
}

#[test]
fn new_request_reaches_only_on_duty_staff_in_their_device_language() {
    let (_temp, db) = setup();
    order(&db);

    let deliveries = take(&db).unwrap();
    assert_eq!(
        deliveries.len(),
        1,
        "paused and signed-out staff get nothing"
    );
    let push = &deliveries[0];
    assert_eq!(push.endpoint, endpoint("rina"));
    assert_eq!(push.payload.title, "New request");
    assert_eq!(push.payload.body, "Tea · Kitchen · employee");
    assert_eq!(push.payload.url, "/staff#request-1");
    let actions = push.payload.actions.as_ref().unwrap();
    assert_eq!((actions.request, actions.accept), (1, "Accept"));
    assert!(take(&db).unwrap().is_empty(), "the outbox is drained");
}

#[test]
fn accepted_request_notifies_the_employee_in_bengali() {
    let (_temp, db) = setup();
    order(&db);
    take(&db).unwrap();
    assert!(orders::staff_action(&db, RINA, 1, StaffAction::Accept, no_reason()).is_ok());

    let deliveries = take(&db).unwrap();
    assert_eq!(deliveries.len(), 1);
    let push = &deliveries[0];
    assert_eq!(push.endpoint, endpoint("employee"));
    assert_eq!(push.payload.title, "গ্রহণ করা হয়েছে");
    assert_eq!(push.payload.body, "চা · Rina");
    assert_eq!(push.payload.url, "/employee/menu#request-1");
    assert!(push.payload.actions.is_none());
}

#[test]
fn cancel_after_accept_notifies_on_duty_staff() {
    let (_temp, db) = setup();
    order(&db);
    assert!(orders::staff_action(&db, RINA, 1, StaffAction::Accept, no_reason()).is_ok());
    take(&db).unwrap();
    let reason = Reason {
        preset_id: None,
        other: Some("Meeting".into()),
    };
    assert!(
        orders::employee_action(&db, EMPLOYEE, 1, orders::EmployeeAction::Cancel, reason).is_ok()
    );

    let deliveries = take(&db).unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].payload.title, "Cancelled");
}

#[test]
fn pending_cancel_notifies_staff_once_the_undo_window_closes() {
    let (_temp, db) = setup();
    order(&db);
    take(&db).unwrap();
    assert!(
        orders::employee_action(
            &db,
            EMPLOYEE,
            1,
            orders::EmployeeAction::Cancel,
            no_reason()
        )
        .is_ok()
    );
    assert!(take(&db).unwrap().is_empty(), "undo is still possible");

    connect(&db)
        .unwrap()
        .execute("UPDATE requests SET cancel_after_ms = 0", [])
        .unwrap();
    assert_eq!(orders::finish_cancellations(&db).unwrap(), 1);
    let deliveries = take(&db).unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].payload.title, "Cancelled");
}

#[test]
fn subscriptions_only_accept_browser_push_services() {
    let (_temp, db) = setup();
    for bad in [
        "http://fcm.googleapis.com/x",
        "https://evil.example/x",
        "https://fcm.googleapis.com:8443/x",
        "https://user@fcm.googleapis.com/x",
        "https://fcm.googleapis.com.evil.example/x",
    ] {
        assert!(
            subscribe(&db, RINA, bad, &keys(), Language::En).is_err(),
            "{bad}"
        );
    }
    let short_auth = Keys {
        auth: URL_SAFE_NO_PAD.encode([7; 8]),
        ..keys()
    };
    assert!(subscribe(&db, RINA, &endpoint("x"), &short_auth, Language::En).is_err());
    assert!(
        subscribe(
            &db,
            RINA,
            "https://web.push.apple.com/abc",
            &keys(),
            Language::En
        )
        .is_ok()
    );
}

struct Fake {
    gone: String,
    sent: Mutex<Vec<String>>,
}

impl Transport for Fake {
    async fn send(&self, request: http::Request<Vec<u8>>) -> Outcome {
        let endpoint = request.uri().to_string();
        assert_eq!(request.headers()["content-encoding"], "aes128gcm");
        assert!(
            request.headers()["authorization"]
                .to_str()
                .unwrap()
                .starts_with("vapid t=")
        );
        self.sent.lock().unwrap().push(endpoint.clone());
        if endpoint == self.gone {
            Outcome::Gone
        } else {
            Outcome::Sent
        }
    }
}

#[tokio::test]
async fn gone_subscriptions_are_deleted_after_sending() {
    let (temp, db) = setup();
    order(&db);
    assert!(orders::staff_action(&db, RINA, 1, StaffAction::Accept, no_reason()).is_ok());
    let vapid =
        Arc::new(Vapid::load_or_create(temp.path(), "mailto:test@example.com".into()).unwrap());
    let fake = Arc::new(Fake {
        gone: endpoint("employee"),
        sent: Mutex::default(),
    });

    assert_eq!(dispatch(&db, &vapid, &fake).await, 2);
    let mut sent = fake.sent.lock().unwrap().clone();
    sent.sort();
    assert_eq!(sent, [endpoint("employee"), endpoint("rina")]);

    let remaining: Vec<String> = connect(&db)
        .unwrap()
        .prepare("SELECT endpoint FROM push_subscriptions ORDER BY endpoint")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(remaining, [endpoint("hasan"), endpoint("rina")]);
}

#[test]
fn vapid_key_is_created_private_and_reused() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let first = Vapid::load_or_create(temp.path(), String::new()).unwrap();
    let mode = std::fs::metadata(temp.path().join("vapid.pem"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let second = Vapid::load_or_create(temp.path(), String::new()).unwrap();
    assert_eq!(first.public_key, second.public_key);
    assert_eq!(URL_SAFE_NO_PAD.decode(&first.public_key).unwrap().len(), 65);
}
