mod cancellations;

use std::path::Path;

pub use cancellations::{EmployeeAction, Reason, employee_action, finish_cancellations};

use kringkring::{
    i18n::Language,
    request::{Status, transition},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::users::{TokenHash, connect};

pub enum Error {
    Forbidden,
    Invalid,
    Location,
    Limit,
    Conflict,
    Reason,
    NoStaff,
    Paused,
    Database,
}

impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Draft {
    pub item_id: Option<i64>,
    pub custom_text: Option<String>,
    pub location_id: Option<i64>,
    pub modifiers: Vec<i64>,
}

struct Employee {
    id: i64,
    default_location: Option<i64>,
}

fn employee(db: &Connection, token: TokenHash) -> Result<Employee> {
    db.query_row(
        "SELECT users.id, users.default_location_id FROM sessions JOIN users ON users.id = sessions.user_id
         WHERE token_hash = ?1 AND users.role = 'employee' AND users.active = 1",
        [token],
        |row| Ok(Employee { id: row.get(0)?, default_location: row.get(1)? }),
    ).optional()?.ok_or(Error::Forbidden)
}

pub fn send(path: &Path, token: TokenHash, draft: &Draft) -> Result<()> {
    let custom = draft.custom_text.as_deref().map(str::trim);
    match (draft.item_id, custom) {
        (Some(_), None) => {}
        (None, Some(text))
            if !text.is_empty()
                && text.chars().count() <= 80
                && !text.chars().any(char::is_control)
                && draft.modifiers.is_empty() => {}
        _ => return Err(Error::Invalid),
    }

    // The open-count check and insert (including modifiers) must commit together.
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let caller = employee(&tx, token)?;
    if !staff_on_duty(&tx)? {
        return Err(Error::NoStaff);
    }
    let open: i64 = tx.query_row(
        "SELECT count(*) FROM requests WHERE employee_id = ?1 AND status IN ('pending', 'accepted')",
        [caller.id], |row| row.get(0),
    )?;
    if open >= 3 {
        return Err(Error::Limit);
    }
    let location = draft
        .location_id
        .or(caller.default_location)
        .ok_or(Error::Location)?;
    let active: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM locations WHERE id = ?1 AND active = 1)",
        [location],
        |row| row.get(0),
    )?;
    if !active {
        return Err(Error::Location);
    }
    if let Some(item) = draft.item_id {
        let active: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM menu_items WHERE id = ?1 AND active = 1)",
            [item],
            |row| row.get(0),
        )?;
        if !active {
            return Err(Error::Invalid);
        }
    }
    tx.execute(
        "INSERT INTO requests (id, employee_id, item_id, custom_text, location_id)
         VALUES ((SELECT last_id + 1 FROM request_id_sequence), ?1, ?2, ?3, ?4)",
        params![caller.id, draft.item_id, custom, location],
    )?;
    let id = tx.last_insert_rowid();
    for modifier in &draft.modifiers {
        let changed = tx.execute(
            "INSERT INTO request_modifiers (request_id, modifier_id)
             SELECT ?1, modifiers.id FROM modifiers JOIN menu_item_modifiers ON modifiers.id = modifier_id
             WHERE item_id = ?2 AND modifiers.id = ?3 AND modifiers.active = 1 ON CONFLICT DO NOTHING",
            params![id, draft.item_id, modifier],
        )?;
        if changed == 0 {
            return Err(Error::Invalid);
        }
    }
    crate::push::enqueue(&tx, id, crate::push::Event::New)?;
    tx.commit()?;
    Ok(())
}

pub struct RequestCard {
    pub id: i64,
    pub name: String,
    pub location: String,
    pub modifiers: Vec<String>,
    pub status: Status,
    pub requester: String,
    pub accepted_by: Option<String>,
    pub mine: bool,
    pub created_at: i64,
    pub closed_at: Option<i64>,
    pub undo_pending: bool,
    pub reason: String,
}

pub struct Overview {
    pub requests: Vec<RequestCard>,
    pub default_location: Option<i64>,
}

