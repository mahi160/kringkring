use std::path::PathBuf;

use askama::Template;
use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kringkring::{i18n::Language, request::Status};

use super::{locale::Locale, page};
use crate::{
    catalog::{self, Entry, Kind},
    orders::{self, Error},
    users::{TokenHash, connect},
};

#[derive(Template)]
#[template(path = "reason_picker.html")]
struct Picker {
    lang: Language,
    id: i64,
    request_name: String,
    presets: Vec<Entry>,
    staff: bool,
    invalid: bool,
}

pub(super) async fn reject(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Path(id): Path<i64>,
) -> Response {
    render(db, token, lang, id, true, false).await
}

pub(super) async fn cancel(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Path(id): Path<i64>,
) -> Response {
    render(db, token, lang, id, false, false).await
}

pub(super) async fn render(
    db: PathBuf,
    token: TokenHash,
    lang: Language,
    id: i64,
    staff: bool,
    invalid: bool,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        let (requests, kind, status) = if staff {
            (
                orders::queue(&db, token, lang)?.requests,
                Kind::RejectReasons,
                Status::Pending,
            )
        } else {
            (
                orders::overview(&db, token, lang)?.requests,
                Kind::CancelReasons,
                Status::Accepted,
            )
        };
        let request = requests
            .into_iter()
            .find(|r| r.id == id && r.status == status && !r.undo_pending)
            .ok_or(Error::Conflict)?;
        let presets = catalog::choices(&connect(&db)?, kind, true)?;
        Ok::<_, Error>(Picker {
            lang,
            id,
            request_name: request.name,
            presets,
            staff,
            invalid,
        })
    })
    .await;
    match result {
        Ok(Ok(view)) => page(
            if invalid {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::OK
            },
            view,
        ),
        Ok(Err(Error::Conflict)) => {
            (StatusCode::CONFLICT, lang.catalog().queue_stale).into_response()
        }
        Ok(Err(Error::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests;
