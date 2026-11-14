//! Developer tool for Dulook's data files.
//!
//! `dulook-data validate`       validate data/hadith-fallback.json
//! `dulook-data build`          rebuild it from the pinned upstream editions
//! `dulook-data build --check`  verify the committed file is reproducible
//! `dulook-data calendar ...`   download yearly prayer calendars from salatcalendar.com (anonymous session)

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use chrono::{Datelike, Local};
use dulook::hadith_data::{SELECTION_MANIFEST, build_hadith_fallback, validate_hadith_data};
use dulook::online::{self, OnlineQuery};
use serde_json::Value;

const DATA_FILE: &str = "data/hadith-fallback.json";
const USAGE: &str = "usage: dulook-data validate | build [--check]
       dulook-data calendar --country <CC> --city <CITY> [--zip <ZIP>] [--street <STREET> [--house <NUMBER>]] [--lat <LAT> --lon <LON>] [--from <YEAR>] [--years <N>] [--out <DIR>]";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn curl(edition: &str, url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("/usr/bin/curl")
        .args(["-fLsS", url])
        .output()
        .map_err(|e| format!("Failed to fetch {edition}: {e}"))?;
    if !output.status.success() {
        let code = output.status.code().map_or_else(|| "signal".to_owned(), |c| c.to_string());
        return Err(format!("Failed to fetch {edition}: curl exit {code}"));
    }
    Ok(output.stdout)
}

fn parsed<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value.parse().map_err(|_| format!("{flag}: invalid value {value:?}"))
}

/// Downloads `--years` calendars starting at `--from` (default: this year and the next 10)
/// into `--out` (default: `data/<slug>`).
fn calendar(args: &[&str]) -> Result<ExitCode, String> {
    let mut query = OnlineQuery::default();
    let (mut latitude, mut longitude, mut out) = (None, None, None);
    let mut from = Local::now().year();
    let mut count = online::YEARS_AHEAD + 1;
    let mut flags = args.iter();
    while let Some(&flag) = flags.next() {
        let Some(&value) = flags.next() else { return Err(format!("{flag} needs a value\n{USAGE}")) };
        match flag {
            "--country" => query.country = value.trim().to_uppercase(),
            "--city" => query.city = value.trim().to_owned(),
            "--zip" => query.zip = value.trim().to_owned(),
            "--street" => query.street = value.trim().to_owned(),
            "--house" => query.house_number = value.trim().to_owned(),
            "--lat" => latitude = Some(parsed::<f64>(flag, value)?),
            "--lon" => longitude = Some(parsed::<f64>(flag, value)?),
            "--from" => from = parsed(flag, value)?,
            "--years" => count = parsed(flag, value)?,
            "--out" => out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {flag}\n{USAGE}")),
        }
    }
    if query.country.is_empty() || query.city.is_empty() || count < 1 {
        return Err(USAGE.to_owned());
    }
    query.coordinates = match (latitude, longitude) {
        (Some(latitude), Some(longitude)) => Some((latitude, longitude)),
        (None, None) => None,
        _ => return Err("--lat and --lon go together".to_owned()),
    };

    let slug = online::slug(&query);
    let dir = out.unwrap_or_else(|| root().join("data").join(&slug));
    let years: Vec<i32> = (from..from + count).collect();
    let location = online::download(&query, None, &years, &dir, false, "", |done, total| {
        let file = dir.join(format!("{slug}_{}.csv", years[done - 1]));
        eprintln!("{done}/{total} {}", file.display());
    })
    .map_err(|error| format!("dulook-data: {error}"))?;
    println!(
        "Wrote {} calendars for {} ({:.6}, {:.6}, {}) to {}.",
        years.len(),
        location.city,
        location.latitude,
        location.longitude,
        location.timezone,
        dir.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let ["calendar", rest @ ..] = args.as_slice() {
        return calendar(rest);
    }
    let root = root();
    let manifest = read_json(&root.join(SELECTION_MANIFEST))?;
    let data_path = root.join(DATA_FILE);
    match args.as_slice() {
        ["validate"] => {
            let summary = validate_hadith_data(&read_json(&data_path)?, &manifest)?;
            println!(
                "Validated {} bilingual Hadith records ({} Bukhari, {} Muslim).",
                summary.total, summary.bukhari, summary.muslim
            );
        }
        ["build"] => {
            let (serialized, count) = build_hadith_fallback(&manifest, curl)?;
            std::fs::write(&data_path, serialized).map_err(|e| format!("{}: {e}", data_path.display()))?;
            println!("Wrote {count} records to {DATA_FILE}.");
        }
        ["build", "--check"] => {
            let (serialized, count) = build_hadith_fallback(&manifest, curl)?;
            let current = std::fs::read_to_string(&data_path).map_err(|e| format!("{}: {e}", data_path.display()))?;
            if current != serialized {
                eprintln!("{DATA_FILE} is not reproducible from the pinned selection");
                return Ok(ExitCode::FAILURE);
            }
            println!("Verified deterministic Hadith fallback ({count} records).");
        }
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}
