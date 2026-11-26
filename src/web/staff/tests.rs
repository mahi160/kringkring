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

async fn post(app: &Router, uri: &str, cookie: &str, form: &str) -> Response {
    call(app, Method::POST, uri, cookie, form).await
}

async fn sign_in(app: &Router, name: &str) -> String {
    let response = post(app, "/login", "", &format!("username={name}&pin=0123")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    format!("{cookie}; kringkring_language=en")
}

async fn stream(app: &Router, uri: &str, cookie: &str) -> Body {
    let response = call(app, Method::GET, uri, cookie, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    response.into_body()
}

async fn next(body: &mut Body) -> String {
    let data = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("SSE event did not arrive")
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    String::from_utf8(data.to_vec()).unwrap()
}

#[tokio::test]
async fn live_queue_first_accept_wins_and_reconnect_reads_current_state() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "employee", "0123").unwrap();
    let conn = crate::users::connect(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO users (username, role, pin_hash) SELECT 'Rina', 'staff', pin_hash FROM users WHERE username = 'employee';
         INSERT INTO users (username, role, pin_hash) SELECT 'Hasan', 'staff', pin_hash FROM users WHERE username = 'employee';
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', '', 10);",
    ).unwrap();
    let app = crate::web::router(db.clone());
    let employee = sign_in(&app, "employee").await;
    let rina = sign_in(&app, "Rina").await;
    let hasan = sign_in(&app, "Hasan").await;
    let mut rina_stream = stream(&app, "/staff/events", &rina).await;
    let mut hasan_stream = stream(&app, "/staff/events", &hasan).await;
    let mut employee_stream = stream(&app, "/employee/events", &employee).await;
    for staff in [&mut rina_stream, &mut hasan_stream] {
        assert!(next(staff).await.contains("No requests"));
    }
    assert!(
        next(&mut employee_stream)
            .await
            .contains("No open requests")
    );

    assert_eq!(
        post(
            &app,
            "/employee/menu",
            &employee,
            "item_id=1&location_id=1&modifiers=1"
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    for staff in [&mut rina_stream, &mut hasan_stream] {
        let html = next(staff).await;
        assert!(
            html.contains("Kitchen")
                && html.contains("Tea")
                && html.contains("Sugar")
                && html.contains("Requested by employee")
        );
        assert!(html.contains("data-created=") && html.contains("value=\"accept\""));
    }
    assert!(next(&mut employee_stream).await.contains("Pending"));

    let action = "/staff/requests/1";
    let (a, b) = tokio::join!(
        post(&app, action, &rina, "action=accept"),
        post(&app, action, &hasan, "action=accept")
    );
    let (winner, winner_name, loser, winner_stream, loser_stream, lost_response) =
        match (a.status(), b.status()) {
            (StatusCode::SEE_OTHER, StatusCode::CONFLICT) => (
                &rina,
                "Rina",
                &hasan,
                &mut rina_stream,
                &mut hasan_stream,
                b,
            ),
            (StatusCode::CONFLICT, StatusCode::SEE_OTHER) => (
                &hasan,
                "Hasan",
                &rina,
                &mut hasan_stream,
                &mut rina_stream,
                a,
            ),
            other => panic!("expected one successful claim and one conflict, received {other:?}"),
        };
    let lost_html = String::from_utf8(
        to_bytes(lost_response.into_body(), 100_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(lost_html.contains(&format!("Accepted by {winner_name}")));
    assert!(next(winner_stream).await.contains("value=\"done\">"));
    let loser_html = next(loser_stream).await;
    assert!(
        loser_html.contains("queue-card--dim") && loser_html.contains("value=\"done\" disabled")
    );
    assert!(next(&mut employee_stream).await.contains("Accepted"));
    assert_eq!(
        post(&app, action, loser, "action=done").await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&app, action, &employee, "action=done").await.status(),
        StatusCode::FORBIDDEN
    );

    let restarted = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    let mut connected = stream(&restarted, "/employee/events", &employee).await;
    assert!(next(&mut connected).await.contains("Accepted"));
    assert_eq!(
        post(&restarted, action, winner, "action=done")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert!(next(&mut connected).await.contains("Delivered"));

    assert_eq!(
        post(
            &restarted,
            "/employee/menu",
            &employee,
            "item_id=2&location_id=1"
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    assert!(next(&mut connected).await.contains("Raw tea"));
    assert_eq!(
        post(
            &restarted,
            "/staff/requests/2",
            winner,
            "action=reject&other=Unavailable"
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    assert!(next(&mut connected).await.contains("Rejected"));
}

#[tokio::test]
async fn orders_require_an_unpaused_staff_account_with_an_active_session() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "employee", "0123").unwrap();
    let conn = crate::users::connect(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO users (username, role, pin_hash) SELECT 'Rina', 'staff', pin_hash FROM users WHERE username = 'employee';
         INSERT INTO users (username, role, pin_hash) SELECT 'Hasan', 'staff', pin_hash FROM users WHERE username = 'employee';
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', '', 10);",
    ).unwrap();
    let app = crate::web::router(db.clone());
    let employee = sign_in(&app, "employee").await;
    let rina = sign_in(&app, "Rina").await;
    let hasan = sign_in(&app, "Hasan").await;
    let duty = "/staff/duty";
    let menu = "/employee/menu";
    let item = "item_id=1&location_id=1";

    assert_eq!(
        post(&app, duty, &employee, "paused=true").await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&app, duty, &rina, "paused=true").await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, duty, &hasan, "paused=true").await.status(),
        StatusCode::SEE_OTHER
    );
    let response = post(&app, menu, &employee, item).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let page = to_bytes(response.into_body(), 100_000).await.unwrap();
    assert!(
        std::str::from_utf8(&page)
            .unwrap()
            .contains("No staff available")
    );
    let count = || {
        conn.query_row("SELECT count(*) FROM requests", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    };
    assert_eq!(count(), 0);

    let app = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    let mut rina_events = stream(&app, "/staff/events", &rina).await;
    assert!(next(&mut rina_events).await.contains("Resume"));
    assert_eq!(
        post(&app, duty, &hasan, "paused=false").await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, menu, &employee, item).await.status(),
        StatusCode::SEE_OTHER
    );
    let denied = post(&app, "/staff/requests/1", &rina, "action=accept").await;
    assert_eq!(denied.status(), StatusCode::CONFLICT);
    let paused = to_bytes(denied.into_body(), 100_000).await.unwrap();
    assert!(
        !std::str::from_utf8(&paused)
            .unwrap()
            .contains("value=\"accept\"")
    );

    assert_eq!(
        post(&app, "/staff/requests/1", &hasan, "action=accept")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, duty, &hasan, "paused=true").await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, "/staff/requests/1", &hasan, "action=done")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, duty, &hasan, "paused=false").await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, "/logout", &hasan, "").await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, menu, &employee, item).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(count(), 1);

    assert_eq!(
        post(&app, duty, &rina, "paused=false").await.status(),
        StatusCode::SEE_OTHER
    );
    assert!(next(&mut rina_events).await.contains("On duty"));
    assert_eq!(
        post(&app, menu, &employee, item).await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(count(), 2);
}