pub const TERMINAL_GRACE_SECONDS: i64 = 5;

pub fn overview(path: &Path, token: TokenHash, lang: Language) -> Result<Overview> {
    let mut db = connect(path)?;
    let tx = db.transaction()?;
    let caller = employee(&tx, token)?;
    Ok(Overview {
        requests: cards(&tx, lang, Some(caller.id), None, false)?,
        default_location: caller.default_location,
    })
}

struct Staff {
    id: i64,
    paused: bool,
}

fn staff(db: &Connection, token: TokenHash) -> Result<Staff> {
    db.query_row(
        "SELECT users.id, users.staff_paused FROM sessions JOIN users ON users.id = sessions.user_id
         WHERE token_hash = ?1 AND users.role = 'staff' AND users.active = 1",
        [token],
        |row| Ok(Staff { id: row.get(0)?, paused: row.get(1)? }),
    ).optional()?.ok_or(Error::Forbidden)
}

fn staff_on_duty(db: &Connection) -> rusqlite::Result<bool> {
    db.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM sessions JOIN users ON users.id = sessions.user_id
             WHERE users.role = 'staff' AND users.active = 1 AND users.staff_paused = 0
         )",
        [],
        |row| row.get(0),
    )
}

pub fn set_paused(path: &Path, token: TokenHash, paused: bool) -> Result<()> {
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let staff = staff(&tx, token)?;
    tx.execute(
        "UPDATE users SET staff_paused = ?1 WHERE id = ?2",
        params![paused, staff.id],
    )?;
    tx.commit()?;
    Ok(())
}

pub struct QueueSnapshot {
    pub requests: Vec<RequestCard>,
    pub paused: bool,
}

pub fn queue(path: &Path, token: TokenHash, lang: Language) -> Result<QueueSnapshot> {
    let mut db = connect(path)?;
    let tx = db.transaction()?;
    let caller = staff(&tx, token)?;
    Ok(QueueSnapshot {
        requests: cards(&tx, lang, None, Some(caller.id), caller.paused)?,
        paused: caller.paused,
    })
}

