//! Line protocol between the QML bridge (`Service.qml`) and the `dulook` process.
//!
//! stdin: one JSON command object per line (`{"cmd": "..."}`).
//! stdout: one JSON message per line, `{"type": "state", ...}` or `{"type": "tick", ...}`.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::service::Hadith;

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Command {
    Settings {
        #[serde(default)]
        settings: Map<String, Value>,
    },
    Dnd {
        value: bool,
    },
    ReloadCalendar,
    RefreshClock,
    NextHadith,
    PreviousHadith,
    ShowFallback,
    RequestHadith,
    TestNotification,
    TestSound,
    StopSound,
    StopPrayerSound {
        key: String,
    },
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    serde_json::from_str(line).map_err(|e| format!("{e}: {line}"))
}

pub enum Event {
    Command(Command),
    StdinClosed,
    HadithFinished(Result<Hadith, String>),
}
