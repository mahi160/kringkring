use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use tower::ServiceExt;

async fn request(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

async fn text(app: &Router, uri: &str, cookie: &str) -> String {
    let response = request(
        app,
        Request::get(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 50_000).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn switch(app: &Router, origin: &str, form: &'static str) -> Response {
    request(
        app,
        Request::post("/language")
            .header(header::HOST, "kringkring.local")
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form))
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn switch_sets_browser_locale_and_keeps_the_brand_bengali() {
    let temp = tempfile::tempdir().unwrap();
    let app = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    let response = switch(&app, "https://kringkring.local", "lang=en&page=health").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/health");
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    for (path, bn, en) in [
        ("/login", "প্রবেশ করুন", "Sign in"),
        ("/health", "ডেটাবেস: সচল", "Database: available"),
    ] {
        for (locale, expected, cookie) in [("bn", bn, ""), ("en", en, cookie)] {
            let html = text(&app, path, cookie).await;
            assert!(html.contains(&format!("<html lang=\"{locale}\">")));
            assert!(html.contains(expected));
            assert!(html.contains("<span lang=\"bn\">ক্রিং ক্রিং</span>"));
        }
    }
    assert_eq!(
        switch(&app, "https://elsewhere.example", "lang=en&page=health")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        switch(
            &app,
            "https://kringkring.local",
            "lang=en&page=%2F%2Felsewhere.example"
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn browser_override_beats_the_signed_in_users_language() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE users SET default_language = 'en' WHERE username = 'operator'",
            [],
        )
        .unwrap();
    let token = crate::users::login(&db, "operator", "0123")
        .unwrap()
        .unwrap()
        .token;
    let app = crate::web::router(db);
    let session = format!("__Host-kringkring_session={token}");

    for path in ["/account", "/health"] {
        assert!(
            text(&app, path, &session)
                .await
                .contains("<html lang=\"en\">")
        );
        assert!(
            text(
                &app,
                path,
                &format!("{session}; kringkring_language=invalid")
            )
            .await
            .contains("<html lang=\"en\">")
        );
        assert!(
            text(&app, path, &format!("{session}; kringkring_language=bn"))
                .await
                .contains("<html lang=\"bn\">")
        );
    }
}
