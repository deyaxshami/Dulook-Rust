//! Interpretation of the plugin's bar-entry settings (port of the original Service.qml properties).

use std::path::Path;

use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};

use crate::jsutil::{js_number, js_string, js_trim};
use crate::model::Prayer;
use crate::translations::Lang;

pub const DEFAULT_CALENDAR_PATH: &str = "data/prayer-times-2026-nordhorn.csv";
pub const DEFAULT_SOUND_PATH: &str = "adans/adan_01.mp3";

#[derive(Clone, Debug)]
pub struct Settings {
    pub language: Lang,
    pub hadith_language: Lang,
    pub calendar_path: String,
    pub effective_calendar_path: String,
    pub prayer_time_notifications_enabled: bool,
    pub prayer_reminders_enabled: bool,
    pub last_third_reminder_enabled: bool,
    pub sound_enabled: bool,
    pub bypass_dnd: bool,
    pub notify: [bool; 5],
    pub remind: [bool; 5],
    pub sound_path: String,
    pub effective_sound_path: String,
    pub sound_volume: i64,
    pub arabic_ui_font: String,
    pub arabic_hadith_font: String,
}

fn setting<'a>(raw: &'a Map<String, Value>, name: &str, fallback: &'a Value) -> &'a Value {
    match raw.get(name) {
        None | Some(Value::Null) => fallback,
        Some(value) => value,
    }
}

fn string_setting(raw: &Map<String, Value>, name: &str, fallback: &str) -> String {
    match raw.get(name) {
        None | Some(Value::Null) => fallback.to_owned(),
        Some(value) => js_string(value),
    }
}

fn bool_setting(raw: &Map<String, Value>, name: &str, fallback: bool) -> bool {
    match raw.get(name) {
        None | Some(Value::Null) => fallback,
        Some(Value::Bool(value)) => *value,
        Some(value) => match js_string(value).to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => fallback,
        },
    }
}

/// Resolves `file://` URLs, `~`/`$HOME` prefixes, and plugin-relative paths.
pub fn expand_path(value: &str, plugin_dir: &Path, home: &str) -> String {
    let mut path = js_trim(value).to_owned();
    if let Some(rest) = path.strip_prefix("file://") {
        path = percent_decode_str(rest).decode_utf8().map_or_else(|_| rest.to_owned(), |p| p.into_owned());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else if let Some(rest) = path.strip_prefix("$HOME/") {
        format!("{home}/{rest}")
    } else if let Some(rest) = path.strip_prefix("${HOME}/") {
        format!("{home}/{rest}")
    } else if !path.is_empty() && !path.starts_with('/') && !path.contains("://") {
        plugin_dir.join(&path).to_string_lossy().into_owned()
    } else {
        path
    }
}

impl Settings {
    pub fn from_raw(raw: &Map<String, Value>, plugin_dir: &Path, home: &str) -> Settings {
        let language = Lang::from_setting(&string_setting(raw, "language", "en"));
        let hadith_language = Lang::from_setting(&string_setting(raw, "hadithLanguage", language.code()));
        let calendar_path = js_trim(&string_setting(raw, "calendarPath", DEFAULT_CALENDAR_PATH)).to_owned();
        let sound_path = js_trim(&string_setting(raw, "soundPath", DEFAULT_SOUND_PATH)).to_owned();
        let notifications_enabled = bool_setting(raw, "notificationsEnabled", true);
        let notify = Prayer::ALL.map(|p| bool_setting(raw, &format!("notify{}", p.name()), true));
        let remind = Prayer::ALL.map(|p| bool_setting(raw, &format!("remind{}", p.name()), notify[p as usize]));
        let default_volume = Value::from(80);
        let volume = js_number(setting(raw, "soundVolume", &default_volume));
        let sound_volume = if volume.is_nan() { 0 } else { volume.clamp(0.0, 100.0) as i64 };

        Settings {
            language,
            hadith_language,
            effective_calendar_path: expand_path(&calendar_path, plugin_dir, home),
            calendar_path,
            prayer_time_notifications_enabled: bool_setting(raw, "prayerTimeNotificationsEnabled", notifications_enabled),
            prayer_reminders_enabled: bool_setting(raw, "prayerRemindersEnabled", notifications_enabled),
            last_third_reminder_enabled: bool_setting(raw, "lastThirdReminderEnabled", notifications_enabled),
            sound_enabled: bool_setting(raw, "soundEnabled", true),
            bypass_dnd: bool_setting(raw, "bypassDnd", false),
            notify,
            remind,
            effective_sound_path: expand_path(&sound_path, plugin_dir, home),
            sound_path,
            sound_volume,
            arabic_ui_font: string_setting(raw, "arabicUiFont", "Noto Sans Arabic"),
            arabic_hadith_font: string_setting(raw, "arabicHadithFont", "Noto Naskh Arabic"),
        }
    }
}
