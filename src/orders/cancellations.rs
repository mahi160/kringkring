use std::path::Path;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;

use super::{Error, Result, employee};
use crate::{
    catalog::Kind,
    users::{TokenHash, connect},
};
use kringkring::request::{Status, transition};

pub struct Reason {
    pub preset_id: Option<i64>,
    pub other: Option<String>,
}

impl Reason {
    pub(super) fn resolve(
        self,
        db: &Connection,
        kind: Kind,
    ) -> Result<(Option<i64>, String, String)> {
        match (self.preset_id, self.other) {
            (Some(id), None) => {
                let sql = format!(
                    "SELECT name_en, name_bn FROM {} WHERE id = ?1 AND active = 1",
                    kind.table()
                );
                db.query_row(&sql, [id], |row| Ok((Some(id), row.get(0)?, row.get(1)?)))
                    .optional()?
                    .ok_or(Error::Reason)
            }
            (None, Some(text)) => {
                let text = text.trim();
                if text.is_empty()
                    || text.chars().count() > 120
                    || text.chars().any(char::is_control)
                {
                    return Err(Error::Reason);
                }
                Ok((None, text.to_owned(), String::new()))
            }
            _ => Err(Error::Reason),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmployeeAction {
    Cancel,
    Undo,
}

pub fn employee_action(
    path: &Path,
    token: TokenHash,
    id: i64,
    action: EmployeeAction,
    reason: Reason,
) -> Result<()> {
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let actor = employee(&tx, token)?.id;
    let current: String = tx
        .query_row(
            "SELECT status FROM requests WHERE id = ?1 AND employee_id = ?2",
            [id, actor],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(Error::Conflict)?;
    let status = Status::parse(&current).ok_or(Error::Database)?;
    let changed = match action {
        EmployeeAction::Undo => tx.execute(
            "UPDATE requests SET cancel_after_ms = NULL, updated_at = unixepoch()
             WHERE id = ?1 AND status = 'pending' AND cancel_after_ms > round(unixepoch('subsec') * 1000)",
            [id],
        )?,
        EmployeeAction::Cancel => {
            transition(status, Status::Cancelled).ok_or(Error::Conflict)?;
            match status {
                Status::Pending => {
                    // A reason came from the accepted-request picker; don't reinterpret it.
                    if reason.preset_id.is_some() || reason.other.is_some() { return Err(Error::Conflict); }
                    tx.execute(
                        "UPDATE requests SET cancel_after_ms = round(unixepoch('subsec') * 1000) + 3000,
                         updated_at = unixepoch() WHERE id = ?1 AND status = 'pending' AND cancel_after_ms IS NULL",
                        [id],
                    )?
                }
                Status::Accepted => {
                    let (preset, en, bn) = reason.resolve(&tx, Kind::CancelReasons)?;
                    tx.execute(
                        "UPDATE requests SET status = 'cancelled', closed_by = ?2, closed_at = unixepoch(),
                         updated_at = unixepoch(), cancel_reason_id = ?3, reason_en = ?4, reason_bn = ?5
                         WHERE id = ?1 AND status = 'accepted'",
                        params![id, actor, preset, en, bn],
                    )?
                }
                _ => return Err(Error::Conflict),
            }
        }
    };
    if changed != 1 {
        return Err(Error::Conflict);
    }
    if status == Status::Accepted && matches!(action, EmployeeAction::Cancel) {
        crate::push::enqueue(&tx, id, crate::push::Event::Cancelled)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn finish_cancellations(path: &Path) -> rusqlite::Result<usize> {
    let mut db = connect(path)?;
    let due: bool = db.query_row(
        "SELECT EXISTS (SELECT 1 FROM requests WHERE status = 'pending'
         AND cancel_after_ms <= round(unixepoch('subsec') * 1000))",
        [],
        |row| row.get(0),
    )?;
    if !due {
        return Ok(0);
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let cancelled = tx
        .prepare(
            "UPDATE requests SET status = 'cancelled', closed_by = employee_id,
             closed_at = unixepoch(), updated_at = unixepoch(), cancel_after_ms = NULL
             WHERE status = 'pending' AND cancel_after_ms <= round(unixepoch('subsec') * 1000)
             RETURNING id",
        )?
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in &cancelled {
        crate::push::enqueue(&tx, *id, crate::push::Event::Cancelled)?;
    }
    tx.commit()?;
    Ok(cancelled.len())
}
