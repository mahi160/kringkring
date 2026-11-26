use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn request(app: &Router, method: Method, uri: &str, cookie: &str, form: &str) -> Response {
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

async fn post(app: &Router, uri: &str, cookie: &str, form: &str) -> StatusCode {
    request(app, Method::POST, uri, cookie, form).await.status()
}

async fn html(app: &Router, uri: &str, cookie: &str) -> String {
    let response = request(app, Method::GET, uri, cookie, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), 100_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn until(body: &mut Body, value: &str) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
            let event = String::from_utf8(data.to_vec()).unwrap();
            if event.contains(value) {
                return event;
            }
        }
    })
    .await
    .expect("no matching SSE update")
}

#[tokio::test]
async fn undo_deadline_and_bilingual_reasons_are_enforced_and_broadcast() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "employee", "0123").unwrap();
    crate::users::connect(&db).unwrap().execute_batch(
        "INSERT INTO users (username, role, pin_hash) SELECT 'staff', 'staff', pin_hash FROM users WHERE username = 'employee';
         INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (1, 'Kitchen', '', 10);",
    ).unwrap();
    let token = |name: &str| {
        crate::users::login(&db, name, "0123")
            .unwrap()
            .unwrap()
            .token
    };
    let employee = format!(
        "__Host-kringkring_session={}; kringkring_language=bn",
        token("employee")
    );
    let staff = format!(
        "__Host-kringkring_session={}; kringkring_language=en",
        token("staff")
    );
    let app = crate::web::router(db.clone());
    for (list, fields) in [
        (
            "rejectreasons",
            "name_en=Sold+out&name_bn=%E0%A6%B6%E0%A7%87%E0%A6%B7&sort_order=10&active=true",
        ),
        (
            "cancelreasons",
            "name_en=Meeting+ended&name_bn=&sort_order=10&active=true",
        ),
    ] {
        let path = format!("/admin/catalog/{list}");
        assert_eq!(
            post(&app, &path, &staff, fields).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post(&app, &path, &employee, fields).await,
            StatusCode::SEE_OTHER
        );
    }
    let mut staff_events = request(&app, Method::GET, "/staff/events", &staff, "")
        .await
        .into_body();
    let mut employee_events = request(&app, Method::GET, "/employee/events", &employee, "")
        .await
        .into_body();
    until(&mut staff_events, "No requests").await;
    until(&mut employee_events, "কোনো চলমান অনুরোধ নেই।").await;

    let send = "item_id=1&location_id=1";
    assert_eq!(
        post(&app, "/employee/menu", &employee, send).await,
        StatusCode::SEE_OTHER
    );
    let cancel = "/employee/requests/1";
    assert_eq!(
        post(&app, cancel, &employee, "action=cancel").await,
        StatusCode::SEE_OTHER
    );
    until(&mut employee_events, "ফিরিয়ে নিন").await;
    assert_eq!(
        post(&app, "/staff/requests/1", &staff, "action=accept").await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&app, cancel, &employee, "action=undo").await,
        StatusCode::SEE_OTHER
    );
    let restored = until(&mut staff_events, "value=\"accept\"").await;
    assert!(restored.contains("Tea"));
    assert_eq!(
        post(&app, cancel, &employee, "action=cancel").await,
        StatusCode::SEE_OTHER
    );
    until(&mut employee_events, "বাতিল হয়েছে").await;
    assert_eq!(
        post(&app, cancel, &employee, "action=undo").await,
        StatusCode::CONFLICT
    );

    assert_eq!(
        post(&app, "/employee/menu", &employee, send).await,
        StatusCode::SEE_OTHER
    );
    assert!(
        html(&app, "/staff/requests/2/reject", &staff)
            .await
            .contains("Sold out")
    );
    assert_eq!(
        post(&app, "/staff/requests/2", &staff, "action=reject&other=+").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            "/staff/requests/2",
            &staff,
            "action=reject&preset_id=1"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    let rejected = until(&mut employee_events, "প্রত্যাখ্যান করা হয়েছে").await;
    assert!(rejected.contains(">শেষ</p>"));
    assert_eq!(
        post(
            &app,
            "/admin/catalog/rejectreasons",
            &employee,
            "id=1&name_en=Renamed&name_bn=&sort_order=10"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    assert!(
        html(&app, "/employee/menu", &employee)
            .await
            .contains(">শেষ</p>")
    );

    assert_eq!(
        post(&app, "/employee/menu", &employee, send).await,
        StatusCode::SEE_OTHER
    );
    assert!(
        !html(&app, "/staff/requests/3/reject", &staff)
            .await
            .contains("Renamed")
    );
    assert_eq!(
        post(
            &app,
            "/staff/requests/3",
            &staff,
            "action=reject&other=Out+of+cups"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    until(&mut employee_events, "Out of cups").await;

    assert_eq!(
        post(&app, "/employee/menu", &employee, send).await,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, "/staff/requests/4", &staff, "action=accept").await,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        post(&app, "/employee/requests/4", &employee, "action=cancel").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(
        html(&app, "/employee/requests/4/cancel", &employee)
            .await
            .contains("Meeting ended")
    );
    assert_eq!(
        post(
            &app,
            "/employee/requests/4",
            &employee,
            "action=cancel&preset_id=1"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    let cancelled = until(&mut staff_events, "Meeting ended").await;
    assert!(cancelled.contains("Cancelled"));
}
