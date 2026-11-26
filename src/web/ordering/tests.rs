use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use tower::ServiceExt;

const MENU: &str = "/employee/menu";

async fn call(app: &Router, method: Method, uri: &str, cookie: &str, form: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::HOST, "kringkring.local")
                .header(header::ORIGIN, "https://kringkring.local")
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(form.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn session(app: &Router, user: &str, pin: &str) -> String {
    let response = call(
        app,
        Method::POST,
        "/login",
        "",
        &format!("username={user}&pin={pin}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    format!("{cookie}; kringkring_language=en")
}

async fn send(app: &Router, cookie: &str, form: &str) -> Response {
    call(app, Method::POST, MENU, cookie, form).await
}

async fn html(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 100_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn two_tap_ordering_saves_default_and_changed_choices_and_enforces_open_limit() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let connection = crate::users::connect(&db).unwrap();
    connection
        .execute_batch(
            "INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES
            (1, 'Kitchen', '', 10), (2, 'Boardroom', '', 20);
         UPDATE users SET default_location_id = 1 WHERE username = 'operator';
         INSERT INTO users (username, role, pin_hash)
         SELECT 'staff', 'staff', pin_hash FROM users WHERE username = 'operator';",
        )
        .unwrap();
    let app = crate::web::router(db);
    let cookie = session(&app, "operator", "0123").await;
    session(&app, "staff", "0123").await;

    let selected = call(&app, Method::GET, "/employee/menu?item_id=1", &cookie, "").await;
    assert_eq!(selected.status(), StatusCode::OK);
    assert!(
        html(selected)
            .await
            .contains("name=\"location_id\" value=\"1\" required checked")
    );
    let response = send(&app, &cookie, "item_id=1&location_id=1").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/employee/menu#requests"
    );

    assert_eq!(
        send(&app, &cookie, "item_id=4&location_id=2&modifiers=1")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&app, &cookie, "custom_text=%20&location_id=1")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let too_long = format!("custom_text={}&location_id=1", "x".repeat(81));
    assert_eq!(
        send(&app, &cookie, &too_long).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let choices = send(
        &app,
        &cookie,
        "item_id=1&location_id=2&modifiers=1&modifiers=2",
    );
    let custom = send(&app, &cookie, "custom_text=Lemon+water&location_id=1");
    let duplicate_slot = send(&app, &cookie, "item_id=4&location_id=2");
    let (choices, custom, duplicate_slot) = tokio::join!(choices, custom, duplicate_slot);
    let statuses = [choices.status(), custom.status(), duplicate_slot.status()];
    assert_eq!(
        statuses
            .iter()
            .filter(|&&status| status == StatusCode::SEE_OTHER)
            .count(),
        2
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|&&status| status == StatusCode::CONFLICT)
            .count(),
        1
    );
    let rows: Vec<(i64, Option<String>, i64)> = connection
        .prepare("SELECT id, custom_text, location_id FROM requests ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].2, 1);
    let response = send(&app, &cookie, "item_id=1&location_id=1").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        html(response)
            .await
            .contains("You already have three open requests.")
    );

    connection
        .execute("UPDATE requests SET status = 'accepted' WHERE id = 1", [])
        .unwrap();
    assert_eq!(
        send(&app, &cookie, "item_id=1&location_id=1")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    connection
        .execute("UPDATE requests SET status = 'done' WHERE id = 1", [])
        .unwrap();
    assert_eq!(
        send(
            &app,
            &cookie,
            "item_id=1&location_id=2&modifiers=1&modifiers=2"
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );

    connection
        .execute("UPDATE requests SET status = 'done' WHERE id = 2", [])
        .unwrap();
    assert_eq!(
        send(&app, &cookie, "custom_text=Lemon+water&location_id=1")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let page = html(call(&app, Method::GET, MENU, &cookie, "").await).await;
    for text in [
        "Lemon water",
        "Boardroom",
        "Kitchen",
        "Sugar",
        "Milk",
        "Pending",
    ] {
        assert!(page.contains(text));
    }
    connection
        .execute("UPDATE locations SET active = 0 WHERE id = 2", [])
        .unwrap();
    let page = html(call(&app, Method::GET, MENU, &cookie, "").await).await;
    assert!(
        page.contains("Boardroom"),
        "existing requests retain deactivated locations"
    );
}
