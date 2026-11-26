use std::{error::Error, fs, future::Future, path::Path, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use kringkring::i18n::Language;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use web_push_native::{
    Auth, WebPushBuilder,
    jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair},
    p256::PublicKey,
};

use crate::users::connect;

const KEY_FILE: &str = "vapid.pem";
/// Pushes older than this are useless: pending requests are re-pushed or expire by then.
const TTL: Duration = Duration::from_secs(600);
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    New,
    Cancelled,
    Accepted,
    Rejected,
    Done,
}

impl Event {
    fn code(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Cancelled => "cancelled",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Done => "done",
        }
    }

    fn for_staff(self) -> bool {
        matches!(self, Self::New | Self::Cancelled)
    }
}

/// Records a push in the caller's transaction so it is sent only if the change commits.
pub fn enqueue(db: &Connection, request_id: i64, event: Event) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO push_outbox (request_id, event) VALUES (?1, ?2)",
        params![request_id, event.code()],
    )
    .map(drop)
}

pub struct Vapid {
    key: ES256KeyPair,
    pub public_key: String,
    contact: String,
}

impl Vapid {
    pub fn load_or_create(
        data_dir: &Path,
        contact: String,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = data_dir.join(KEY_FILE);
        let key = match fs::read_to_string(&path) {
            Ok(pem) => ES256KeyPair::from_pem(&pem)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let key = ES256KeyPair::generate();
                write_private(&path, key.to_pem()?.as_bytes())?;
                key
            }
            Err(error) => return Err(error.into()),
        };
        let public_key =
            URL_SAFE_NO_PAD.encode(key.public_key().public_key().to_bytes_uncompressed());
        Ok(Self {
            key,
            public_key,
            contact,
        })
    }
}

fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    // NamedTempFile is created with mode 0600 and renamed into place atomically.
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    std::io::Write::write_all(&mut file, contents)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Browser subscription keys as sent by `PushSubscription.toJSON()`.
#[derive(serde::Deserialize)]
pub struct Keys {
    pub p256dh: String,
    pub auth: String,
}

pub enum SubscribeError {
    Invalid,
    Database,
}

/// Stores a subscription for the signed-in device, replacing any earlier owner of the endpoint.
pub fn subscribe(
    path: &Path,
    token: crate::users::TokenHash,
    endpoint: &str,
    keys: &Keys,
    language: Language,
) -> Result<(), SubscribeError> {
    let valid_keys = URL_SAFE_NO_PAD
        .decode(keys.p256dh.trim_end_matches('='))
        .ok()
        .is_some_and(|key| PublicKey::from_sec1_bytes(&key).is_ok())
        && URL_SAFE_NO_PAD
            .decode(keys.auth.trim_end_matches('='))
            .is_ok_and(|auth| auth.len() == 16);
    if !valid_keys || !trusted_endpoint(endpoint) {
        return Err(SubscribeError::Invalid);
    }
    connect(path)
        .and_then(|db| {
            db.execute(
                "INSERT INTO push_subscriptions (endpoint, session_token_hash, p256dh, auth, language)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (endpoint) DO UPDATE SET session_token_hash = excluded.session_token_hash,
                 p256dh = excluded.p256dh, auth = excluded.auth, language = excluded.language",
                params![endpoint, token, keys.p256dh, keys.auth, language.code()],
            )
        })
        .map(drop)
        .map_err(|_| SubscribeError::Database)
}

