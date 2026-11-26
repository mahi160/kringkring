mod admin_history;
mod admin_users;
mod auth;
mod catalog;
mod live;
mod locale;
mod ordering;
mod push;
mod reasons;
mod staff;

pub use push::PushKey;

use std::path::PathBuf;

use askama::Template;
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use include_dir::{Dir, include_dir};
use kringkring::i18n::Language;
use locale::Locale;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets");

#[derive(Template)]
#[template(path = "health.html")]
struct HealthPage {
    lang: Language,
    available: bool,
}

pub fn router(db: PathBuf) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .merge(auth::routes(&db))
        .merge(admin_users::routes(&db))
        .merge(admin_history::routes(&db))
        .merge(catalog::routes(&db))
        .merge(ordering::routes(&db))
        .merge(staff::routes(&db))
        .merge(push::routes(&db))
        .merge(locale::routes());
    router = assets(router);

    router
        .layer(axum::Extension(live::Changes::new(db.clone())))
        .with_state(db)
}

fn assets<S: Clone + Send + Sync + 'static>(mut router: Router<S>) -> Router<S> {
    for (file, content_type) in [
        ("htmx.min.js", "text/javascript; charset=utf-8"),
        ("htmx-sse.min.js", "text/javascript; charset=utf-8"),
        ("queue-clock.js", "text/javascript; charset=utf-8"),
        ("feedback.js", "text/javascript; charset=utf-8"),
        ("push.js", "text/javascript; charset=utf-8"),
        ("theme.js", "text/javascript; charset=utf-8"),
        ("login.js", "text/javascript; charset=utf-8"),
        ("metro.css", "text/css; charset=utf-8"),
        ("fonts.css", "text/css; charset=utf-8"),
    ] {
        let asset = ASSETS
            .get_file(file)
            .expect("registered embedded asset")
            .contents();
        router = router.route(
            &format!("/assets/{file}"),
            get(move || async move { ([(header::CONTENT_TYPE, content_type)], asset) }),
        );
    }
    router
}

async fn health(State(db): State<PathBuf>, Locale(lang): Locale) -> Response {
    let available = matches!(
        tokio::task::spawn_blocking(move || crate::db::check(&db)).await,
        Ok(Ok(()))
    );
    let status = if available {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    page(status, HealthPage { lang, available })
}

fn page(status: StatusCode, template: impl Template) -> Response {
    match template.render() {
        Ok(html) => (status, [(header::CACHE_CONTROL, "no-store")], Html(html)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    async fn get(app: Router, path: &str) -> axum::response::Response {
        app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn health_reflects_database_availability() {
        let temp = tempfile::tempdir().unwrap();
        let path = crate::db::initialize(temp.path()).unwrap();
        let app = super::router(path.clone());
        let response = get(app.clone(), "/health").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("ডেটাবেস: সচল"));

        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            get(app, "/health").await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(!path.exists(), "health checks must not create a database");
    }

    #[tokio::test]
    async fn serves_vendored_htmx() {
        let temp = tempfile::tempdir().unwrap();
        let path = crate::db::initialize(temp.path()).unwrap();
        let response = get(super::router(path), "/assets/htmx.min.js").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("2.0.11"));
    }
}
