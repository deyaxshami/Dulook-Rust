//! Line protocol between the QML bridge (`Service.qml`) and the `dulook` process.
//!
//! stdin: one JSON command object per line (`{"cmd": "..."}`).
//! stdout: one JSON message per line, `{"type": "state", ...}` or `{"type": "tick", ...}`.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::online::{DownloadError, OnlineLocation, PlaceSuggestion, SuggestField};
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
    /// The user clicked the blinking bar label.
    AcknowledgeAlert,
    #[serde(rename_all = "camelCase")]
    SuggestPlaces {
        field: SuggestField,
        #[serde(default)]
        country: String,
        #[serde(default)]
        city: String,
        #[serde(default)]
        zip: String,
        #[serde(default)]
        street: String,
        #[serde(default)]
        house_number: String,
    },
    ClearPlaceSuggestions,
    /// Reverse-geocodes a point pinned on the map.
    PinPlace {
        latitude: f64,
        longitude: f64,
    },
    /// Map tiles (`z/x/y`) the settings map shows; missing ones are downloaded.
    MapTiles {
        #[serde(default)]
        tiles: Vec<String>,
    },
    /// Downloads the place an online service found; `replace` is the folder being edited
    /// (empty for a new calendar).
    #[serde(rename_all = "camelCase")]
    InstallCalendar {
        #[serde(default)]
        mode: String,
        #[serde(default)]
        country: String,
        #[serde(default)]
        city: String,
        #[serde(default)]
        zip: String,
        #[serde(default)]
        street: String,
        #[serde(default)]
        house_number: String,
        #[serde(default)]
        latitude: String,
        #[serde(default)]
        longitude: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        replace: String,
    },
    /// Gives a downloaded calendar a friendly name (empty clears it).
    RenameCalendar {
        folder: String,
        #[serde(default)]
        name: String,
    },
    DeleteCalendar {
        folder: String,
    },
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    serde_json::from_str(line).map_err(|e| format!("{e}: {line}"))
}

pub enum Event {
    Command(Command),
    StdinClosed,
    HadithFinished(Result<Hadith, String>),
    DownloadProgress { done: usize, total: usize },
    DownloadFinished { slug: String, result: Result<OnlineLocation, DownloadError> },
    PlaceSuggestions { generation: u64, result: Result<Vec<PlaceSuggestion>, String> },
    PinnedPlace { generation: u64, result: Result<Option<PlaceSuggestion>, String> },
    MapTilesFetched { batch: Vec<String>, result: Result<(), String> },
}
