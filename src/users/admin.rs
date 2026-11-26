use std::path::Path;

use kringkring::i18n::Language;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, ffi, params};

use super::{Role, TokenHash, connect, hash_pin, valid_pin, valid_username};

pub enum Error {
    Invalid,
    Duplicate,
    LastAdmin,
    Location,
    NotFound,
    Forbidden,
    Internal,
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        if matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.extended_code == ffi::SQLITE_CONSTRAINT_UNIQUE)
        {
            Self::Duplicate
        } else {
            Self::Internal
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Profile {
    pub username: String,
    pub role: Role,
    pub language: Language,
    pub is_admin: bool,
    pub default_location: Option<i64>,
}

pub struct User {
    pub id: i64,
    pub profile: Profile,
    pub active: bool,
    pub locked: bool,
}

impl User {
    pub fn has_default_location(&self, id: &i64) -> bool {
        self.profile.default_location == Some(*id)
    }
}

pub enum Change {
    Edit(Profile),
    ResetPin(String),
    Unlock,
    Revoke,
    Deactivate,
}

pub fn list(path: &Path) -> Result<Vec<User>> {
    let db = connect(path)?;
    let mut statement = db.prepare(
        "SELECT id, username, role = 'staff', default_language = 'en', is_admin,
         active, failed_pins = 5, default_location_id FROM users ORDER BY active DESC, username COLLATE NOCASE, id",
    )?;
    let users = statement
        .query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                profile: Profile {
                    username: row.get(1)?,
                    role: if row.get(2)? {
                        Role::Staff
                    } else {
                        Role::Employee
                    },
                    language: if row.get(3)? {
                        Language::En
                    } else {
                        Language::Bn
                    },
                    is_admin: row.get(4)?,
                    default_location: row.get(7)?,
                },
                active: row.get(5)?,
                locked: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(users)
}

pub(crate) fn write<T>(
    path: &Path,
    actor: TokenHash,
    apply: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    let mut db = connect(path)?;
    // Recheck the acting session and serialize the last-admin check with all writes.
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let allowed: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM sessions JOIN users ON users.id = sessions.user_id
         WHERE token_hash = ?1 AND users.active = 1 AND users.is_admin = 1)",
        [actor],
        |row| row.get(0),
    )?;
    if !allowed {
        return Err(Error::Forbidden);
    }
    let result = apply(&tx)?;
    tx.commit()?;
    Ok(result)
}

fn new_hash(pin: &str) -> Result<String> {
    if !valid_pin(pin) {
        return Err(Error::Invalid);
    }
    hash_pin(pin).map_err(|_| Error::Internal)
}

pub fn create(path: &Path, actor: TokenHash, profile: Profile, pin: &str) -> Result<()> {
    if !valid_username(&profile.username) {
        return Err(Error::Invalid);
    }
    let hash = new_hash(pin)?;
    write(path, actor, |db| {
        db.execute(
            "INSERT INTO users (username, pin_hash, role, is_admin, default_language)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile.username,
                hash,
                profile.role.code(),
                profile.is_admin,
                profile.language.code()
            ],
        )?;
        Ok(())
    })
}

pub fn apply(path: &Path, actor: TokenHash, id: i64, change: Change) -> Result<bool> {
    write(path, actor, |db| {
        let is_admin: bool = db
            .query_row(
                "SELECT is_admin FROM users WHERE id = ?1 AND active = 1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        let removes_admin = match &change {
            Change::Edit(profile) => !profile.is_admin,
            Change::Deactivate => true,
            _ => false,
        };
        if is_admin && removes_admin {
            let another: bool = db.query_row(
                "SELECT EXISTS (SELECT 1 FROM users WHERE id != ?1 AND active = 1 AND is_admin = 1)",
                [id], |row| row.get(0),
            )?;
            if !another {
                return Err(Error::LastAdmin);
            }
        }
        match change {
            Change::Edit(profile) => {
                if !valid_username(&profile.username) {
                    return Err(Error::Invalid);
                }
                let location = if profile.role == Role::Employee {
                    profile.default_location
                } else {
                    None
                };
                if let Some(location) = location {
                    let allowed: bool = db.query_row(
                        "SELECT EXISTS (SELECT 1 FROM locations WHERE id = ?1 AND
                         (active = 1 OR id = (SELECT default_location_id FROM users WHERE id = ?2)))",
                        [location, id], |row| row.get(0),
                    )?;
                    if !allowed {
                        return Err(Error::Location);
                    }
                }
                db.execute(
                    "UPDATE users SET username = ?1, role = ?2, is_admin = ?3, default_language = ?4, default_location_id = ?5 WHERE id = ?6",
                    params![profile.username, profile.role.code(), profile.is_admin, profile.language.code(), location, id],
                )?;
            }
            Change::ResetPin(pin) => {
                let hash = new_hash(&pin)?;
                db.execute(
                    "UPDATE users SET pin_hash = ?1, failed_pins = 0 WHERE id = ?2",
                    params![hash, id],
                )?;
                revoke_id(db, id)?;
            }
            Change::Unlock => {
                db.execute("UPDATE users SET failed_pins = 0 WHERE id = ?1", [id])?;
            }
            Change::Revoke => revoke_id(db, id)?,
            Change::Deactivate => {
                db.execute("UPDATE users SET active = 0 WHERE id = ?1", [id])?;
                revoke_id(db, id)?;
            }
        }
        // The acting admin may have just revoked their own session or changed their role.
        db.query_row(
            "SELECT EXISTS (SELECT 1 FROM sessions JOIN users ON users.id = sessions.user_id
             WHERE token_hash = ?1 AND users.active = 1 AND users.is_admin = 1)",
            [actor],
            |row| row.get(0),
        )
        .map_err(Error::from)
    })
}

pub fn revoke(path: &Path, actor: TokenHash, username: &str) -> Result<()> {
    write(path, actor, |db| {
        if let Some(id) = db
            .query_row(
                "SELECT id FROM users WHERE username = ?1",
                [username],
                |row| row.get(0),
            )
            .optional()?
        {
            revoke_id(db, id)?;
        }
        Ok(())
    })
}

fn revoke_id(db: &Connection, id: i64) -> Result<()> {
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [id])?;
    Ok(())
}
