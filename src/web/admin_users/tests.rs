use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use tower::ServiceExt;

async fn call(app: &Router, method: Method, path: &str, cookie: &str, form: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
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

async fn post(app: &Router, path: &str, cookie: &str, form: &str) -> Response {
    call(app, Method::POST, path, cookie, form).await
}

async fn get(app: &Router, path: &str, cookie: &str) -> Response {
    call(app, Method::GET, path, cookie, "").await
}

async fn login(app: &Router, user: &str, pin: &str) -> Response {
    post(app, "/login", "", &format!("username={user}&pin={pin}")).await
}

fn cookie(response: Response) -> String {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
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
async fn manages_the_account_lifecycle_and_checks_admin_on_every_route() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(dir.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let app = crate::web::router(db);
    let admin = cookie(login(&app, "operator", "0123").await);
    for (user, role, flag) in [
        ("employee", "employee", ""),
        ("staff", "staff", ""),
        ("manager", "staff", "&is_admin=true"),
    ] {
        let form = format!("username={user}&role={role}&language=en&pin=0456{flag}");
        assert_eq!(
            post(&app, "/admin/users", &admin, &form).await.status(),
            StatusCode::SEE_OTHER
        );
    }

    let user = cookie(login(&app, "employee", "0456").await);
    let staff = cookie(login(&app, "staff", "0456").await);
    let manager = cookie(login(&app, "manager", "0456").await);
    // Admin-chosen PINs must be replaced before the account can be used.
    for session in [&user, &staff, &manager] {
        assert_eq!(
            get(&app, "/staff", session).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post(&app, "/setup", session, "pin=0457&confirm=0457")
                .await
                .status(),
            StatusCode::SEE_OTHER
        );
    }
    for denied in [&user, &staff] {
        assert_eq!(
            get(&app, "/admin/users", denied).await.status(),
            StatusCode::FORBIDDEN
        );
        for uri in [
            "/admin/users",
            "/admin/users/2/edit",
            "/admin/users/2/action",
            "/admin/sessions/revoke",
        ] {
            assert_eq!(
                post(&app, uri, denied, "").await.status(),
                StatusCode::FORBIDDEN
            );
        }
    }
    assert_eq!(get(&app, "/staff", &manager).await.status(), StatusCode::OK);
    let response = get(&app, "/admin/users", &manager).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(html(response).await.contains("manager"));

    let edit = "username=renamed&role=staff&language=bn";
    assert_eq!(
        post(&app, "/admin/users/2/edit", &admin, edit)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(get(&app, "/staff", &user).await.status(), StatusCode::OK);
    assert_eq!(
        login(&app, "employee", "0456").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let user_second = cookie(login(&app, "renamed", "0457").await);

    let action = "/admin/users/2/action";
    assert_eq!(
        post(&app, action, &admin, "action=revoke").await.status(),
        StatusCode::SEE_OTHER
    );
    for session in [user, user_second] {
        assert_eq!(
            get(&app, "/account", &session).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    for _ in 0..5 {
        assert_eq!(
            login(&app, "renamed", "0000").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        login(&app, "renamed", "0457").await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(&app, action, &admin, "action=unlock").await.status(),
        StatusCode::SEE_OTHER
    );
    let user = cookie(login(&app, "renamed", "0457").await);
    assert_eq!(
        post(&app, action, &admin, "action=reset&pin=0789")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        get(&app, "/account", &user).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "renamed", "0457").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let user = cookie(login(&app, "renamed", "0789").await);
    assert_eq!(
        post(&app, action, &admin, "action=deactivate")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        get(&app, "/account", &user).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "renamed", "0789").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn last_admin_cannot_be_demoted_or_deactivated() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(dir.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let app = crate::web::router(db);
    let admin = cookie(login(&app, "operator", "0123").await);
    for (uri, form) in [
        (
            "/admin/users/1/edit",
            "username=operator&role=employee&language=bn",
        ),
        ("/admin/users/1/action", "action=deactivate"),
    ] {
        let response = post(&app, uri, &admin, form).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(
            html(response)
                .await
                .contains("অন্তত একজন সক্রিয় প্রশাসক রাখতে হবে।")
        );
    }
    assert_eq!(
        get(&app, "/admin/users", &admin).await.status(),
        StatusCode::OK
    );
}
