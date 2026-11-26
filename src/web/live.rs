use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use askama::Template;
use axum::response::{
    IntoResponse, Response, Sse,
    sse::{Event, KeepAlive},
};
use kringkring::i18n::Language;
use tokio::sync::watch;

use crate::{
    orders::{self, Error, QueueSnapshot, RequestCard},
    users::TokenHash,
};

#[derive(Clone)]
pub(super) struct Changes(Arc<LiveState>);

struct LiveState {
    changes: watch::Sender<()>,
    staff_connections: Mutex<HashMap<TokenHash, usize>>,
}

struct StaffConnection {
    changes: Changes,
    token: TokenHash,
}

impl Drop for StaffConnection {
    fn drop(&mut self) {
        let mut staff = self.changes.0.staff_connections.lock().unwrap();
        if let Some(count) = staff.get_mut(&self.token) {
            *count -= 1;
            if *count == 0 {
                staff.remove(&self.token);
            }
        }
        drop(staff);
        self.changes.publish();
    }
}

impl Changes {
    pub fn new(db: PathBuf) -> Self {
        let updates = Arc::new(LiveState {
            changes: watch::channel(()).0,
            staff_connections: Mutex::default(),
        });
        let prune_db = db.clone();
        let prune_state = Arc::downgrade(&updates);
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_secs(86400));
            loop {
                ticks.tick().await;
                let Some(state) = prune_state.upgrade() else {
                    break;
                };
                let db = prune_db.clone();
                match tokio::task::spawn_blocking(move || crate::history::prune_daily(&db)).await {
                    Ok(Ok(removed)) if removed > 0 => {
                        state.changes.send_replace(());
                    }
                    Ok(Ok(_)) => {}
                    _ => eprintln!("Could not prune request history"),
                }
            }
        });
        let weak = Arc::downgrade(&updates);
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(Duration::from_millis(200));
            loop {
                ticks.tick().await;
                let Some(updates) = weak.upgrade() else {
                    break;
                };
                let db = db.clone();
                match tokio::task::spawn_blocking(move || orders::finish_cancellations(&db)).await {
                    Ok(Ok(count)) if count > 0 => {
                        updates.changes.send_replace(());
                    }
                    _ => {}
                }
            }
        });
        Self(updates)
    }

    pub fn publish(&self) {
        self.0.changes.send_replace(());
    }

    pub fn online(&self) -> HashSet<TokenHash> {
        self.0
            .staff_connections
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect()
    }

    fn connect_staff(&self, token: TokenHash) -> StaffConnection {
        *self
            .0
            .staff_connections
            .lock()
            .unwrap()
            .entry(token)
            .or_default() += 1;
        self.publish();
        StaffConnection {
            changes: self.clone(),
            token,
        }
    }
}

#[derive(Template)]
#[template(path = "request_cards.html")]
pub(super) struct Cards {
    lang: Language,
    requests: Vec<RequestCard>,
    duty_paused: Option<bool>,
}

impl Cards {
    pub fn staff(lang: Language, queue: QueueSnapshot) -> Self {
        Self {
            lang,
            requests: queue.requests,
            duty_paused: Some(queue.paused),
        }
    }

    pub fn employee(lang: Language, requests: Vec<RequestCard>) -> Self {
        Self {
            lang,
            requests,
            duty_paused: None,
        }
    }
}

pub(super) fn subscribe(
    db: PathBuf,
    token: TokenHash,
    lang: Language,
    changes: Changes,
    staff: bool,
) -> Response {
    stream(changes, staff.then_some(token), move || {
        let cards = if staff {
            Cards::staff(lang, orders::queue(&db, token, lang)?)
        } else {
            Cards::employee(lang, orders::overview(&db, token, lang)?.requests)
        };
        cards.render().map_err(|_| Error::Database)
    })
}

pub(super) fn stream(
    state: Changes,
    staff_token: Option<TokenHash>,
    render: impl Fn() -> Result<String, Error> + Send + Sync + 'static,
) -> Response {
    let render = Arc::new(render);
    let mut changes = state.0.changes.subscribe();
    let stream = async_stream::stream! {
        let _staff = staff_token.map(|token| state.connect_staff(token));
        let mut previous = None;
        // Also refresh terminal-card expiry and recheck permission without any new writes.
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = timer.tick() => {}
                change = changes.changed() => { if change.is_err() { break; } }
            }
            let render = Arc::clone(&render);
            let loaded = tokio::task::spawn_blocking(move || render()).await;
            match loaded {
                Ok(Ok(html)) => {
                    if previous.as_ref() != Some(&html) {
                        yield Ok::<_, Infallible>(Event::default().event("cards").data(&html));
                        previous = Some(html);
                    }
                }
                Ok(Err(Error::Forbidden)) => {
                    yield Ok(Event::default().event("cards").data(""));
                    yield Ok(Event::default().event("closed").data(""));
                    break;
                }
                _ => break,
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
