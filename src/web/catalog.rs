use std::path::{Path as FsPath, PathBuf};

use askama::Template;
use axum::{
    Extension, Router,
    extract::{Path, RawForm, State, rejection::RawFormRejection},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use kringkring::i18n::Language;
use serde::Deserialize;

use super::{
    auth::{Access, protect, same_origin},
    locale::Locale,
    page,
};
use crate::{
    catalog::{self, Catalog, Fields, ItemFields, Kind},
    users::{TokenHash, admin::Error},
};

const CATALOG: &str = "/admin/catalog";

#[derive(Template)]
#[template(path = "admin_catalog.html")]
struct AdminCatalog {
    lang: Language,
    data: Catalog,
    colors: &'static [&'static str],
    icons: &'static [&'static str],
    rejects: Vec<catalog::Entry>,
    cancels: Vec<catalog::Entry>,
}

#[derive(Template)]
#[template(path = "catalog_error.html")]
struct CatalogError {
    lang: Language,
    message: &'static str,
}

#[derive(Deserialize)]
struct CatalogForm {
    id: Option<i64>,
    name_en: String,
    name_bn: String,
    sort_order: i32,
    #[serde(default)]
    active: bool,
    color: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    modifiers: Vec<i64>,
}

pub(super) fn routes(db: &FsPath) -> Router<PathBuf> {
    let writes = Router::new()
        .route("/admin/catalog/{kind}", post(save))
        .route_layer(middleware::from_fn(same_origin));
    protect(
        Router::new().route(CATALOG, get(index)).merge(writes),
        db,
        Access::Admin,
    )
}

async fn index(State(db): State<PathBuf>, Locale(lang): Locale) -> Response {
    match tokio::task::spawn_blocking(move || {
        let conn = crate::users::connect(&db)?;
        Ok::<_, Error>(AdminCatalog {
            lang,
            data: catalog::load(&db, false)?,
            colors: catalog::COLORS,
            icons: catalog::ICONS,
            rejects: catalog::choices(&conn, Kind::RejectReasons, false)?,
            cancels: catalog::choices(&conn, Kind::CancelReasons, false)?,
        })
    })
    .await
    {
        Ok(Ok(view)) => page(StatusCode::OK, view),
        _ => failure(lang, Error::Internal),
    }
}

async fn save(
    State(db): State<PathBuf>,
    Extension(actor): Extension<TokenHash>,
    Locale(lang): Locale,
    Path(kind): Path<Kind>,
    form: Result<RawForm, RawFormRejection>,
) -> Response {
    let Ok(RawForm(bytes)) = form else {
        return failure(lang, Error::Invalid);
    };
    let Ok(form) = serde_html_form::from_bytes::<CatalogForm>(&bytes) else {
        return failure(lang, Error::Invalid);
    };
    let result = tokio::task::spawn_blocking(move || {
        let fields = Fields {
            name_en: form.name_en,
            name_bn: form.name_bn,
            sort_order: form.sort_order,
            active: form.active,
        };
        match kind {
            Kind::Menu => {
                let (Some(color), Some(icon)) = (form.color, form.icon) else {
                    return Err(Error::Invalid);
                };
                catalog::save_item(
                    &db,
                    actor,
                    form.id,
                    ItemFields {
                        fields,
                        color,
                        icon,
                        modifiers: form.modifiers,
                    },
                )
            }
            _ => catalog::save_choice(&db, actor, kind, form.id, fields),
        }
    })
    .await;
    match result {
        Ok(Ok(())) => Redirect::to(match kind {
            Kind::RejectReasons => "/admin/catalog#rejectreasons",
            Kind::CancelReasons => "/admin/catalog#cancelreasons",
            _ => CATALOG,
        })
        .into_response(),
        Ok(Err(error)) => failure(lang, error),
        Err(_) => failure(lang, Error::Internal),
    }
}

fn failure(lang: Language, error: Error) -> Response {
    let t = lang.catalog();
    let (status, message) = match error {
        Error::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, t.catalog_invalid),
        Error::Forbidden => (StatusCode::FORBIDDEN, t.admin_required),
        Error::NotFound => (StatusCode::NOT_FOUND, t.catalog_not_found),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, t.catalog_unavailable),
    };
    page(status, CatalogError { lang, message })
}

#[cfg(test)]
mod tests;
