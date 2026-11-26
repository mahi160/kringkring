//! Self-service: replacing an administrator-chosen PIN and picking a default location.

use std::path::Path;

use argon2::{
    Argon2,
    password_hash::{self, PasswordVerifier},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{TokenHash, allowed_location, connect, hash_pin, valid_pin};
use crate::catalog::{Entry, Kind, choices};

pub enum Error {
    Invalid,
    Reused,
    Location,
    Forbidden,
    Internal,
}

impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Internal
    }
}

type Result<T> = std::result::Result<T, Error>;

struct Caller {
    id: i64,
    employee: bool,
    pin_hash: String,
    pending: bool,
}

fn caller(db: &Connection, token: TokenHash) -> Result<Caller> {
    db.query_row(
        "SELECT users.id, users.role = 'employee', users.pin_hash, users.must_change_pin
         FROM sessions JOIN users ON users.id = sessions.user_id
         WHERE token_hash = ?1 AND users.active = 1",
        [token],
        |row| {
            Ok(Caller {
                id: row.get(0)?,
                employee: row.get(1)?,
                pin_hash: row.get(2)?,
                pending: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or(Error::Forbidden)
}

pub struct Settings {
    pub pending: bool,
    pub employee: bool,
    pub locations: Vec<Entry>,
    pub default_location: Option<i64>,
}

impl Settings {
    pub fn is_default(&self, id: &i64) -> bool {
        self.default_location == Some(*id)
    }
}

pub fn settings(path: &Path, token: TokenHash) -> Result<Settings> {
    let mut db = connect(path)?;
    let tx = db.transaction()?;
    let caller = caller(&tx, token)?;
    let default_location = tx.query_row(
        "SELECT default_location_id FROM users WHERE id = ?1",
        [caller.id],
        |row| row.get(0),
    )?;
    Ok(Settings {
        pending: caller.pending,
        employee: caller.employee,
        locations: choices(&tx, Kind::Locations, false)?,
        default_location,
    })
}

fn set_location(db: &Connection, caller: &Caller, location: Option<i64>) -> Result<()> {
    if let Some(location) = location
        && !allowed_location(db, location, caller.id)?
    {
        return Err(Error::Location);
    }
    db.execute(
        "UPDATE users SET default_location_id = ?1 WHERE id = ?2",
        params![location, caller.id],
    )?;
    Ok(())
}

/// Replaces an administrator-chosen PIN; other sessions that knew it are revoked.
pub fn complete(path: &Path, token: TokenHash, pin: &str, location: Option<i64>) -> Result<()> {
    if !valid_pin(pin) {
        return Err(Error::Invalid);
    }
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let caller = caller(&tx, token)?;
    if !caller.pending {
        return Err(Error::Forbidden);
    }
    match Argon2::default().verify_password(pin.as_bytes(), caller.pin_hash.as_str()) {
        Ok(()) => return Err(Error::Reused),
        Err(password_hash::Error::PasswordInvalid) => {}
        Err(_) => return Err(Error::Internal),
    }
    if caller.employee {
        set_location(&tx, &caller, location)?;
    }
    let hash = hash_pin(pin).map_err(|_| Error::Internal)?;
    tx.execute(
        "UPDATE users SET pin_hash = ?1, must_change_pin = 0 WHERE id = ?2",
        params![hash, caller.id],
    )?;
    tx.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND token_hash != ?2",
        params![caller.id, token],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn change_location(path: &Path, token: TokenHash, location: Option<i64>) -> Result<()> {
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let caller = caller(&tx, token)?;
    if !caller.employee || caller.pending {
        return Err(Error::Forbidden);
    }
    set_location(&tx, &caller, location)?;
    tx.commit()?;
    Ok(())
}
