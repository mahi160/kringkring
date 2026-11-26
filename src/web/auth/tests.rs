use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use rusqlite::Connection;
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    cookie: &str,
    form: &str,
    origin: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, "kringkring.local")
                .header(header::ORIGIN, origin)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(form.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post(app: &Router, path: &str, cookie: &str, form: &str) -> Response {
    request(
        app,
        Method::POST,
        path,
        cookie,
        form,
        "https://kringkring.local",
    )
    .await
}

async fn get(app: &Router, path: &str, cookie: &str) -> StatusCode {
    request(app, Method::GET, path, cookie, "", "")
        .await
        .status()
}

async fn login(app: &Router, username: &str, pin: &str) -> Response {
    post(app, "/login", "", &format!("username={username}&pin={pin}")).await
}

fn session(response: Response) -> String {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(cookie.split("; ").any(|value| value == attr));
    }
    cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn successful_login_resets_consecutive_failures_but_fifth_failure_locks() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let app = crate::web::router(db.clone());
    for _ in 0..4 {
        assert_eq!(
            login(&app, "operator", "1111").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let cookie = session(login(&app, "operator", "0123").await);
    assert_eq!(get(&app, "/account", &cookie).await, StatusCode::OK);
    for _ in 0..5 {
        assert_eq!(
            login(&app, "operator", "1111").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let restarted = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    assert_eq!(
        login(&restarted, "operator", "0123").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn role_and_admin_guards_and_session_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    Connection::open(&db)
        .unwrap()
        .execute_batch(
            "INSERT INTO users (username, pin_hash, role)
         SELECT 'employee', pin_hash, 'employee' FROM users WHERE username = 'operator';
         INSERT INTO users (username, pin_hash, role)
         SELECT 'staff', pin_hash, 'staff' FROM users WHERE username = 'operator';",
        )
        .unwrap();
    let app = crate::web::router(db);
    let admin = session(login(&app, "operator", "0123").await);
    let employee = session(login(&app, "employee", "0123").await);
    let staff = session(login(&app, "staff", "0123").await);
    drop(app);

    let app = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    assert_eq!(get(&app, "/account", "").await, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&app, "/account", &admin).await, StatusCode::OK);
    assert_eq!(get(&app, "/admin", &admin).await, StatusCode::NO_CONTENT);
    assert_eq!(
        get(&app, "/employee", &employee).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get(&app, "/staff", &employee).await, StatusCode::FORBIDDEN);
    assert_eq!(get(&app, "/admin", &employee).await, StatusCode::FORBIDDEN);
    assert_eq!(get(&app, "/staff", &staff).await, StatusCode::OK);
    assert_eq!(get(&app, "/employee", &staff).await, StatusCode::FORBIDDEN);
    assert_eq!(get(&app, "/admin", &staff).await, StatusCode::FORBIDDEN);

    let path = "/admin/sessions/revoke";
    assert_eq!(
        post(&app, path, &staff, "username=employee").await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Method::POST,
            path,
            &admin,
            "username=employee",
            "https://wrong.example"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(get(&app, "/account", &employee).await, StatusCode::OK);
    assert_eq!(
        post(&app, path, &admin, "username=employee").await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get(&app, "/account", &employee).await,
        StatusCode::UNAUTHORIZED
    );

    let response = post(&app, "/logout", &staff, "").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        get(&app, "/account", &staff).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(get(&app, "/account", &admin).await, StatusCode::OK);
}

#[tokio::test]
async fn wheel_lists_active_names_and_remembers_the_last_successful_login() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "Mina", "0123").unwrap();
    Connection::open(&db).unwrap().execute_batch(
        "INSERT INTO users (username, role, pin_hash) SELECT 'Arif', 'employee', pin_hash FROM users WHERE id = 1;
         INSERT INTO users (username, role, pin_hash, active) SELECT 'Inactive user', 'staff', pin_hash, 0 FROM users WHERE id = 1;",
    ).unwrap();
    let app = crate::web::router(db);
    async fn html(response: Response) -> String {
        String::from_utf8(
            axum::body::to_bytes(response.into_body(), 50_000)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }
    let menu = html(request(&app, Method::GET, "/login", "", "", "").await).await;
    assert!(menu.find(">Arif</button>").unwrap() < menu.find(">Mina</button>").unwrap());
    assert!(!menu.contains("Inactive user"));
    let pin = html(request(&app, Method::GET, "/login?user=1", "", "", "").await).await;
    assert!(pin.contains("name=\"username\" value=\"Mina\"") && pin.contains("name=\"pin\""));
    assert!(
        !html(request(&app, Method::GET, "/login?user=3", "", "", "").await)
            .await
            .contains("name=\"pin\"")
    );

    let failed = login(&app, "Mina", "0000").await;
    assert_eq!(failed.status(), StatusCode::UNAUTHORIZED);
    assert!(!failed.headers().contains_key(header::SET_COOKIE));
    let retry = html(failed).await;
    assert!(retry.contains("role=\"alert\"") && retry.contains("value=\"Mina\""));
    let response = login(&app, "Mina", "0123").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let remembered = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|h| h.to_str().unwrap())
        .find(|h| h.starts_with("__Host-kringkring_last_user="))
        .unwrap();
    let cookie = remembered.split(';').next().unwrap();
    let wheel = html(request(&app, Method::GET, "/login", cookie, "", "").await).await;
    assert!(wheel.contains("value=\"1\" data-last-user>Mina</button>"));
}
