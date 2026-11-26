use std::path::{Path, PathBuf};

use askama::Template;
use axum::{
    Extension, Form, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header, uri::Authority},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;

use super::{locale::Locale, page};
use kringkring::i18n::Language;

use crate::users::{self, Role, TokenHash};

const COOKIE: &str = "__Host-kringkring_session";
const COOKIE_FLAGS: &str = "Path=/; HttpOnly; Secure; SameSite=Lax";

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    lang: Language,
    username: String,
    failed: bool,
}

#[derive(Deserialize)]
struct Credentials {
    username: String,
    pin: String,
}

#[derive(Deserialize)]
struct RevokeSessions {
    username: String,
}

#[derive(Clone, Copy)]
pub(super) enum Access {
    /// Signed in, even before replacing an administrator-chosen PIN.
    Setup,
    SignedIn,
    Role(Role),
    Admin,
}

pub fn routes(db: &Path) -> Router<PathBuf> {
    let mutations = Router::new()
        .route("/login", post(login))
        .merge(protect(
            Router::new().route("/logout", post(logout)),
            db,
            Access::Setup,
        ))
        .merge(protect(
            Router::new().route("/admin/sessions/revoke", post(revoke)),
            db,
            Access::Admin,
        ))
        .route_layer(middleware::from_fn(same_origin));

    Router::new()
        .route(
            "/login",
            get(|Locale(lang)| async move {
                page(
                    StatusCode::OK,
                    LoginPage {
                        lang,
                        username: String::new(),
                        failed: false,
                    },
                )
            }),
        )
        .merge(mutations)
        .merge(protect(
            Router::new().route("/", get(home)),
            db,
            Access::SignedIn,
        ))
        .merge(protect(
            Router::new().route("/employee", get(allowed)),
            db,
            Access::Role(Role::Employee),
        ))
        .merge(protect(
            Router::new().route("/admin", get(allowed)),
            db,
            Access::Admin,
        ))
}

fn wants_page(request: &Request) -> bool {
    let headers = request.headers();
    request.method() == axum::http::Method::GET
        && !headers.contains_key("hx-request")
        && headers
            .get(header::ACCEPT)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|accept| accept.contains("text/html"))
}

/// Browser page loads go to the login page; API, htmx, and SSE requests get a bare 401.
fn signed_out(request: &Request) -> Response {
    if wants_page(request) {
        Redirect::to("/login").into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Until the user replaces an administrator-chosen PIN, pages go to setup and everything else is refused.
fn setup_required(request: &Request) -> Response {
    if wants_page(request) {
        Redirect::to("/setup").into_response()
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

/// Each role's main screen: employees order from the menu, staff work the queue.
async fn home(Extension(role): Extension<Role>) -> Redirect {
    Redirect::to(match role {
        Role::Employee => "/employee/menu",
        Role::Staff => "/staff",
    })
}

async fn allowed() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub(super) fn protect(router: Router<PathBuf>, db: &Path, access: Access) -> Router<PathBuf> {
    router.route_layer(middleware::from_fn_with_state(
        (db.to_owned(), access),
        authorize,
    ))
}

async fn authorize(
    State((db, access)): State<(PathBuf, Access)>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = session_token(request.headers()) else {
        return signed_out(&request);
    };
    let principal = match tokio::task::spawn_blocking(move || users::principal(&db, token)).await {
        Ok(Ok(Some(principal))) => principal,
        Ok(Ok(None)) => return signed_out(&request),
        _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let authorized = match access {
        Access::Setup => true,
        _ if principal.setup_pending => return setup_required(&request),
        Access::SignedIn => true,
        Access::Role(role) => principal.role == role,
        Access::Admin => principal.is_admin,
    };
    if !authorized {
        return StatusCode::FORBIDDEN.into_response();
    }
    request.extensions_mut().insert(token);
    request.extensions_mut().insert(principal.language);
    request.extensions_mut().insert(principal.is_admin);
    request.extensions_mut().insert(principal.role);
    next.run(request).await
}

pub(super) fn session_token(headers: &HeaderMap) -> Option<TokenHash> {
    super::locale::cookie(headers, COOKIE).and_then(users::token_hash)
}

pub(super) async fn same_origin(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let host = request.uri().authority().cloned().or_else(|| {
        headers
            .get(header::HOST)?
            .to_str()
            .ok()?
            .parse::<Authority>()
            .ok()
    });
    let trusted = origin.zip(host).is_some_and(|(origin, host)| {
        origin.strip_prefix("https://") == Some(host.as_str())
            || (matches!(host.host(), "localhost" | "127.0.0.1")
                && origin.strip_prefix("http://") == Some(host.as_str()))
    });
    if !trusted {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

async fn login(
    State(db): State<PathBuf>,
    Locale(lang): Locale,
    Form(credentials): Form<Credentials>,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        let result = users::login(&db, &credentials.username, &credentials.pin);
        (credentials.username, result)
    })
    .await;
    match result {
        Ok((_, Ok(Some(session)))) => (
            [(
                header::SET_COOKIE,
                format!(
                    "{COOKIE}={}; {COOKIE_FLAGS}; Max-Age=34560000",
                    session.token
                ),
            )],
            Redirect::to("/"),
        )
            .into_response(),
        Ok((username, Ok(None))) => page(
            StatusCode::UNAUTHORIZED,
            LoginPage {
                lang,
                username,
                failed: true,
            },
        ),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn logout(State(db): State<PathBuf>, Extension(token): Extension<TokenHash>) -> Response {
    match tokio::task::spawn_blocking(move || users::logout(&db, token)).await {
        Ok(Ok(())) => (
            [(
                header::SET_COOKIE,
                format!("{COOKIE}=; {COOKIE_FLAGS}; Max-Age=0"),
            )],
            Redirect::to("/login"),
        )
            .into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn revoke(
    State(db): State<PathBuf>,
    Extension(actor): Extension<TokenHash>,
    Form(target): Form<RevokeSessions>,
) -> StatusCode {
    match tokio::task::spawn_blocking(move || users::admin::revoke(&db, actor, &target.username))
        .await
    {
        Ok(Ok(())) => StatusCode::NO_CONTENT,
        Ok(Err(users::admin::Error::Forbidden)) => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests;
