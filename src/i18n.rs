mod bn;
mod en;

pub const BRAND: &str = "ক্রিং ক্রিং";

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Bn,
    En,
}

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "bn" => Some(Self::Bn),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Bn => "bn",
            Self::En => "en",
        }
    }

    pub fn brand(self) -> &'static str {
        BRAND
    }

    pub fn other(self) -> Self {
        match self {
            Self::Bn => Self::En,
            Self::En => Self::Bn,
        }
    }

    pub fn catalog(self) -> &'static Catalog {
        match self {
            Self::Bn => &bn::CATALOG,
            Self::En => &en::CATALOG,
        }
    }

    pub fn status_label(self, status: &crate::request::Status) -> &'static str {
        use crate::request::Status;
        let t = self.catalog();
        match status {
            Status::Pending => t.request_pending,
            Status::Accepted => t.request_accepted,
            Status::Done => t.delivered,
            Status::Rejected => t.request_rejected,
            Status::Cancelled => t.cancelled,
            Status::Expired => t.request_expired,
        }
    }

    pub fn digits(self, value: &str) -> std::borrow::Cow<'_, str> {
        match self {
            Self::En => value.into(),
            Self::Bn => value
                .chars()
                .map(|ch| match ch {
                    '0'..='9' => ['০', '১', '২', '৩', '৪', '৫', '৬', '৭', '৮', '৯']
                        [ch as usize - '0' as usize],
                    _ => ch,
                })
                .collect::<String>()
                .into(),
        }
    }

    pub fn bilingual<'a>(self, en: &'a str, bn: &'a str) -> &'a str {
        let (first, second) = match self {
            Self::En => (en, bn),
            Self::Bn => (bn, en),
        };
        if first.trim().is_empty() {
            second
        } else {
            first
        }
    }
}

