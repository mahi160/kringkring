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
async fn signed_out_browser_pages_go_to_login() {
    let temp = tempfile::tempdir().unwrap();
    let app = crate::web::router(crate::db::initialize(temp.path()).unwrap());
    let open = |path: &str, accept: &str, htmx: bool| {
        let mut request = Request::builder()
            .uri(path)
            .header(header::HOST, "kringkring.local")
            .header(header::ACCEPT, accept);
        if htmx {
            request = request.header("hx-request", "true");
        }
        app.clone().oneshot(request.body(Body::empty()).unwrap())
    };
    let location = |response: &Response| response.headers()[header::LOCATION].to_owned();
    let html = "text/html,application/xhtml+xml";

    let root = open("/", html, false).await.unwrap();
    assert_eq!(location(&root), "/login");
    let account = open("/account", html, false).await.unwrap();
    assert_eq!(account.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&account), "/login");
    for (accept, htmx) in [(html, true), ("text/event-stream", false)] {
        let response = open("/staff/events", accept, htmx).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
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
    for (cookie, home) in [(&employee, "/employee/menu"), (&staff, "/staff")] {
        let response = request(&app, Method::GET, "/", cookie, "", "").await;
        assert_eq!(response.headers()[header::LOCATION], home);
    }

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

async fn html(response: Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(response.into_body(), 100_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn page(app: &Router, path: &str, cookie: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header(header::HOST, "kringkring.local")
                .header(header::ACCEPT, "text/html")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn login_page_asks_for_username_and_pin_without_listing_names() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "Mina", "0123").unwrap();
    let app = crate::web::router(db);

    let form = html(page(&app, "/login", "").await).await;
    assert!(form.contains("name=\"username\"") && form.contains("name=\"pin\""));
    assert!(form.contains("href=\"/signup\"") && !form.contains("Mina"));

    let failed = login(&app, "Mina", "0000").await;
    assert_eq!(failed.status(), StatusCode::UNAUTHORIZED);
    assert!(!failed.headers().contains_key(header::SET_COOKIE));
    let retry = html(failed).await;
    assert!(retry.contains("role=\"alert\"") && retry.contains("value=\"Mina\""));
    let response = login(&app, "Mina", "0123").await;
    assert_eq!(response.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn admin_chosen_pins_must_be_replaced_before_using_the_app() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    Connection::open(&db)
        .unwrap()
        .execute(
            "INSERT INTO locations (id, name_en, name_bn, sort_order) VALUES (7, 'Boardroom', '', 10)",
            [],
        )
        .unwrap();
    let app = crate::web::router(db);
    let admin = session(login(&app, "operator", "0123").await);
    let form = "username=arif&role=employee&language=en&pin=0456";
    assert_eq!(
        post(&app, "/admin/users", &admin, form).await.status(),
        StatusCode::SEE_OTHER
    );

    let first = session(login(&app, "arif", "0456").await);
    let second = session(login(&app, "arif", "0456").await);
    assert_eq!(
        page(&app, "/", &first).await.headers()[header::LOCATION],
        "/setup"
    );
    assert_eq!(get(&app, "/employee", &first).await, StatusCode::FORBIDDEN);
    let setup = html(page(&app, "/setup", &first).await).await;
    assert!(setup.contains("Boardroom"));

    for (form, status) in [
        (
            "pin=0789&confirm=0788&default_location=7",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "pin=0456&confirm=0456&default_location=7",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "pin=0789&confirm=0789&default_location=99",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        assert_eq!(post(&app, "/setup", &first, form).await.status(), status);
    }
    let done = post(
        &app,
        "/setup",
        &first,
        "pin=0789&confirm=0789&default_location=7",
    )
    .await;
    assert_eq!(done.headers()[header::LOCATION], "/");
    assert_eq!(get(&app, "/employee", &first).await, StatusCode::NO_CONTENT);
    assert_eq!(
        get(&app, "/employee", &second).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        page(&app, "/setup", &first).await.headers()[header::LOCATION],
        "/"
    );
    let account = html(page(&app, "/account", &first).await).await;
    assert!(account.contains("value=\"7\" selected"));
    assert_eq!(
        post(&app, "/account/location", &first, "default_location=0")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    let account = html(page(&app, "/account", &first).await).await;
    assert!(!account.contains("value=\"7\" selected"));

    assert_eq!(
        post(
            &app,
            "/admin/users/2/action",
            &admin,
            "action=reset&pin=1111"
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let reset = session(login(&app, "arif", "1111").await);
    assert_eq!(get(&app, "/employee", &reset).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn signup_requests_wait_for_an_admin() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let app = crate::web::router(db);
    let admin = session(login(&app, "operator", "0123").await);

    let sent = post(&app, "/signup", "", "username=nadia&pin=2468&confirm=2468").await;
    assert_eq!(sent.status(), StatusCode::OK);
    assert!(html(sent).await.contains("role=\"status\""));
    for (form, status) in [
        ("username=nadia&pin=1357&confirm=1357", StatusCode::CONFLICT),
        (
            "username=operator&pin=1357&confirm=1357",
            StatusCode::CONFLICT,
        ),
        (
            "username=omar&pin=1357&confirm=1358",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "username=omar&pin=13&confirm=13",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        assert_eq!(post(&app, "/signup", "", form).await.status(), status);
    }
    assert_eq!(
        post(&app, "/signup", "", "username=omar&pin=1357&confirm=1357")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        login(&app, "nadia", "2468").await.status(),
        StatusCode::UNAUTHORIZED
    );

    let listed = html(page(&app, "/admin/users", &admin).await).await;
    assert!(listed.contains("nadia") && listed.contains("omar"));
    assert_eq!(
        post(&app, "/admin/signups/1", "", "decision=accept&role=staff")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for (id, decision) in [(1, "accept"), (2, "reject")] {
        let form = format!("decision={decision}&role=staff");
        let path = format!("/admin/signups/{id}");
        assert_eq!(
            post(&app, &path, &admin, &form).await.status(),
            StatusCode::SEE_OTHER
        );
    }
    let nadia = session(login(&app, "nadia", "2468").await);
    assert_eq!(
        page(&app, "/", &nadia).await.headers()[header::LOCATION],
        "/staff"
    );
    assert_eq!(
        login(&app, "omar", "1357").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let listed = html(page(&app, "/admin/users", &admin).await).await;
    assert!(!listed.contains("omar"));
}
