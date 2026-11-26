//! Account page, first-use PIN setup, and public signup requests.

use std::path::{Path, PathBuf};

use askama::Template;
use axum::{
    Extension, Form, Router,
    extract::{State, rejection::FormRejection},
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
use crate::users::{
    TokenHash,
    setup::{self, Error, Settings},
    signup::{self, RequestError},
};

#[derive(Template)]
#[template(path = "account.html")]
struct AccountPage {
    lang: Language,
    is_admin: bool,
    settings: Settings,
    error: Option<&'static str>,
}

#[derive(Template)]
#[template(path = "setup.html")]
struct SetupPage {
    lang: Language,
    settings: Settings,
    error: Option<&'static str>,
}

#[derive(Template)]
#[template(path = "signup.html")]
struct SignupPage {
    lang: Language,
    username: String,
    sent: bool,
    error: Option<&'static str>,
}

#[derive(Deserialize)]
struct LocationForm {
    /// 0 clears the default location.
    default_location: i64,
}

#[derive(Deserialize)]
struct SetupForm {
    pin: String,
    confirm: String,
    #[serde(default)]
    default_location: i64,
}

#[derive(Deserialize)]
struct SignupForm {
    username: String,
    pin: String,
    confirm: String,
}

pub(super) fn routes(db: &Path) -> Router<PathBuf> {
    let signup_write = Router::new()
        .route("/signup", post(request_signup))
        .route_layer(middleware::from_fn(same_origin));
    let setup_write = Router::new()
        .route("/setup", post(complete))
        .route_layer(middleware::from_fn(same_origin));
    let account_write = Router::new()
        .route("/account/location", post(change_location))
        .route_layer(middleware::from_fn(same_origin));
    Router::new()
        .route("/signup", get(signup_page))
        .merge(signup_write)
        .merge(protect(
            Router::new()
                .route("/setup", get(setup_page))
                .merge(setup_write),
            db,
            Access::Setup,
        ))
        .merge(protect(
            Router::new()
                .route("/account", get(account))
                .merge(account_write),
            db,
            Access::SignedIn,
        ))
}

fn location(id: i64) -> Option<i64> {
    (id != 0).then_some(id)
}

fn message(lang: Language, error: Error) -> (StatusCode, &'static str) {
    let t = lang.catalog();
    match error {
        Error::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, t.pin_invalid),
        Error::Reused => (StatusCode::UNPROCESSABLE_ENTITY, t.pin_reused),
        Error::Location => (StatusCode::UNPROCESSABLE_ENTITY, t.location_invalid),
        Error::Forbidden => (StatusCode::FORBIDDEN, t.user_unavailable),
        Error::Internal => (StatusCode::INTERNAL_SERVER_ERROR, t.user_unavailable),
    }
}

async fn load(db: PathBuf, token: TokenHash) -> Result<Settings, Error> {
    tokio::task::spawn_blocking(move || setup::settings(&db, token))
        .await
        .unwrap_or(Err(Error::Internal))
}

async fn account(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Extension(is_admin): Extension<bool>,
    Locale(lang): Locale,
) -> Response {
    match load(db, token).await {
        Ok(settings) => page(
            StatusCode::OK,
            AccountPage {
                lang,
                is_admin,
                settings,
                error: None,
            },
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn change_location(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Extension(is_admin): Extension<bool>,
    Locale(lang): Locale,
    form: Result<Form<LocationForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let write_db = db.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        setup::change_location(&write_db, token, location(form.default_location))
    })
    .await
    .unwrap_or(Err(Error::Internal));
    let Err(error) = outcome else {
        return Redirect::to("/account").into_response();
    };
    let (status, error) = message(lang, error);
    match load(db, token).await {
        Ok(settings) => page(
            status,
            AccountPage {
                lang,
                is_admin,
                settings,
                error: Some(error),
            },
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn setup_page(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
) -> Response {
    match load(db, token).await {
        Ok(settings) if !settings.pending => Redirect::to("/").into_response(),
        Ok(settings) => page(
            StatusCode::OK,
            SetupPage {
                lang,
                settings,
                error: None,
            },
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn complete(
    State(db): State<PathBuf>,
    Extension(token): Extension<TokenHash>,
    Locale(lang): Locale,
    form: Result<Form<SetupForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let (status, error) = if form.pin != form.confirm {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            lang.catalog().pin_mismatch,
        )
    } else {
        let write_db = db.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            setup::complete(&write_db, token, &form.pin, location(form.default_location))
        })
        .await
        .unwrap_or(Err(Error::Internal));
        match outcome {
            Ok(()) => return Redirect::to("/").into_response(),
            Err(error) => message(lang, error),
        }
    };
    match load(db, token).await {
        Ok(settings) => page(
            status,
            SetupPage {
                lang,
                settings,
                error: Some(error),
            },
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn signup_page(Locale(lang): Locale) -> Response {
    page(
        StatusCode::OK,
        SignupPage {
            lang,
            username: String::new(),
            sent: false,
            error: None,
        },
    )
}

async fn request_signup(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    form: Result<Form<SignupForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let t = lang.catalog();
    let (status, error) = if form.pin != form.confirm {
        (StatusCode::UNPROCESSABLE_ENTITY, t.pin_mismatch)
    } else {
        let username = form.username.clone();
        let outcome =
            tokio::task::spawn_blocking(move || signup::request(&db, &username, &form.pin, lang))
                .await
                .unwrap_or(Err(RequestError::Internal));
        match outcome {
            Ok(()) => {
                return page(
                    StatusCode::OK,
                    SignupPage {
                        lang,
                        username: String::new(),
                        sent: true,
                        error: None,
                    },
                );
            }
            Err(RequestError::Invalid) => (StatusCode::UNPROCESSABLE_ENTITY, t.user_invalid),
            Err(RequestError::Taken) => (StatusCode::CONFLICT, t.signup_taken),
            Err(RequestError::Full) => (StatusCode::TOO_MANY_REQUESTS, t.signup_full),
            Err(RequestError::Internal) => (StatusCode::INTERNAL_SERVER_ERROR, t.user_unavailable),
        }
    };
    page(
        status,
        SignupPage {
            lang,
            username: form.username,
            sent: false,
            error: Some(error),
        },
    )
}
