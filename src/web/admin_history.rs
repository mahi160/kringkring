use std::path::{Path, PathBuf};

use askama::Template;
use axum::{
    Extension, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use kringkring::{i18n::Language, request::Status};

use super::{
    auth::{Access, protect},
    live::{self, Changes},
    locale::Locale,
    page,
};
use crate::{
    history::{self, Filters, History},
    orders::Error,
    users::{self, TokenHash},
};

#[derive(Template)]
#[template(path = "admin_history.html")]
struct HistoryPage {
    lang: Language,
    filters: Filters,
    result: Option<History>,
    statuses: [Status; 6],
    previous: Option<u32>,
    next: Option<u32>,
}

impl HistoryPage {
    fn selected(&self, status: &Status) -> bool {
        self.filters.status == status.code()
    }
}

pub(super) fn routes(db: &Path) -> Router<PathBuf> {
    protect(
        Router::new()
            .route("/admin/history", get(history))
            .route("/admin/status", get(status))
            .route("/admin/status/events", get(events)),
        db,
        Access::Admin,
    )
}

async fn history(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    query: Result<Query<Filters>, QueryRejection>,
) -> Response {
    let Ok(Query(filters)) = query else {
        return history_page(lang, Filters::default(), None);
    };
    let loaded = tokio::task::spawn_blocking(move || {
        let result = history::load(&db, &filters, lang);
        (filters, result)
    })
    .await;
    match loaded {
        Ok((filters, Ok(result))) => history_page(lang, filters, Some(result)),
        Ok((filters, Err(history::Error::InvalidFilter))) => history_page(lang, filters, None),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn history_page(lang: Language, filters: Filters, result: Option<History>) -> Response {
    let previous = (filters.page > 1).then(|| filters.page - 1);
    let next = if result.as_ref().is_some_and(|result| result.has_next) {
        filters.page.max(1).checked_add(1)
    } else {
        None
    };
    let code = if result.is_some() {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    page(
        code,
        HistoryPage {
            lang,
            filters,
            result,
            statuses: Status::ALL,
            previous,
            next,
        },
    )
}

struct StaffStatus {
    id: i64,
    name: String,
    paused: bool,
    on_duty: bool,
    online: bool,
}

#[derive(Template)]
#[template(path = "staff_status_rows.html")]
struct StatusRows {
    lang: Language,
    staff: Vec<StaffStatus>,
}

#[derive(Template)]
#[template(path = "staff_status.html")]
struct StatusPage {
    lang: Language,
    rows: StatusRows,
}

fn status_rows(db: &Path, lang: Language, changes: &Changes) -> rusqlite::Result<StatusRows> {
    let online = changes.online();
    let conn = users::connect(db)?;
    let mut query = conn.prepare(
        "SELECT users.id, username, staff_paused, token_hash FROM users
         LEFT JOIN sessions ON users.id = sessions.user_id
         WHERE active = 1 AND role = 'staff'
         ORDER BY username COLLATE NOCASE, users.id",
    )?;
    let mut rows = query.query([])?;
    let mut staff = Vec::<StaffStatus>::new();
    while let Some(row) = rows.next()? {
        let id = row.get(0)?;
        let token: Option<TokenHash> = row.get(3)?;
        let row_online = token.is_some_and(|token| online.contains(&token));
        if let Some(last) = staff.last_mut().filter(|entry| entry.id == id) {
            last.online |= row_online;
        } else {
            let paused = row.get(2)?;
            staff.push(StaffStatus {
                id,
                paused,
                name: row.get(1)?,
                on_duty: !paused && token.is_some(),
                online: row_online,
            });
        }
    }
    Ok(StatusRows { lang, staff })
}

async fn status(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
) -> Response {
    match tokio::task::spawn_blocking(move || status_rows(&db, lang, &changes)).await {
        Ok(Ok(rows)) => page(StatusCode::OK, StatusPage { lang, rows }),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn events(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
    Extension(token): Extension<TokenHash>,
) -> Response {
    live::stream(changes.clone(), None, move || {
        if !users::principal(&db, token)
            .map_err(|_| Error::Database)?
            .is_some_and(|user| user.is_admin)
        {
            return Err(Error::Forbidden);
        }
        status_rows(&db, lang, &changes)?
            .render()
            .map_err(|_| Error::Database)
    })
}

#[cfg(test)]
mod tests;
