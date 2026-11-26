//! Signup requests: anyone may ask for an account with their own name and PIN; an administrator accepts or rejects.

use std::path::Path;

use kringkring::i18n::Language;
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    Role, TokenHash,
    admin::{Error, write},
    connect, hash_pin, valid_pin, valid_username,
};

/// Bounds unauthenticated writes; admins clear the list by accepting or rejecting.
const MAX_PENDING: i64 = 50;

pub enum RequestError {
    Invalid,
    Taken,
    Full,
    Internal,
}

impl From<rusqlite::Error> for RequestError {
    fn from(error: rusqlite::Error) -> Self {
        match Error::from(error) {
            Error::Duplicate => Self::Taken,
            _ => Self::Internal,
        }
    }
}

pub fn request(
    path: &Path,
    username: &str,
    pin: &str,
    language: Language,
) -> Result<(), RequestError> {
    if !valid_username(username) || !valid_pin(pin) {
        return Err(RequestError::Invalid);
    }
    let hash = hash_pin(pin).map_err(|_| RequestError::Internal)?;
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (taken, pending): (bool, i64) = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM users WHERE username = ?1), (SELECT count(*) FROM signup_requests)",
        [username],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if taken {
        return Err(RequestError::Taken);
    }
    if pending >= MAX_PENDING {
        return Err(RequestError::Full);
    }
    tx.execute(
        "INSERT INTO signup_requests (username, pin_hash, default_language) VALUES (?1, ?2, ?3)",
        params![username, hash, language.code()],
    )?;
    tx.commit()?;
    Ok(())
}

pub struct Signup {
    pub id: i64,
    pub username: String,
}

pub fn pending(path: &Path) -> rusqlite::Result<Vec<Signup>> {
    connect(path)?
        .prepare("SELECT id, username FROM signup_requests ORDER BY created_at, id")?
        .query_map([], |row| {
            Ok(Signup {
                id: row.get(0)?,
                username: row.get(1)?,
            })
        })?
        .collect()
}

/// The requester chose their own PIN, so no PIN change is forced.
pub fn accept(path: &Path, actor: TokenHash, id: i64, role: Role) -> Result<(), Error> {
    write(path, actor, |db| {
        let inserted = db.execute(
            "INSERT INTO users (username, pin_hash, role, default_language)
             SELECT username, pin_hash, ?2, default_language FROM signup_requests WHERE id = ?1",
            params![id, role.code()],
        )?;
        if inserted == 0 {
            return Err(Error::NotFound);
        }
        db.execute("DELETE FROM signup_requests WHERE id = ?1", [id])?;
        Ok(())
    })
}

pub fn reject(path: &Path, actor: TokenHash, id: i64) -> Result<(), Error> {
    write(path, actor, |db| {
        db.query_row("SELECT 1 FROM signup_requests WHERE id = ?1", [id], |_| {
            Ok(())
        })
        .optional()?
        .ok_or(Error::NotFound)?;
        db.execute("DELETE FROM signup_requests WHERE id = ?1", [id])?;
        Ok(())
    })
}