fn cards(
    db: &Connection,
    lang: Language,
    employee: Option<i64>,
    viewer: Option<i64>,
    paused: bool,
) -> Result<Vec<RequestCard>> {
    let mut statement = db.prepare(
        "SELECT requests.id, COALESCE(custom_text, menu_items.name_en), COALESCE(custom_text, menu_items.name_bn),
         locations.name_en, locations.name_bn, status, requester.username, accepter.username,
         COALESCE(accepted_by = ?2, 0), requests.created_at, requests.closed_at,
         cancel_after_ms IS NOT NULL, reason_en, reason_bn
         FROM requests LEFT JOIN menu_items ON menu_items.id = item_id
         JOIN locations ON locations.id = location_id JOIN users AS requester ON requester.id = employee_id
         LEFT JOIN users AS accepter ON accepter.id = accepted_by
         WHERE (?1 IS NULL OR employee_id = ?1) AND (NOT ?4 OR accepted_by = ?2)
           AND (status IN ('pending', 'accepted') OR closed_at > unixepoch() - ?3)
         ORDER BY CASE WHEN ?1 IS NULL THEN requests.created_at ELSE -requests.created_at END,
                  CASE WHEN ?1 IS NULL THEN requests.id ELSE -requests.id END",
    )?;
    let requests = statement.query_map(
        params![employee, viewer, TERMINAL_GRACE_SECONDS, paused],
        |row| {
            let name_en: String = row.get(1)?;
            let name_bn: String = row.get(2)?;
            let location_en: String = row.get(3)?;
            let location_bn: String = row.get(4)?;
            let status: String = row.get(5)?;
            Ok(RequestCard {
                id: row.get(0)?,
                name: lang.bilingual(&name_en, &name_bn).to_owned(),
                location: lang.bilingual(&location_en, &location_bn).to_owned(),
                modifiers: Vec::new(),
                status: Status::parse(&status).ok_or(rusqlite::Error::InvalidQuery)?,
                requester: row.get(6)?,
                accepted_by: row.get(7)?,
                mine: row.get(8)?,
                created_at: row.get(9)?,
                closed_at: row.get(10)?,
                undo_pending: row.get(11)?,
                reason: lang
                    .bilingual(&row.get::<_, String>(12)?, &row.get::<_, String>(13)?)
                    .to_owned(),
            })
        },
    )?;
    let mut links = modifier_query(db)?;
    Ok(requests
        .map(|row| {
            let mut request = row?;
            request.modifiers = read_modifiers(&mut links, request.id, lang)?;
            Ok(request)
        })
        .collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn modifier_query(db: &Connection) -> rusqlite::Result<rusqlite::Statement<'_>> {
    db.prepare(
        "SELECT name_en, name_bn FROM request_modifiers JOIN modifiers ON modifiers.id = modifier_id
         WHERE request_id = ?1 ORDER BY sort_order, modifiers.id",
    )
}

pub(crate) fn read_modifiers(
    query: &mut rusqlite::Statement<'_>,
    id: i64,
    lang: Language,
) -> rusqlite::Result<Vec<String>> {
    query
        .query_map([id], |row| {
            let en: String = row.get(0)?;
            let bn: String = row.get(1)?;
            Ok(lang.bilingual(&en, &bn).to_owned())
        })?
        .collect()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StaffAction {
    Accept,
    Reject,
    Done,
}

pub fn staff_action(
    path: &Path,
    token: TokenHash,
    id: i64,
    action: StaffAction,
    reason: Reason,
) -> Result<Status> {
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let caller = staff(&tx, token)?;
    if caller.paused && !matches!(action, StaffAction::Done) {
        return Err(Error::Paused);
    }
    let actor = caller.id;
    let (current, accepter): (String, Option<i64>) = tx
        .query_row(
            "SELECT status, accepted_by FROM requests WHERE id = ?1 AND cancel_after_ms IS NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(Error::Conflict)?;
    let from = Status::parse(&current).ok_or(Error::Database)?;
    let to = match action {
        StaffAction::Accept => Status::Accepted,
        StaffAction::Reject => Status::Rejected,
        StaffAction::Done => Status::Done,
    };
    transition(from, to).ok_or(Error::Conflict)?;
    if matches!(action, StaffAction::Done) && accepter != Some(actor) {
        return Err(Error::Conflict);
    }
    let changed = match action {
        StaffAction::Accept => tx.execute(
            "UPDATE requests SET status = 'accepted', accepted_by = ?2, accepted_at = unixepoch(),
             updated_at = unixepoch() WHERE id = ?1 AND status = 'pending' AND cancel_after_ms IS NULL",
            [id, actor],
        )?,
        StaffAction::Reject => {
            let (preset, en, bn) = reason.resolve(&tx, crate::catalog::Kind::RejectReasons)?;
            tx.execute(
                "UPDATE requests SET status = 'rejected', closed_by = ?2, closed_at = unixepoch(),
                 updated_at = unixepoch(), reject_reason_id = ?3, reason_en = ?4, reason_bn = ?5
                 WHERE id = ?1 AND status = 'pending' AND cancel_after_ms IS NULL",
                params![id, actor, preset, en, bn],
            )?
        }
        StaffAction::Done => tx.execute(
            "UPDATE requests SET status = 'done', closed_by = ?2, closed_at = unixepoch(),
             updated_at = unixepoch() WHERE id = ?1 AND status = 'accepted' AND accepted_by = ?2",
            [id, actor],
        )?,
    };
    if changed != 1 {
        return Err(Error::Conflict);
    }
    let event = match action {
        StaffAction::Accept => crate::push::Event::Accepted,
        StaffAction::Reject => crate::push::Event::Rejected,
        StaffAction::Done => crate::push::Event::Done,
    };
    crate::push::enqueue(&tx, id, event)?;
    tx.commit()?;
    Ok(to)
}
