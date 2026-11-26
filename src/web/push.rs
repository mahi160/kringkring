use std::path::{Path, PathBuf};

use axum::{
    Extension, Json, Router,
    extract::State,
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;

use super::{
    ASSETS,
    auth::{Access, protect, same_origin},
    locale::Locale,
};
use crate::{
    push::{self, Keys, SubscribeError},
    users::TokenHash,
};

/// The server's VAPID public key (base64url), added as an extension by `serve`.
#[derive(Clone)]
pub struct PushKey(pub String);

#[derive(Deserialize)]
struct Subscription {
    endpoint: String,
    keys: Keys,
}

pub(super) fn routes(db: &Path) -> Router<PathBuf> {
    let mut router = Router::new().route(
        "/push/key",
        get(|Extension(PushKey(key)): Extension<PushKey>| async move { key }),
    );
    // Service worker and manifest must live at the root so their scope covers the whole app.
    for (path, file, content_type) in [
        ("/sw.js", "sw.js", "text/javascript; charset=utf-8"),
        (
            "/manifest.webmanifest",
            "manifest.webmanifest",
            "application/manifest+json",
        ),
        ("/icon-192.png", "icon-192.png", "image/png"),
        ("/icon-512.png", "icon-512.png", "image/png"),
    ] {
        let body = ASSETS
            .get_file(file)
            .expect("registered embedded asset")
            .contents();
        router = router.route(
            path,
            get(move || async move {
                (
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::CACHE_CONTROL, "no-cache"),
                    ],
                    body,
                )
            }),
        );
    }
    router.merge(protect(
        Router::new()
            .route("/push/subscriptions", post(subscribe))
            .route_layer(middleware::from_fn(same_origin)),
        db,
        Access::SignedIn,
    ))
}

async fn subscribe(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Json(subscription): Json<Subscription>,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        push::subscribe(&db, token, &subscription.endpoint, &subscription.keys, lang)
    })
    .await;
    match result {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(SubscribeError::Invalid)) => StatusCode::UNPROCESSABLE_ENTITY.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
