use std::path::{Path as FsPath, PathBuf};

use askama::Template;
use axum::{
    Extension, Form, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use kringkring::i18n::Language;
use serde::Deserialize;

use super::{
    auth::{Access, protect, same_origin},
    live::{self, Cards, Changes},
    locale::Locale,
    page, reasons,
};
use crate::{
    orders::{self, Error, Reason, StaffAction},
    users::{Role, TokenHash},
};

#[derive(Template)]
#[template(path = "staff_queue.html")]
struct Queue {
    lang: Language,
    cards: Cards,
    stale: bool,
}

#[derive(Deserialize)]
struct StaffForm {
    action: StaffAction,
    preset_id: Option<i64>,
    other: Option<String>,
}

pub(super) fn routes(db: &FsPath) -> Router<PathBuf> {
    let actions = Router::new()
        .route("/staff/requests/{id}", post(mutate))
        .route("/staff/duty", post(duty))
        .route_layer(middleware::from_fn(same_origin));
    protect(
        Router::new()
            .route("/staff", get(index))
            .route("/staff/events", get(events))
            .route("/staff/requests/{id}/reject", get(reasons::reject))
            .merge(actions),
        db,
        Access::Role(Role::Staff),
    )
}

async fn index(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
) -> Response {
    render(db, token, lang, false).await
}

async fn events(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
) -> Response {
    live::subscribe(db, token, lang, changes, true)
}

async fn mutate(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
    Path(id): Path<i64>,
    Form(form): Form<StaffForm>,
) -> Response {
    let result = tokio::task::spawn_blocking({
        let db = db.clone();
        move || {
            orders::staff_action(
                &db,
                token,
                id,
                form.action,
                Reason {
                    preset_id: form.preset_id,
                    other: form.other,
                },
            )
        }
    })
    .await;
    match result {
        Ok(Ok(_)) => {
            changes.publish();
            Redirect::to("/staff").into_response()
        }
        Ok(Err(Error::Reason)) => reasons::render(db, token, lang, id, true, true).await,
        Ok(Err(Error::Conflict | Error::Paused)) => render(db, token, lang, true).await,
        Ok(Err(Error::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn render(db: PathBuf, token: TokenHash, lang: Language, stale: bool) -> Response {
    match tokio::task::spawn_blocking(move || orders::queue(&db, token, lang)).await {
        Ok(Ok(queue)) => page(
            if stale {
                StatusCode::CONFLICT
            } else {
                StatusCode::OK
            },
            Queue {
                lang,
                cards: Cards::staff(lang, queue),
                stale,
            },
        ),
        Ok(Err(Error::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct DutyForm {
    paused: bool,
}

async fn duty(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Extension(changes): Extension<Changes>,
    Form(form): Form<DutyForm>,
) -> Response {
    match tokio::task::spawn_blocking(move || orders::set_paused(&db, token, form.paused)).await {
        Ok(Ok(())) => {
            changes.publish();
            Redirect::to("/staff").into_response()
        }
        Ok(Err(Error::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests;