/// Only browser push services; never let a client make the server POST to arbitrary hosts.
fn trusted_endpoint(endpoint: &str) -> bool {
    let Ok(uri) = endpoint.parse::<http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("https")
        && uri.port().is_none()
        && uri
            .authority()
            .is_some_and(|authority| !authority.as_str().contains('@'))
        && uri.host().is_some_and(|host| {
            host == "fcm.googleapis.com"
                || host == "web.push.apple.com"
                || host.ends_with(".web.push.apple.com")
                || host == "updates.push.services.mozilla.com"
        })
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Payload {
    pub title: String,
    pub body: String,
    pub url: String,
    pub tag: String,
    /// Set for staff "new request" pushes; Android shows Accept/Reject buttons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Actions>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Actions {
    pub request: i64,
    pub accept: &'static str,
    pub reject: &'static str,
}

pub struct Delivery {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub payload: Payload,
}

/// Claims every queued event and renders one payload per recipient device.
pub fn take(path: &Path) -> rusqlite::Result<Vec<Delivery>> {
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let queued = {
        let mut statement =
            tx.prepare("SELECT id, request_id, event FROM push_outbox ORDER BY id")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut deliveries = Vec::new();
    for (id, request, event) in queued {
        tx.execute("DELETE FROM push_outbox WHERE id = ?1", [id])?;
        let Some(event) = parse(&event) else { continue };
        deliveries.extend(render(&tx, request, event)?);
    }
    tx.commit()?;
    Ok(deliveries)
}

fn parse(code: &str) -> Option<Event> {
    [
        Event::New,
        Event::Cancelled,
        Event::Accepted,
        Event::Rejected,
        Event::Done,
    ]
    .into_iter()
    .find(|event| event.code() == code)
}

struct RequestText {
    name_en: String,
    name_bn: String,
    location_en: String,
    location_bn: String,
    requester: String,
    accepter: Option<String>,
    reason_en: String,
    reason_bn: String,
}

fn render(db: &Connection, request: i64, event: Event) -> rusqlite::Result<Vec<Delivery>> {
    let Some(text) = db
        .query_row(
            "SELECT COALESCE(custom_text, menu_items.name_en), COALESCE(custom_text, menu_items.name_bn),
             locations.name_en, locations.name_bn, requester.username, accepter.username,
             reason_en, reason_bn
             FROM requests LEFT JOIN menu_items ON menu_items.id = item_id
             JOIN locations ON locations.id = location_id
             JOIN users AS requester ON requester.id = employee_id
             LEFT JOIN users AS accepter ON accepter.id = accepted_by
             WHERE requests.id = ?1",
            [request],
            |row| {
                Ok(RequestText {
                    name_en: row.get(0)?,
                    name_bn: row.get(1)?,
                    location_en: row.get(2)?,
                    location_bn: row.get(3)?,
                    requester: row.get(4)?,
                    accepter: row.get(5)?,
                    reason_en: row.get(6)?,
                    reason_bn: row.get(7)?,
                })
            },
        )
        .optional()?
    else {
        return Ok(Vec::new());
    };
    // Staff pushes go to on-duty staff only: signed in (has a session) and not paused.
    let mut statement = db.prepare(
        "SELECT endpoint, p256dh, auth, language = 'en' FROM push_subscriptions
         JOIN sessions ON sessions.token_hash = session_token_hash JOIN users ON users.id = sessions.user_id
         WHERE users.active = 1 AND CASE WHEN ?2
             THEN users.role = 'staff' AND users.staff_paused = 0
             ELSE users.id = (SELECT employee_id FROM requests WHERE id = ?1) END",
    )?;
    let mut modifiers = crate::orders::modifier_query(db)?;
    let recipients = statement
        .query_map(params![request, event.for_staff()], |row| {
            let lang = if row.get(3)? {
                Language::En
            } else {
                Language::Bn
            };
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, lang))
        })?
        .collect::<rusqlite::Result<Vec<(String, String, String, Language)>>>()?;
    recipients
        .into_iter()
        .map(|(endpoint, p256dh, auth, lang)| {
            let extras = crate::orders::read_modifiers(&mut modifiers, request, lang)?;
            Ok(Delivery {
                endpoint,
                p256dh,
                auth,
                payload: payload(&text, &extras, request, event, lang),
            })
        })
        .collect()
}

fn payload(
    text: &RequestText,
    modifiers: &[String],
    request: i64,
    event: Event,
    lang: Language,
) -> Payload {
    let t = lang.catalog();
    let mut item = lang.bilingual(&text.name_en, &text.name_bn).to_owned();
    if !modifiers.is_empty() {
        item = format!("{item} ({})", modifiers.join(", "));
    }
    let location = lang.bilingual(&text.location_en, &text.location_bn);
    let (title, body) = match event {
        Event::New => (
            t.push_new,
            format!("{item} · {location} · {}", text.requester),
        ),
        Event::Cancelled => (
            t.cancelled,
            format!("{item} · {location} · {}", text.requester),
        ),
        Event::Accepted => (
            t.request_accepted,
            format!("{item} · {}", text.accepter.as_deref().unwrap_or_default()),
        ),
        Event::Rejected => {
            let reason = lang.bilingual(&text.reason_en, &text.reason_bn);
            (t.request_rejected, format!("{item} · {reason}"))
        }
        Event::Done => (t.delivered, item),
    };
    let page = if event.for_staff() {
        "/staff"
    } else {
        "/employee/menu"
    };
    Payload {
        title: title.to_owned(),
        body,
        url: format!("{page}#request-{request}"),
        tag: format!("request-{request}"),
        actions: (event == Event::New).then_some(Actions {
            request,
            accept: t.queue_accept,
            reject: t.queue_reject,
        }),
    }
}

