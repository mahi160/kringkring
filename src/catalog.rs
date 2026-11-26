use std::path::Path;

use kringkring::i18n::Language;
use rusqlite::{Connection, params};
use serde::Deserialize;

use crate::users::{
    TokenHash,
    admin::{self, Error},
    connect,
};

pub const COLORS: &[&str] = &[
    "blue", "brick", "teal", "ochre", "slate", "violet", "magenta", "denim", "green", "red",
];
pub const ICONS: &[&str] = &["tea", "coffee", "water", "snack"];

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Menu,
    Modifiers,
    Locations,
    RejectReasons,
    CancelReasons,
}

impl Kind {
    pub(crate) fn table(self) -> &'static str {
        match self {
            Self::Menu => "menu_items",
            Self::Modifiers => "modifiers",
            Self::Locations => "locations",
            Self::RejectReasons => "reject_reasons",
            Self::CancelReasons => "cancel_reasons",
        }
    }
}

pub struct Fields {
    pub name_en: String,
    pub name_bn: String,
    pub sort_order: i32,
    pub active: bool,
}

pub struct Entry {
    pub id: i64,
    pub fields: Fields,
}

impl Entry {
    pub fn name(&self, lang: &Language) -> &str {
        lang.bilingual(&self.fields.name_en, &self.fields.name_bn)
    }
}

pub struct Item {
    pub entry: Entry,
    pub color: String,
    pub icon: String,
    pub modifiers: Vec<i64>,
}

pub struct Catalog {
    pub items: Vec<Item>,
    pub modifiers: Vec<Entry>,
    pub locations: Vec<Entry>,
}

fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        fields: Fields {
            name_en: row.get(1)?,
            name_bn: row.get(2)?,
            sort_order: row.get(3)?,
            active: row.get(4)?,
        },
    })
}

pub fn choices(db: &Connection, kind: Kind, active_only: bool) -> rusqlite::Result<Vec<Entry>> {
    let query = format!(
        "SELECT id, name_en, name_bn, sort_order, active FROM {} WHERE active OR NOT ?1 ORDER BY sort_order, id",
        kind.table()
    );
    db.prepare(&query)?
        .query_map([active_only], entry)?
        .collect()
}

pub fn load(path: &Path, active_only: bool) -> rusqlite::Result<Catalog> {
    let mut db = connect(path)?;
    let tx = db.transaction()?;
    let mut items = tx
        .prepare(
            "SELECT id, name_en, name_bn, sort_order, active, color, icon
         FROM menu_items WHERE active OR NOT ?1 ORDER BY sort_order, id",
        )?
        .query_map([active_only], |row| {
            Ok(Item {
                entry: entry(row)?,
                color: row.get(5)?,
                icon: row.get(6)?,
                modifiers: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let modifiers = choices(&tx, Kind::Modifiers, active_only)?;
    let locations = choices(&tx, Kind::Locations, active_only)?;
    let mut links = tx.prepare(
        "SELECT modifier_id FROM menu_item_modifiers
         JOIN modifiers ON modifiers.id = modifier_id
         WHERE item_id = ?1 AND (modifiers.active OR NOT ?2)",
    )?;
    for item in &mut items {
        item.modifiers = links
            .query_map(params![item.entry.id, active_only], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(Catalog {
        items,
        modifiers,
        locations,
    })
}

fn validate(fields: &Fields) -> Result<(), Error> {
    if fields.name_en.trim().is_empty() && fields.name_bn.trim().is_empty() {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}

pub fn save_choice(
    path: &Path,
    actor: TokenHash,
    kind: Kind,
    id: Option<i64>,
    fields: Fields,
) -> Result<(), Error> {
    validate(&fields)?;
    let table = match kind {
        Kind::Menu => return Err(Error::Invalid),
        _ => kind.table(),
    };
    admin::write(path, actor, |db| {
        match id {
            Some(id) => {
                let changed = db.execute(
                    &format!("UPDATE {table} SET name_en = ?1, name_bn = ?2, sort_order = ?3, active = ?4 WHERE id = ?5"),
                    params![fields.name_en.trim(), fields.name_bn.trim(), fields.sort_order, fields.active, id],
                )?;
                if changed == 0 {
                    return Err(Error::NotFound);
                }
            }
            None => {
                db.execute(&format!("INSERT INTO {table} (name_en, name_bn, sort_order, active) VALUES (?1, ?2, ?3, ?4)"),
                    params![fields.name_en.trim(), fields.name_bn.trim(), fields.sort_order, fields.active])?;
            }
        }
        Ok(())
    })
}

pub struct ItemFields {
    pub fields: Fields,
    pub color: String,
    pub icon: String,
    pub modifiers: Vec<i64>,
}

pub fn save_item(
    path: &Path,
    actor: TokenHash,
    id: Option<i64>,
    item: ItemFields,
) -> Result<(), Error> {
    validate(&item.fields)?;
    if !COLORS.contains(&item.color.as_str()) || !ICONS.contains(&item.icon.as_str()) {
        return Err(Error::Invalid);
    }
    let f = item.fields;
    admin::write(path, actor, |db| {
        let id = match id {
            Some(id) => {
                let changed = db.execute(
                    "UPDATE menu_items SET name_en = ?1, name_bn = ?2, sort_order = ?3, active = ?4, color = ?5, icon = ?6 WHERE id = ?7",
                    params![f.name_en.trim(), f.name_bn.trim(), f.sort_order, f.active, item.color, item.icon, id],
                )?;
                if changed == 0 {
                    return Err(Error::NotFound);
                }
                id
            }
            None => {
                db.execute("INSERT INTO menu_items (name_en, name_bn, sort_order, active, color, icon) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![f.name_en.trim(), f.name_bn.trim(), f.sort_order, f.active, item.color, item.icon])?;
                db.last_insert_rowid()
            }
        };
        db.execute("DELETE FROM menu_item_modifiers WHERE item_id = ?1", [id])?;
        for modifier in item.modifiers {
            if db.execute("INSERT INTO menu_item_modifiers (item_id, modifier_id) SELECT ?1, id FROM modifiers WHERE id = ?2 ON CONFLICT DO NOTHING", [id, modifier])? == 0 {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    })
}
