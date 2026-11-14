//! OpenStreetMap tiles for the settings map, downloaded with Dulook's User-Agent and cached
//! on disk as the OSM tile usage policy requires.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::online::USER_AGENT;

pub const TILE_URL: &str = "https://tile.openstreetmap.org";
pub const MAX_ZOOM: u32 = 19;
/// Tiles are kept at least a week (OSM policy) before they are downloaded again.
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// Most tiles requested at once; a panel-sized map needs about 12.
pub const MAX_TILES: usize = 64;

/// `$XDG_CACHE_HOME/dulook/tiles`, or `~/.cache/dulook/tiles`.
pub fn cache_root(home: &str) -> PathBuf {
    match std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir.join("dulook/tiles"),
        _ => Path::new(home).join(".cache/dulook/tiles"),
    }
}

/// A tile key `z/x/y` inside the map at zoom `z`.
pub fn valid_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('/').collect();
    let number = |part: &str| (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<u32>().ok()).flatten();
    match parts.as_slice() {
        [z, x, y] => match (number(z), number(x), number(y)) {
            (Some(z), Some(x), Some(y)) => z <= MAX_ZOOM && x < 1 << z && y < 1 << z,
            _ => false,
        },
        _ => false,
    }
}

pub fn tile_path(root: &Path, key: &str) -> PathBuf {
    root.join(format!("{key}.png"))
}

fn part_path(root: &Path, key: &str) -> PathBuf {
    root.join(format!("{key}.png.part"))
}

/// A cached tile that is younger than the policy's minimum cache time.
pub fn is_fresh(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| {
        meta.len() > 0 && meta.modified().ok().and_then(|time| time.elapsed().ok()).is_some_and(|age| age < MAX_AGE)
    })
}

/// Downloads `keys` into `root` with one curl process (at most 2 connections, as OSM asks).
/// Each tile is written to a `.part` file and renamed when complete.
pub fn fetch(root: &Path, keys: &[String]) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/curl");
    command
        .args(["-sS", "-f", "--max-time", "20", "--parallel", "--parallel-max", "2", "--create-dirs", "-A", USER_AGENT])
        .args(["-w", "%{exitcode} %{filename_effective}\n"]);
    for key in keys {
        command.arg("-o").arg(part_path(root, key)).arg(format!("{TILE_URL}/{key}.png"));
    }
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    let mut done = 0;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some(("0", file)) = line.split_once(' ') else { continue };
        let Some(key) = keys.iter().find(|key| part_path(root, key).as_os_str() == file) else { continue };
        if fs::rename(file, tile_path(root, key)).is_ok() {
            done += 1;
        }
    }
    for key in keys {
        let _ = fs::remove_file(part_path(root, key));
    }
    match keys.len() - done {
        0 => Ok(()),
        failed => Err(format!("{failed} of {} map tiles failed", keys.len())),
    }
}
