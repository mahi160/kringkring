use std::{convert::Infallible, path::PathBuf};

use axum::{
    Form, Router,
    extract::FromRequestParts,
    http::{HeaderMap, header, request::Parts},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::post,
};
use kringkring::i18n::Language;
use serde::Deserialize;

const COOKIE: &str = "kringkring_language";

pub(super) struct Locale(pub Language);

impl FromRequestParts<PathBuf> for Locale {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, db: &PathBuf) -> Result<Self, Self::Rejection> {
        if let Some(lang) = cookie(&parts.headers, COOKIE).and_then(Language::from_code) {
            return Ok(Self(lang));
        }
        if let Some(&lang) = parts.extensions.get::<Language>() {
            return Ok(Self(lang));
        }
        if let Some(token) = super::auth::session_token(&parts.headers) {
            let db = db.clone();
            if let Ok(Ok(Some(principal))) =
                tokio::task::spawn_blocking(move || crate::users::principal(&db, token)).await
            {
                return Ok(Self(principal.language));
            }
        }
        Ok(Self(Language::default()))
    }
}

pub(super) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Page {
    Login,
    Account,
    Health,
    Users,
    Catalog,
    Menu,
    Queue,
    History,
    Status,
}

#[derive(Deserialize)]
struct Preference {
    lang: Language,
    page: Page,
}

pub(super) fn routes() -> Router<PathBuf> {
    Router::new()
        .route("/language", post(set_language))
        .route_layer(middleware::from_fn(super::auth::same_origin))
}

async fn set_language(Form(preference): Form<Preference>) -> Response {
    let return_to = match preference.page {
        Page::Login => "/login",
        Page::Account => "/account",
        Page::Health => "/health",
        Page::Users => "/admin/users",
        Page::Catalog => "/admin/catalog",
        Page::Menu => "/employee/menu",
        Page::Queue => "/staff",
        Page::History => "/admin/history",
        Page::Status => "/admin/status",
    };
    let code = preference.lang.code();
    (
        [
            (
                header::SET_COOKIE,
                format!(
                    "{COOKIE}={code}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=34560000"
                ),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Redirect::to(return_to),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