#[derive(serde::Serialize)]
pub struct Catalog {
    pub skip: &'static str,
    pub theme: &'static str,
    pub mute_sounds: &'static str,
    pub switch_language: &'static str,
    pub switch_title: &'static str,
    pub login: &'static str,
    pub login_error: &'static str,
    pub choose_login_name: &'static str,
    pub login_search: &'static str,
    pub login_no_matches: &'static str,
    pub login_no_names: &'static str,
    pub login_change_user: &'static str,
    pub login_clear: &'static str,
    pub login_backspace: &'static str,
    pub username: &'static str,
    pub pin: &'static str,
    pub account: &'static str,
    pub signed_in: &'static str,
    pub sign_out: &'static str,
    pub health: &'static str,
    pub database_ok: &'static str,
    pub database_down: &'static str,
    pub tea: &'static str,
    pub coffee: &'static str,
    pub water: &'static str,
    pub snacks: &'static str,
    pub locations: &'static str,
    pub staff_queue: &'static str,
    pub delivered: &'static str,
    pub cancel: &'static str,
    pub cancelled: &'static str,
    pub actions: &'static str,
    pub unavailable: &'static str,
    pub request_status: &'static str,
    pub manage_users: &'static str,
    pub users_title: &'static str,
    pub users_intro: &'static str,
    pub add_user: &'static str,
    pub user_directory: &'static str,
    pub user_empty: &'static str,
    pub employee: &'static str,
    pub staff: &'static str,
    pub role: &'static str,
    pub administrator: &'static str,
    pub admin_access: &'static str,
    pub default_language: &'static str,
    pub bengali: &'static str,
    pub english: &'static str,
    pub user_active: &'static str,
    pub user_inactive: &'static str,
    pub user_locked: &'static str,
    pub edit_profile: &'static str,
    pub save_changes: &'static str,
    pub reset_pin: &'static str,
    pub reset_notice: &'static str,
    pub unlock: &'static str,
    pub revoke_sessions: &'static str,
    pub deactivate: &'static str,
    pub deactivate_notice: &'static str,
    pub account_access: &'static str,
    pub back_to_users: &'static str,
    pub user_invalid: &'static str,
    pub username_taken: &'static str,
    pub last_admin: &'static str,
    pub user_not_found: &'static str,
    pub admin_required: &'static str,
    pub user_unavailable: &'static str,
    pub manage_catalog: &'static str,
    pub catalog_title: &'static str,
    pub catalog_intro: &'static str,
    pub menu_title: &'static str,
    pub menu_items: &'static str,
    pub modifiers: &'static str,
    pub menu_preview: &'static str,
    pub add_item: &'static str,
    pub add_modifier: &'static str,
    pub add_location: &'static str,
    pub name_en: &'static str,
    pub name_bn: &'static str,
    pub sort_order: &'static str,
    pub color: &'static str,
    pub tile_icon: &'static str,
    pub default_location: &'static str,
    pub no_default_location: &'static str,
    pub location_invalid: &'static str,
    pub catalog_invalid: &'static str,
    pub catalog_not_found: &'static str,
    pub catalog_unavailable: &'static str,
    pub catalog_empty: &'static str,
    pub blue: &'static str,
    pub brick: &'static str,
    pub teal: &'static str,
    pub ochre: &'static str,
    pub slate: &'static str,
    pub violet: &'static str,
    pub magenta: &'static str,
    pub denim: &'static str,
    pub green: &'static str,
    pub red: &'static str,
    pub request_custom: &'static str,
    pub request_custom_label: &'static str,
    pub request_send: &'static str,
    pub request_open: &'static str,
    pub request_none: &'static str,
    pub request_pending: &'static str,
    pub request_accepted: &'static str,
    pub request_rejected: &'static str,
    pub request_expired: &'static str,
    pub request_invalid: &'static str,
    pub request_location_required: &'static str,
    pub request_no_locations: &'static str,
    pub request_limit: &'static str,
    pub request_unavailable: &'static str,
    pub queue_oldest: &'static str,
    pub queue_empty: &'static str,
    pub queue_requested_by: &'static str,
    pub queue_accepted_by: &'static str,
    pub queue_accept: &'static str,
    pub queue_reject: &'static str,
    pub queue_done: &'static str,
    pub queue_stale: &'static str,
    pub reject_reasons: &'static str,
    pub cancel_reasons: &'static str,
    pub add_reason: &'static str,
    pub choose_reason: &'static str,
    pub other_reason: &'static str,
    pub reason_label: &'static str,
    pub reason_invalid: &'static str,
    pub cancel_request: &'static str,
    pub cancel_scheduled: &'static str,
    pub cancel_undo: &'static str,
    pub request_changed: &'static str,
    pub duty_on: &'static str,
    pub duty_pause: &'static str,
    pub duty_resume: &'static str,
    pub duty_paused: &'static str,
    pub duty_paused_note: &'static str,
    pub no_staff_available: &'static str,
    pub admin_history: &'static str,
    pub admin_status: &'static str,
    pub history_window: &'static str,
    pub history_invalid: &'static str,
    pub history_empty: &'static str,
    pub history_all: &'static str,
    pub history_from: &'static str,
    pub history_to: &'static str,
    pub history_filter: &'static str,
    pub history_previous: &'static str,
    pub history_next: &'static str,
    pub history_created: &'static str,
    pub history_updated: &'static str,
    pub history_accepted: &'static str,
    pub history_closed: &'static str,
    pub history_closed_by: &'static str,
    pub history_reason: &'static str,
    pub history_id: &'static str,
    pub status_intro: &'static str,
    pub status_none: &'static str,
    pub status_off_duty: &'static str,
    pub status_online: &'static str,
    pub status_offline: &'static str,
    pub push_new: &'static str,
    pub push_enable: &'static str,
    pub push_blocked: &'static str,
}

impl Catalog {
    pub fn color_name(&self, code: &str) -> &str {
        match code {
            "blue" => self.blue,
            "brick" => self.brick,
            "teal" => self.teal,
            "ochre" => self.ochre,
            "slate" => self.slate,
            "violet" => self.violet,
            "magenta" => self.magenta,
            "denim" => self.denim,
            "green" => self.green,
            "red" => self.red,
            _ => self.unavailable,
        }
    }

    pub fn icon_name(&self, code: &str) -> &str {
        match code {
            "tea" => self.tea,
            "coffee" => self.coffee,
            "water" => self.water,
            "snack" => self.snacks,
            _ => self.unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language::{Bn, En};

    #[test]
    fn both_catalogs_supply_the_same_nonempty_keys() {
        let en = serde_json::to_value(En.catalog()).unwrap();
        let bn = serde_json::to_value(Bn.catalog()).unwrap();
        let en = en.as_object().unwrap();
        let bn = bn.as_object().unwrap();
        assert!(en.keys().eq(bn.keys()));
        assert!(
            en.values()
                .chain(bn.values())
                .all(|s| !s.as_str().unwrap().trim().is_empty())
        );
    }

    #[test]
    fn formats_bengali_digits_without_changing_separators() {
        assert_eq!(Bn.digits("12:05"), "১২:০৫");
        assert_eq!(En.digits("12:05"), "12:05");
    }

    #[test]
    fn picks_preferred_bilingual_content_and_falls_back() {
        assert_eq!(Bn.bilingual("Tea", "চা"), "চা");
        assert_eq!(En.bilingual("Tea", "চা"), "Tea");
        assert_eq!(Bn.bilingual("Tea", " "), "Tea");
        assert_eq!(En.bilingual("", "চা"), "চা");
    }
}