pub enum Outcome {
    Sent,
    /// The push service says the subscription is gone (404/410).
    Gone,
    Failed,
}

pub trait Transport: Send + Sync + 'static {
    fn send(&self, request: http::Request<Vec<u8>>) -> impl Future<Output = Outcome> + Send;
}

/// Sends every queued push once; returns how many devices were attempted.
pub async fn dispatch<T: Transport>(path: &Path, vapid: &Arc<Vapid>, transport: &Arc<T>) -> usize {
    let db = path.to_owned();
    let deliveries = match tokio::task::spawn_blocking(move || take(&db)).await {
        Ok(Ok(deliveries)) => deliveries,
        _ => {
            eprintln!("Could not read the push outbox");
            return 0;
        }
    };
    let count = deliveries.len();
    let mut sends = tokio::task::JoinSet::new();
    for delivery in deliveries {
        let Some(request) = build(vapid, &delivery) else {
            eprintln!("Could not build a push; dropping the subscription");
            sends.spawn(async move { (delivery.endpoint, Outcome::Gone) });
            continue;
        };
        let transport = Arc::clone(transport);
        sends.spawn(async move {
            let outcome = tokio::time::timeout(SEND_TIMEOUT, transport.send(request))
                .await
                .unwrap_or(Outcome::Failed);
            (delivery.endpoint, outcome)
        });
    }
    while let Some(result) = sends.join_next().await {
        let Ok((endpoint, outcome)) = result else {
            continue;
        };
        match outcome {
            Outcome::Sent => {}
            Outcome::Failed => eprintln!("A push service did not accept a notification"),
            Outcome::Gone => {
                let db = path.to_owned();
                let removed = tokio::task::spawn_blocking(move || {
                    connect(&db)?.execute(
                        "DELETE FROM push_subscriptions WHERE endpoint = ?1",
                        [endpoint],
                    )
                })
                .await;
                if !matches!(removed, Ok(Ok(_))) {
                    eprintln!("Could not remove an expired push subscription");
                }
            }
        }
    }
    count
}

fn build(vapid: &Vapid, delivery: &Delivery) -> Option<http::Request<Vec<u8>>> {
    let key = URL_SAFE_NO_PAD
        .decode(delivery.p256dh.trim_end_matches('='))
        .ok()?;
    let auth = URL_SAFE_NO_PAD
        .decode(delivery.auth.trim_end_matches('='))
        .ok()?;
    let body = serde_json::to_vec(&delivery.payload).ok()?;
    let mut request = WebPushBuilder::new(
        delivery.endpoint.parse().ok()?,
        PublicKey::from_sec1_bytes(&key).ok()?,
        Auth::clone_from_slice(auth.get(..16)?),
    )
    .with_valid_duration(TTL)
    .with_vapid(&vapid.key, &vapid.contact)
    .build(body)
    .ok()?;
    request
        .headers_mut()
        .insert("urgency", http::HeaderValue::from_static("high"));
    Some(request)
}

/// Drains the outbox twice a second for as long as the server runs.
pub async fn run<T: Transport>(path: std::path::PathBuf, vapid: Arc<Vapid>, transport: Arc<T>) {
    let mut ticks = tokio::time::interval(Duration::from_millis(500));
    loop {
        ticks.tick().await;
        // Detached so one slow push service can't hold back the next batch.
        let (path, vapid, transport) = (path.clone(), Arc::clone(&vapid), Arc::clone(&transport));
        tokio::spawn(async move { dispatch(&path, &vapid, &transport).await });
    }
}

/// Real HTTPS transport to FCM/APNs/Mozilla, trusting the bundled WebPKI roots.
pub struct Https(
    hyper_util::client::legacy::Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        http_body_util::Full<bytes::Bytes>,
    >,
);

impl Https {
    pub fn new() -> Self {
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_only()
            .enable_all_versions()
            .build();
        Self(
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(connector),
        )
    }
}

impl Transport for Https {
    async fn send(&self, request: http::Request<Vec<u8>>) -> Outcome {
        let (parts, body) = request.into_parts();
        let request = http::Request::from_parts(parts, http_body_util::Full::from(body));
        match self.0.request(request).await {
            Ok(response) => classify(response.status()),
            Err(_) => Outcome::Failed,
        }
    }
}

fn classify(status: http::StatusCode) -> Outcome {
    match status.as_u16() {
        200..=299 => Outcome::Sent,
        404 | 410 => Outcome::Gone,
        _ => Outcome::Failed,
    }
}

#[cfg(test)]
mod tests;
