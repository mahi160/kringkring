use std::path::{Path as FsPath, PathBuf};

use askama::Template;
use axum::{
    Extension, Form, Router,
    extract::{Path, State, rejection::FormRejection},
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
    Role, TokenHash,
    admin::{self, Change, Error, Profile, User},
};

const USERS: &str = "/admin/users";

#[derive(Template)]
#[template(path = "admin_users.html")]
struct UsersPage {
    lang: Language,
    users: Vec<User>,
    locations: Vec<crate::catalog::Entry>,
}

#[derive(Template)]
#[template(path = "admin_users_error.html")]
struct ErrorPage {
    lang: Language,
    message: &'static str,
}

#[derive(Deserialize)]
struct ProfileForm {
    username: String,
    role: Role,
    language: Language,
    #[serde(default)]
    is_admin: bool,
    #[serde(default)]
    pin: String,
    default_location: Option<i64>,
}

impl ProfileForm {
    fn parts(self) -> (Profile, String) {
        (
            Profile {
                username: self.username,
                role: self.role,
                language: self.language,
                is_admin: self.is_admin,
                default_location: self.default_location.filter(|id| *id != 0),
            },
            self.pin,
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Reset,
    Unlock,
    Revoke,
    Deactivate,
}

#[derive(Deserialize)]
struct ActionForm {
    action: Action,
    #[serde(default)]
    pin: String,
}

pub(super) fn routes(db: &FsPath) -> Router<PathBuf> {
    let writes = Router::new()
        .route(USERS, post(create))
        .route("/admin/users/{id}/edit", post(edit))
        .route("/admin/users/{id}/action", post(action))
        .route_layer(middleware::from_fn(same_origin));
    protect(
        Router::new().route(USERS, get(index)).merge(writes),
        db,
        Access::Admin,
    )
}

async fn index(State(db): State<PathBuf>, Locale(lang): Locale) -> Response {
    match tokio::task::spawn_blocking(move || {
        let users = admin::list(&db)?;
        let locations = crate::catalog::choices(
            &crate::users::connect(&db)?,
            crate::catalog::Kind::Locations,
            false,
        )?;
        Ok::<_, Error>(UsersPage {
            lang,
            users,
            locations,
        })
    })
    .await
    {
        Ok(Ok(view)) => page(StatusCode::OK, view),
        _ => error(lang, Error::Internal),
    }
}

async fn create(
    State(db): State<PathBuf>,
    Extension(actor): Extension<TokenHash>,
    Locale(lang): Locale,
    form: Result<Form<ProfileForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return error(lang, Error::Invalid);
    };
    let (profile, pin) = form.parts();
    finish(
        lang,
        tokio::task::spawn_blocking(move || {
            admin::create(&db, actor, profile, &pin).map(|()| true)
        })
        .await,
    )
}

async fn edit(
    State(db): State<PathBuf>,
    Extension(actor): Extension<TokenHash>,
    Locale(lang): Locale,
    Path(id): Path<i64>,
    form: Result<Form<ProfileForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return error(lang, Error::Invalid);
    };
    let (profile, _) = form.parts();
    change(db, actor, lang, id, Change::Edit(profile)).await
}

async fn action(
    State(db): State<PathBuf>,
    Extension(actor): Extension<TokenHash>,
    Locale(lang): Locale,
    Path(id): Path<i64>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return error(lang, Error::Invalid);
    };
    let change = match form.action {
        Action::Reset => Change::ResetPin(form.pin),
        Action::Unlock => Change::Unlock,
        Action::Revoke => Change::Revoke,
        Action::Deactivate => Change::Deactivate,
    };
    self::change(db, actor, lang, id, change).await
}

async fn change(
    db: PathBuf,
    actor: TokenHash,
    lang: Language,
    id: i64,
    change: Change,
) -> Response {
    finish(
        lang,
        tokio::task::spawn_blocking(move || admin::apply(&db, actor, id, change)).await,
    )
}

fn finish(
    lang: Language,
    outcome: Result<Result<bool, Error>, tokio::task::JoinError>,
) -> Response {
    match outcome {
        Ok(Ok(true)) => Redirect::to(USERS).into_response(),
        Ok(Ok(false)) => Redirect::to("/login").into_response(),
        Ok(Err(reason)) => error(lang, reason),
        Err(_) => error(lang, Error::Internal),
    }
}

fn error(lang: Language, error: Error) -> Response {
    let t = lang.catalog();
    let (status, message) = match error {
        Error::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, t.user_invalid),
        Error::Duplicate => (StatusCode::CONFLICT, t.username_taken),
        Error::Location => (StatusCode::UNPROCESSABLE_ENTITY, t.location_invalid),
        Error::LastAdmin => (StatusCode::CONFLICT, t.last_admin),
        Error::NotFound => (StatusCode::NOT_FOUND, t.user_not_found),
        Error::Forbidden => (StatusCode::FORBIDDEN, t.admin_required),
        Error::Internal => (StatusCode::INTERNAL_SERVER_ERROR, t.user_unavailable),
    };
    page(status, ErrorPage { lang, message })
}

#[cfg(test)]
mod tests;
