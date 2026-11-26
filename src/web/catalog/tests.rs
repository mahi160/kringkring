use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use tower::ServiceExt;

async fn call(app: &Router, method: Method, path: &str, cookie: &str, data: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, "kringkring.local")
                .header(header::ORIGIN, "https://kringkring.local")
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(data.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn page(app: &Router, path: &str, cookie: &str) -> String {
    let response = call(app, Method::GET, path, cookie, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), 200_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn post(app: &Router, path: &str, cookie: &str, data: &str) -> StatusCode {
    call(app, Method::POST, path, cookie, data).await.status()
}

fn assert_order(html: &str, names: &[&str]) {
    let mut rest = html;
    for name in names {
        rest = rest
            .split_once(name)
            .unwrap_or_else(|| panic!("missing or out-of-order: {name}"))
            .1;
    }
}

#[tokio::test]
async fn catalog_seeds_edits_orders_and_archives_without_removing_records() {
    let temp = tempfile::tempdir().unwrap();
    let db = crate::db::initialize(temp.path()).unwrap();
    crate::users::init_admin(&db, "operator", "0123").unwrap();
    let token = crate::users::login(&db, "operator", "0123")
        .unwrap()
        .unwrap()
        .token;
    let cookie = format!("__Host-kringkring_session={token}; kringkring_language=en");
    let app = crate::web::router(db.clone());

    let menu = "/employee/menu";
    let initial = page(&app, menu, &cookie).await;
    assert_order(
        &initial,
        &[
            ">Tea</h3>",
            ">Raw tea</h3>",
            ">Black coffee</h3>",
            ">Water</h3>",
        ],
    );
    let selected = page(&app, "/employee/menu?item_id=1", &cookie).await;
    assert_order(&selected, &[">Sugar</span>", ">Milk</span>"]);

    for (kind, query) in [
        (
            "locations",
            "name_en=Boardroom&name_bn=&sort_order=20&active=true",
        ),
        (
            "locations",
            "name_en=Kitchen&name_bn=&sort_order=10&active=true",
        ),
        ("modifiers", "name_en=Ice&name_bn=&sort_order=5&active=true"),
        (
            "menu",
            "name_en=Juice&name_bn=&sort_order=0&active=true&color=teal&icon=water&modifiers=3&modifiers=1",
        ),
    ] {
        assert_eq!(
            post(&app, &format!("/admin/catalog/{kind}"), &cookie, query).await,
            StatusCode::SEE_OTHER
        );
    }
    let with_juice = page(&app, menu, &cookie).await;
    assert_order(&with_juice, &[">Juice</h3>", ">Tea</h3>"]);
    let selected = page(&app, "/employee/menu?item_id=5", &cookie).await;
    assert_order(&selected, &[">Ice</span>", ">Sugar</span>"]);
    assert_order(&selected, &[">Kitchen</span>", ">Boardroom</span>"]);

    assert_eq!(post(&app, "/admin/catalog/menu", &cookie,
        "id=5&name_en=NotSaved&name_bn=&sort_order=99&active=true&color=teal&icon=water&modifiers=999").await,
        StatusCode::UNPROCESSABLE_ENTITY);
    assert!(page(&app, menu, &cookie).await.contains(">Juice</h3>"));
    assert_eq!(
        post(
            &app,
            "/admin/catalog/menu",
            &cookie,
            "name_en=Invalid&name_bn=&sort_order=12&active=true&color=made-up&icon=water"
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let edit = "/admin/users/1/edit";
    assert_eq!(
        post(
            &app,
            edit,
            &cookie,
            "username=operator&role=employee&language=bn&is_admin=true&default_location=1"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    assert!(
        page(&app, "/admin/users", &cookie)
            .await
            .contains("value=\"1\" selected>Boardroom")
    );

    for (kind, query) in [
        ("locations", "id=1&name_en=Boardroom&name_bn=&sort_order=1"),
        (
            "locations",
            "id=2&name_en=Kitchen&name_bn=&sort_order=30&active=true",
        ),
        ("modifiers", "id=3&name_en=Ice&name_bn=&sort_order=50"),
        (
            "menu",
            "id=1&name_en=Tea&name_bn=&sort_order=10&color=blue&icon=tea&modifiers=1&modifiers=2",
        ),
        (
            "menu",
            "id=5&name_en=Fruit+juice&name_bn=&sort_order=99&active=true&color=teal&icon=water&modifiers=3",
        ),
    ] {
        assert_eq!(
            post(&app, &format!("/admin/catalog/{kind}"), &cookie, query).await,
            StatusCode::SEE_OTHER
        );
    }
    let current = page(&app, menu, &cookie).await;
    assert!(!current.contains(">Tea</h3>"));
    let selected = page(&app, "/employee/menu?item_id=5", &cookie).await;
    assert!(!selected.contains(">Boardroom</span>") && !selected.contains(">Ice</span>"));
    assert_order(&current, &[">Water</h3>", ">Fruit juice</h3>"]);
    let archived = page(&app, "/admin/catalog", &cookie).await;
    assert!(archived.contains("Boardroom") && archived.contains("Deactivated"));
    assert!(
        page(&app, "/admin/users", &cookie)
            .await
            .contains("value=\"1\" selected>Boardroom (Deactivated)")
    );

    let persisted = crate::catalog::load(&db, false).unwrap();
    let tea = persisted
        .items
        .iter()
        .find(|item| item.entry.id == 1)
        .unwrap();
    assert_eq!(tea.entry.fields.name_en, "Tea");
    assert!(!tea.entry.fields.active);
    assert_eq!(tea.modifiers, [1, 2]);
    assert!(
        persisted
            .locations
            .iter()
            .any(|location| location.id == 1 && !location.fields.active)
    );
}
