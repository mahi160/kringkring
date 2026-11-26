use std::path::Path;

use kringkring::{i18n::Language, request::Status};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::Deserialize;

use crate::{orders, users::connect};

const RETENTION: i64 = 30 * 24 * 60 * 60;
const PAGE_SIZE: i64 = 50;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub status: String,
    pub user: String,
    pub from: String,
    pub to: String,
    pub page: u32,
}

pub struct Request {
    pub id: i64,
    pub status: Status,
    pub name: String,
    pub location: String,
    pub employee: String,
    pub modifiers: Vec<String>,
    pub reason: String,
    pub created: String,
    pub updated: String,
    pub accepted: Option<String>,
    pub accepted_by: Option<String>,
    pub closed: Option<String>,
    pub closed_by: Option<String>,
}

pub struct Employee {
    pub id: i64,
    pub name: String,
}

pub struct History {
    pub requests: Vec<Request>,
    pub employees: Vec<Employee>,
    pub user_id: Option<i64>,
    pub has_next: bool,
}

pub enum Error {
    InvalidFilter,
    Database,
}

impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

fn day(db: &Connection, entered: &str) -> Result<Option<i64>, Error> {
    if entered.is_empty() {
        return Ok(None);
    }
    if entered.len() != 10 {
        return Err(Error::InvalidFilter);
    }
    let day: Option<i64> = db.query_row(
        "SELECT CASE WHEN date(?1, '+0 days') = ?1 THEN unixepoch(?1) END",
        [entered],
        |row| row.get(0),
    )?;
    day.map(Some).ok_or(Error::InvalidFilter)
}

pub fn load(path: &Path, filter: &Filters, lang: Language) -> Result<History, Error> {
    let mut db = connect(path)?;
    let tx = db.transaction()?;
    let status = if filter.status.is_empty() {
        None
    } else {
        Some(
            Status::parse(&filter.status)
                .ok_or(Error::InvalidFilter)?
                .code(),
        )
    };
    let user_id = if filter.user.is_empty() {
        None
    } else {
        Some(
            filter
                .user
                .parse::<i64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or(Error::InvalidFilter)?,
        )
    };
    let from = day(&tx, &filter.from)?;
    let to = day(&tx, &filter.to)?;
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(Error::InvalidFilter);
    }
    let cutoff = now(&tx)? - RETENTION;
    let employees = tx
        .prepare(
            "SELECT DISTINCT users.id, username FROM users JOIN requests ON users.id = employee_id
         WHERE created_at >= ?1 ORDER BY username COLLATE NOCASE, users.id",
        )?
        .query_map([cutoff], |row| {
            Ok(Employee {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut query = tx.prepare(
        "SELECT r.id, r.status, COALESCE(r.custom_text, m.name_en), COALESCE(r.custom_text, m.name_bn),
         l.name_en, l.name_bn, employee.username, r.reason_en, r.reason_bn,
         datetime(r.created_at, 'unixepoch'), datetime(r.updated_at, 'unixepoch'),
         datetime(r.accepted_at, 'unixepoch'), accepter.username,
         datetime(r.closed_at, 'unixepoch'), closer.username
         FROM requests r JOIN users employee ON employee.id = r.employee_id
         LEFT JOIN menu_items m ON m.id = r.item_id JOIN locations l ON l.id = r.location_id
         LEFT JOIN users accepter ON accepter.id = r.accepted_by LEFT JOIN users closer ON closer.id = r.closed_by
         WHERE r.created_at >= ?1 AND (?2 IS NULL OR r.status = ?2)
           AND (?3 IS NULL OR r.employee_id = ?3) AND (?4 IS NULL OR r.created_at >= ?4)
           AND (?5 IS NULL OR r.created_at < ?5)
         ORDER BY r.created_at DESC, r.id DESC LIMIT ?6 OFFSET ?7",
    )?;
    let rows = query.query_map(
        params![
            cutoff,
            status,
            user_id,
            from,
            to.map(|day| day + 86400),
            PAGE_SIZE + 1,
            i64::from(filter.page.saturating_sub(1)) * PAGE_SIZE
        ],
        |row| {
            let status: String = row.get(1)?;
            let localized = |en, bn| -> rusqlite::Result<String> {
                Ok(lang
                    .bilingual(&row.get::<_, String>(en)?, &row.get::<_, String>(bn)?)
                    .to_owned())
            };
            Ok(Request {
                id: row.get(0)?,
                status: Status::parse(&status).ok_or(rusqlite::Error::InvalidQuery)?,
                name: localized(2, 3)?,
                location: localized(4, 5)?,
                employee: row.get(6)?,
                reason: localized(7, 8)?,
                created: lang.digits(&row.get::<_, String>(9)?).into_owned(),
                updated: lang.digits(&row.get::<_, String>(10)?).into_owned(),
                accepted: row
                    .get::<_, Option<String>>(11)?
                    .map(|time| lang.digits(&time).into_owned()),
                accepted_by: row.get(12)?,
                closed: row
                    .get::<_, Option<String>>(13)?
                    .map(|time| lang.digits(&time).into_owned()),
                closed_by: row.get(14)?,
                modifiers: Vec::new(),
            })
        },
    )?;
    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_next = rows.len() > PAGE_SIZE as usize;
    rows.truncate(PAGE_SIZE as usize);
    let mut modifiers = orders::modifier_query(&tx)?;
    for request in &mut rows {
        request.modifiers = orders::read_modifiers(&mut modifiers, request.id, lang)?;
    }
    Ok(History {
        requests: rows,
        employees,
        user_id,
        has_next,
    })
}

fn now(db: &Connection) -> rusqlite::Result<i64> {
    db.query_row("SELECT unixepoch()", [], |row| row.get(0))
}

pub fn prune_daily(path: &Path) -> rusqlite::Result<usize> {
    let mut db = connect(path)?;
    let now = now(&db)?;
    prune(&mut db, now)
}

fn prune(db: &mut Connection, now: i64) -> rusqlite::Result<usize> {
    let cutoff = now - RETENTION;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM request_modifiers WHERE request_id IN (SELECT id FROM requests WHERE created_at < ?1)", [cutoff])?;
    let removed = tx.execute("DELETE FROM requests WHERE created_at < ?1", [cutoff])?;
    tx.commit()?;
    Ok(removed)
}

#[cfg(test)]
mod tests;
