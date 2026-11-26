pub mod admin;
pub mod setup;
pub mod signup;

use std::{error::Error, path::Path};

use argon2::{
    Argon2,
    password_hash::{self, PasswordHasher, PasswordVerifier},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use kringkring::i18n::Language;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub type TokenHash = [u8; 32];

#[derive(Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Employee,
    Staff,
}

impl Role {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Employee => "employee",
            Self::Staff => "staff",
        }
    }
}

pub struct Principal {
    pub role: Role,
    pub is_admin: bool,
    pub language: Language,
    pub setup_pending: bool,
}

pub(crate) fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

fn valid_pin(pin: &str) -> bool {
    pin.len() == 4 && pin.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_username(username: &str) -> bool {
    !username.is_empty() && username.trim() == username
}

fn hash_pin(pin: &str) -> password_hash::Result<String> {
    Ok(Argon2::default().hash_password(pin.as_bytes())?.to_string())
}

pub fn init_admin(path: &Path, username: &str, pin: &str) -> Result<()> {
    if !valid_username(username) {
        return Err("username must not be empty or have surrounding whitespace".into());
    }
    if !valid_pin(pin) {
        return Err("PIN must be exactly four ASCII digits".into());
    }
    let hash = hash_pin(pin)?;
    let changed = connect(path)?.execute(
        "INSERT INTO users (username, pin_hash, role, is_admin)
         SELECT ?1, ?2, 'employee', 1 WHERE NOT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)",
        params![username, hash],
    )?;
    if changed == 0 {
        return Err("an admin already exists".into());
    }
    Ok(())
}

/// A default location must be active, unless it is the user's current one.
fn allowed_location(db: &Connection, location: i64, user: i64) -> rusqlite::Result<bool> {
    db.query_row(
        "SELECT EXISTS (SELECT 1 FROM locations WHERE id = ?1 AND
         (active = 1 OR id = (SELECT default_location_id FROM users WHERE id = ?2)))",
        [location, user],
        |row| row.get(0),
    )
}

pub struct LoginSession {
    pub token: String,
}

pub fn login(path: &Path, username: &str, pin: &str) -> Result<Option<LoginSession>> {
    if !valid_pin(pin) {
        return Ok(None);
    }
    let mut conn = connect(path)?;
    // Serialize the counter check, verification, and update across concurrent logins.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let user = tx
        .query_row(
            "SELECT id, pin_hash, failed_pins FROM users WHERE username = ?1 AND active = 1",
            [username],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u8>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((id, hash, failures)) = user else {
        return Ok(None);
    };
    if failures == 5 {
        return Ok(None);
    }
    match Argon2::default().verify_password(pin.as_bytes(), hash.as_str()) {
        Ok(()) => {}
        Err(password_hash::Error::PasswordInvalid) => {
            tx.execute(
                "UPDATE users SET failed_pins = failed_pins + 1 WHERE id = ?1",
                [id],
            )?;
            tx.commit()?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    }
    let mut token = [0; 32];
    getrandom::fill(&mut token)?;
    let digest: TokenHash = Sha256::digest(token).into();
    tx.execute("UPDATE users SET failed_pins = 0 WHERE id = ?1", [id])?;
    tx.execute(
        "INSERT INTO sessions (token_hash, user_id) VALUES (?1, ?2)",
        params![digest, id],
    )?;
    tx.commit()?;
    Ok(Some(LoginSession {
        token: URL_SAFE_NO_PAD.encode(token),
    }))
}

pub fn token_hash(encoded: &str) -> Option<TokenHash> {
    if encoded.len() != 43 {
        return None;
    }
    let mut token = [0; 32];
    URL_SAFE_NO_PAD.decode_slice(encoded, &mut token).ok()?;
    Some(Sha256::digest(token).into())
}

pub fn principal(path: &Path, digest: TokenHash) -> Result<Option<Principal>> {
    Ok(connect(path)?
        .query_row(
            "SELECT users.role = 'staff', users.is_admin, users.default_language = 'en', users.must_change_pin FROM sessions
         JOIN users ON users.id = sessions.user_id WHERE sessions.token_hash = ?1 AND users.active = 1",
            [digest],
            |row| {
                Ok(Principal {
                    role: if row.get(0)? {
                        Role::Staff
                    } else {
                        Role::Employee
                    },
                    is_admin: row.get(1)?,
                    language: if row.get(2)? { Language::En } else { Language::Bn },
                    setup_pending: row.get(3)?,
                })
            },
        )
        .optional()?)
}

pub fn logout(path: &Path, digest: TokenHash) -> Result<()> {
    connect(path)?.execute("DELETE FROM sessions WHERE token_hash = ?1", [digest])?;
    Ok(())
}
