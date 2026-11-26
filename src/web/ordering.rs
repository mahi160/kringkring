use std::path::{Path, PathBuf};

use askama::Template;
use axum::{
    Extension, Form, Router,
    extract::{
        Path as RequestPath, Query, RawForm, State,
        rejection::{QueryRejection, RawFormRejection},
    },
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
    catalog::{self, Catalog},
    orders::{self, Draft, EmployeeAction, Error, Reason},
    users::{Role, TokenHash},
};

const MENU: &str = "/employee/menu";

#[derive(Template)]
#[template(path = "employee_menu.html")]
struct EmployeeMenu {
    lang: Language,
    data: Catalog,
    cards: Cards,
    composer: Option<Composer>,
    error: Option<&'static str>,
}

impl EmployeeMenu {
    fn selected(&self, id: &i64) -> bool {
        self.composer.as_ref().and_then(|draft| draft.item_id) == Some(*id)
    }
}

struct Composer {
    item_id: Option<i64>,
    title: String,
    custom_text: String,
    allowed_modifiers: Vec<i64>,
    modifiers: Vec<i64>,
    location_id: Option<i64>,
}

impl Composer {
    fn at(&self, id: &i64) -> bool {
        self.location_id == Some(*id)
    }
}

#[derive(Deserialize, Default)]
struct Selection {
    item_id: Option<i64>,
    #[serde(default)]
    custom: bool,
}

#[derive(Deserialize)]
struct OrderForm {
    item_id: Option<i64>,
    custom_text: Option<String>,
    location_id: Option<i64>,
    #[serde(default)]
    modifiers: Vec<i64>,
}

impl From<Selection> for Draft {
    fn from(selection: Selection) -> Self {
        Self {
            item_id: selection.item_id,
            custom_text: selection.custom.then(String::new),
            location_id: None,
            modifiers: Vec::new(),
        }
    }
}

pub(super) fn routes(db: &Path) -> Router<PathBuf> {
    let write = Router::new()
        .route(MENU, post(send))
        .route("/employee/requests/{id}", post(cancel))
        .route_layer(middleware::from_fn(same_origin));
    protect(
        Router::new()
            .route(MENU, get(index))
            .route("/employee/events", get(events))
            .route("/employee/requests/{id}/cancel", get(reasons::cancel))
            .merge(write),
        db,
        Access::Role(Role::Employee),
    )
}

async fn index(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    selection: Result<Query<Selection>, QueryRejection>,
) -> Response {
    let (selection, error) = match selection {
        Ok(Query(selection)) => (selection, None),
        Err(_) => (Selection::default(), Some(Error::Invalid)),
    };
    render(db, token, lang, selection.into(), error).await
}

async fn events(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
) -> Response {
    live::subscribe(db, token, lang, changes, false)
}

async fn send(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
    form: Result<RawForm, RawFormRejection>,
) -> Response {
    let parsed = form
        .ok()
        .and_then(|RawForm(bytes)| serde_html_form::from_bytes::<OrderForm>(&bytes).ok());
    let Some(form) = parsed else {
        return render(
            db,
            token,
            lang,
            Selection::default().into(),
            Some(Error::Invalid),
        )
        .await;
    };
    let draft = Draft {
        item_id: form.item_id,
        custom_text: form.custom_text,
        location_id: form.location_id,
        modifiers: form.modifiers,
    };
    let result = tokio::task::spawn_blocking({
        let db = db.clone();
        move || {
            let result = orders::send(&db, token, &draft);
            (draft, result)
        }
    })
    .await;
    match result {
        Ok((_, Ok(()))) => {
            changes.publish();
            Redirect::to("/employee/menu#requests").into_response()
        }
        Ok((draft, Err(error))) => render(db, token, lang, draft, Some(error)).await,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn render(
    db: PathBuf,
    token: TokenHash,
    lang: Language,
    draft: Draft,
    mut error: Option<Error>,
) -> Response {
    let loaded = tokio::task::spawn_blocking(move || {
        let overview = orders::overview(&db, token, lang)?;
        let data = catalog::load(&db, true)?;
        Ok::<_, Error>((overview, data))
    })
    .await;
    let (overview, data) = match loaded {
        Ok(Ok(view)) => view,
        Ok(Err(Error::Forbidden)) => return StatusCode::FORBIDDEN.into_response(),
        _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let t = lang.catalog();
    let selection = match (draft.item_id, draft.custom_text.as_ref()) {
        (Some(id), None) => data
            .items
            .iter()
            .find(|item| item.entry.id == id)
            .map(|item| (item.entry.name(&lang).to_owned(), item.modifiers.clone())),
        (None, Some(_)) => Some((t.request_custom.to_owned(), Vec::new())),
        (None, None) => None,
        _ => None,
    };
    if selection.is_none()
        && (draft.item_id.is_some() || draft.custom_text.is_some())
        && error.is_none()
    {
        error = Some(Error::Invalid);
    }
    let composer = selection.map(|(title, allowed_modifiers)| Composer {
        title,
        allowed_modifiers,
        item_id: draft.item_id,
        custom_text: draft.custom_text.unwrap_or_default(),
        location_id: draft.location_id.or(overview.default_location),
        modifiers: draft.modifiers,
    });
    let (status, error) = match error {
        None => (StatusCode::OK, None),
        Some(Error::Forbidden | Error::Paused) => return StatusCode::FORBIDDEN.into_response(),
        Some(Error::Conflict) => (StatusCode::CONFLICT, Some(t.request_changed)),
        Some(Error::Invalid | Error::Reason) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Some(t.request_invalid))
        }
        Some(Error::Location) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(t.request_location_required),
        ),
        Some(Error::Limit) => (StatusCode::CONFLICT, Some(t.request_limit)),
        Some(Error::NoStaff) => (StatusCode::SERVICE_UNAVAILABLE, Some(t.no_staff_available)),
        Some(Error::Database) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(t.request_unavailable),
        ),
    };
    page(
        status,
        EmployeeMenu {
            lang,
            data,
            cards: Cards::employee(lang, overview.requests),
            composer,
            error,
        },
    )
}

#[derive(Deserialize)]
struct CancelForm {
    action: EmployeeAction,
    preset_id: Option<i64>,
    other: Option<String>,
}

async fn cancel(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    Extension(changes): Extension<Changes>,
    RequestPath(id): RequestPath<i64>,
    Form(form): Form<CancelForm>,
) -> Response {
    let result = tokio::task::spawn_blocking({
        let db = db.clone();
        move || {
            orders::employee_action(
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
        Ok(Ok(())) => {
            changes.publish();
            Redirect::to("/employee/menu#requests").into_response()
        }
        Ok(Err(Error::Reason)) => reasons::render(db, token, lang, id, false, true).await,
        Ok(Err(Error::Conflict)) => {
            render(
                db,
                token,
                lang,
                Selection::default().into(),
                Some(Error::Conflict),
            )
            .await
        }
        Ok(Err(Error::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests;
