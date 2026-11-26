use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

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

async fn text(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 150_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn until(body: &mut Body, terms: &[&str]) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
            let frame = std::str::from_utf8(&frame).unwrap();
            if terms.iter().all(|term| frame.contains(term)) {
                break;
            }
        }
    })
    .await
    .expect("no matching status event");
}

#[tokio::test]
async fn admin_can_filter_history_and_see_live_staff_presence() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let conn = crate::users::connect(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO users (username, role, pin_hash) SELECT 'Rina', 'staff', pin_hash FROM users WHERE id = 1;
         INSERT INTO users (username, role, pin_hash) SELECT 'Nila', 'employee', pin_hash FROM users WHERE id = 1;
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', '', 10);
         INSERT INTO requests (id, employee_id, item_id, location_id, status, created_at, closed_at, closed_by, reason_en)
            VALUES (1, 1, 1, 1, 'rejected', unixepoch()-86400, unixepoch()-86400, 2, 'Sold out'),
                   (2, 3, 2, 1, 'pending', unixepoch(), NULL, NULL, ''),
                   (3, 1, 3, 1, 'pending', unixepoch()-2678400, NULL, NULL, '');
         INSERT INTO request_modifiers (request_id, modifier_id) VALUES (1,1);",
    ).unwrap();
    let date: String = conn
        .query_row("SELECT date('now', '-1 day')", [], |row| row.get(0))
        .unwrap();
    let cookie = |name: &str| {
        format!(
            "__Host-kringkring_session={}; kringkring_language=en",
            crate::users::login(&db, name, "0123")
                .unwrap()
                .unwrap()
                .token
        )
    };
    let admin = cookie("operator");
    let staff = cookie("Rina");
    let app = crate::web::router(db.clone());
    for path in ["/admin/history", "/admin/status", "/admin/status/events"] {
        assert_eq!(
            call(&app, Method::GET, path, &staff, "").await.status(),
            StatusCode::FORBIDDEN
        );
    }
    let history = text(call(&app, Method::GET, "/admin/history", &admin, "").await).await;
    assert!(history.find(">Raw tea</strong>").unwrap() < history.find(">Tea</strong>").unwrap());
    assert!(!history.contains("Black coffee"));
    let filter = format!("/admin/history?status=rejected&user=1&from={date}&to={date}");
    let page = text(call(&app, Method::GET, &filter, &admin, "").await).await;
    for field in ["Sold out", "Sugar", "Closed by", "Rina", "Kitchen"] {
        assert!(page.contains(field));
    }
    assert!(!page.contains("Raw tea"));
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/admin/history?from=2026-02-30",
            &admin,
            ""
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let mut status = call(&app, Method::GET, "/admin/status/events", &admin, "")
        .await
        .into_body();
    until(&mut status, &["On duty", "Offline"]).await;
    let mut tab = call(&app, Method::GET, "/staff/events", &staff, "")
        .await
        .into_body();
    until(&mut tab, &["On duty"]).await;
    until(&mut status, &[">Online<"]).await;
    let mut tab_two = call(&app, Method::GET, "/staff/events", &staff, "")
        .await
        .into_body();
    until(&mut tab_two, &["On duty"]).await;
    drop(tab);
    assert_eq!(
        call(&app, Method::POST, "/staff/duty", &staff, "paused=true")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    until(&mut status, &[">Paused<", ">Online<"]).await;
    drop(tab_two);
    until(&mut status, &[">Paused<", ">Offline<"]).await;
}
