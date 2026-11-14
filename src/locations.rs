//! Per-location prayer calendars, one CSV per year:
//! `<root>/<location>/prayer-times-<year>-<location>.csv`, where `<root>` is the bundled
//! `data/` folder or the downloaded calendars folder.

use std::fs;
use std::path::{Path, PathBuf};

/// `location` setting value that selects the `calendarPath` CSV instead of a bundled location.
pub const CUSTOM: &str = "custom";
/// `location` setting value that selects the downloaded country/city calendar.
pub const ONLINE: &str = "online";
pub const DEFAULT_LOCATION: &str = ONLINE;

pub fn calendar_file(root: &Path, location: &str, year: i32) -> PathBuf {
    root.join(location).join(format!("prayer-times-{year}-{location}.csv"))
}

/// Folders under `data/` that hold at least one calendar of their own, sorted by name.
pub fn bundled_locations(plugin_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(plugin_dir.join("data")) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name != CUSTOM && name != ONLINE && has_calendar(&plugin_dir.join("data").join(name), name))
        .collect();
    names.sort();
    names
}

fn has_calendar(dir: &Path, location: &str) -> bool {
    let suffix = format!("-{location}.csv");
    fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .any(|name| name.starts_with("prayer-times-") && name.ends_with(&suffix))
    })
}

/// Display name of a location folder: `3p-services` → `3p Services`.
pub fn label(location: &str) -> String {
    location
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}
