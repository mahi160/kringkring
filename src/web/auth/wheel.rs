use std::path::PathBuf;

use askama::Template;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use kringkring::i18n::Language;
use serde::Deserialize;

use super::super::{
    locale::{Locale, cookie},
    page,
};
use crate::users::{self, LoginName};

pub(super) const LAST_USER: &str = "__Host-kringkring_last_user";

#[derive(Deserialize)]
pub(super) struct Selection {
    user: Option<i64>,
}

enum Step {
    Select,
    User(i64),
    Retry(String),
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    lang: Language,
    users: Vec<LoginName>,
    selected: Option<LoginName>,
    remembered: Option<i64>,
    failed: bool,
}

impl LoginPage {
    fn was_last(&self, id: &i64) -> bool {
        self.remembered == Some(*id)
    }
}

pub(super) async fn index(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    headers: HeaderMap,
    Query(selection): Query<Selection>,
) -> Response {
    let remembered = cookie(&headers, LAST_USER).and_then(|id| id.parse().ok());
    render(
        db,
        lang,
        selection.user.map_or(Step::Select, Step::User),
        remembered,
    )
    .await
}

pub(super) async fn failed(db: PathBuf, lang: Language, username: String) -> Response {
    render(db, lang, Step::Retry(username), None).await
}

async fn render(db: PathBuf, lang: Language, step: Step, remembered: Option<i64>) -> Response {
    let Ok(Ok(mut users)) = tokio::task::spawn_blocking(move || users::login_names(&db)).await
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let selected = users
        .iter()
        .position(|name| match &step {
            Step::Select => false,
            Step::User(id) => name.id == *id,
            Step::Retry(username) => name.username == *username,
        })
        .map(|index| users.remove(index));
    let failed = matches!(step, Step::Retry(_));
    let status = if failed {
        StatusCode::UNAUTHORIZED
    } else {
        StatusCode::OK
    };
    page(
        status,
        LoginPage {
            lang,
            users,
            selected,
            remembered,
            failed,
        },
    )
}
