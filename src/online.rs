//! Online location calendars: city suggestions and geocoding through Photon, and the
//! anonymous salatcalendar.com scraper that writes one CSV per year.
//!
//! HTTP goes through `/usr/bin/curl`. salatcalendar runs in a fresh session whose cookie
//! jar lives in a private temporary folder that is removed afterwards; no account is used.

use std::fmt;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Days, NaiveDate};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model;
use crate::service::try_write_atomic;

pub const SITE: &str = "https://www.salatcalendar.com/index.php";
pub const PHOTON: &str = "https://photon.komoot.io/api/";
pub const PHOTON_REVERSE: &str = "https://photon.komoot.io/reverse";
pub const USER_AGENT: &str = concat!("Dulook/", env!("CARGO_PKG_VERSION"), " (+https://github.com/deyaxshami/Dulook-Rust)");
/// Years downloaded after the current one.
pub const YEARS_AHEAD: i32 = 10;
/// Coordinates closer than this to the stored ones count as the same place.
pub const SAME_PLACE_KM: f64 = 5.0;
/// Fixed calculation settings sent to `form/checkform`: manual 12°/12° angles, Dhuhr +5 min,
/// automatic altitude/madhhab/DST, standard refraction, 12-hour output.
pub const CALC_SETTINGS: [(&str, &str); 17] = [
    ("CalcMethodId", "-1"),
    ("FajerAng", "12"),
    ("IshaAng", "12"),
    ("addsub", "-1"),
    ("AddFaj", "0"),
    ("AddSho", "0"),
    ("AddDho", "5"),
    ("AddAsr", "0"),
    ("AddMag", "0"),
    ("AddIsh", "0"),
    ("AltEffect", "auto"),
    ("Refraction_Settings", "Standard"),
    ("temp", "10"),
    ("pres", "1010"),
    ("Aser_Madhhab", "auto"),
    ("Daylight_Saving_Time", "Auto"),
    ("Output_Hour_Format", "12h"),
];
const HEADER: [&str; 9] = ["Day", "Date", "Hijri", "Fajr", "Sunrise", "Dhuhr", "Asr", "Maghrib", "Isha"];
const LOCATION_FILE: &str = "location.json";

static ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap());
static CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<t[dh][^>]*>(.*?)</t[dh]>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// Month abbreviation after the Hijri date: `22/07/1448 (Raj)`.
static HIJRI_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\([^)]*\)$").unwrap());

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OnlineQuery {
    /// ISO 3166-1 alpha-2 code, upper case.
    pub country: String,
    pub city: String,
    pub zip: String,
    pub street: String,
    pub house_number: String,
    /// Latitude/longitude picked from a suggestion or pinned on the map; skips geocoding.
    pub coordinates: Option<(f64, f64)>,
    /// The coordinates were pinned on the map: they win over geocoding the street and name
    /// the folder.
    pub pinned: bool,
}

/// A downloaded location; stored as `location.json` next to its CSV files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineLocation {
    pub country: String,
    pub city: String,
    pub zip: String,
    #[serde(default)]
    pub street: String,
    #[serde(default)]
    pub house_number: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

/// A suggestion for the location fields: a city, a ZIP code, a street, or a house on a street.
/// Empty fields are not part of the suggestion.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSuggestion {
    pub city: String,
    pub zip: String,
    pub street: String,
    pub house_number: String,
    pub country: String,
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Location field the user is typing in, which decides what is suggested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestField {
    City,
    Street,
    House,
}

#[derive(Clone, Debug)]
pub enum DownloadError {
    NotFound,
    Timezone,
    Network(String),
}

impl DownloadError {
    /// Translation key shown to the user.
    pub fn key(&self) -> &'static str {
        match self {
            DownloadError::NotFound => "locationNotFound",
            DownloadError::Timezone => "timezoneUnavailable",
            DownloadError::Network(_) => "downloadFailed",
        }
    }
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::NotFound => f.write_str("city or ZIP code not found"),
            DownloadError::Timezone => f.write_str("system time zone unavailable"),
            DownloadError::Network(detail) => f.write_str(detail),
        }
    }
}

/// `$XDG_DATA_HOME/dulook`, or `~/.local/share/dulook`.
pub fn data_root(home: &str) -> PathBuf {
    match std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => dir.join("dulook"),
        _ => Path::new(home).join(".local/share/dulook"),
    }
}

pub fn calendars_root(data_dir: &Path) -> PathBuf {
    data_dir.join("calendars")
}

/// Folder name of a location, `[Country]_[ZIP]_[City]_[Street]_[HouseNumber]` with the
/// optional parts left out when empty: `DE_48527_Nordhorn`, `DE_48529_Nordhorn_Hauptstraße_1`,
/// `EG_سوهاج`. A location pinned on the map is `[Country]_[ZIP]_[City]_[lat]_[lon]` with
/// 6 decimals instead: `DE_48529_Nordhorn_52.433010_7.069130`. Letters keep their case; other
/// characters inside a part become `-`.
pub fn slug(query: &OnlineQuery) -> String {
    let country = query.country.trim().to_uppercase();
    let pin = query.coordinates.filter(|_| query.pinned);
    let named: &[&str] = if pin.is_some() {
        &[country.as_str(), &query.zip, &query.city]
    } else {
        &[country.as_str(), &query.zip, &query.city, &query.street, &query.house_number]
    };
    let mut parts: Vec<String> = named
        .iter()
        .map(|part| {
            let mapped: String = part.trim().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
            mapped.split('-').filter(|word| !word.is_empty()).collect::<Vec<_>>().join("-")
        })
        .filter(|part| !part.is_empty())
        .collect();
    if let Some((latitude, longitude)) = pin {
        parts.push(format!("{latitude:.6}_{longitude:.6}"));
    }
    parts.join("_")
}

/// One year of a location's calendar: `<root>/<slug>/<slug>_<year>.csv`.
pub fn calendar_file(root: &Path, slug: &str, year: i32) -> PathBuf {
    root.join(slug).join(format!("{slug}_{year}.csv"))
}

pub fn read_location(dir: &Path) -> Option<OnlineLocation> {
    serde_json::from_str(&fs::read_to_string(dir.join(LOCATION_FILE)).ok()?).ok()
}

/// Great-circle distance between two latitude/longitude pairs.
pub fn distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lat2) = (a.0.to_radians(), b.0.to_radians());
    let (dlat, dlon) = ((b.0 - a.0).to_radians(), (b.1 - a.1).to_radians());
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().asin()
}

/// IANA name of the system time zone (`$TZ`, else the `/etc/localtime` link target).
pub fn system_timezone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.strip_prefix(':').unwrap_or(&tz);
        if tz.contains('/') && !tz.starts_with('/') {
            return Some(tz.to_owned());
        }
    }
    let target = fs::read_link("/etc/localtime").ok()?;
    let (_, zone) = target.to_str()?.split_once("zoneinfo/")?;
    (!zone.is_empty()).then(|| zone.to_owned())
}

fn curl(args: &[String]) -> Result<String, String> {
    let output = Command::new("/usr/bin/curl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() { format!("curl {}", output.status) } else { detail });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn push_form(args: &mut Vec<String>, key: &str, value: &str) {
    args.push("--data-urlencode".to_owned());
    args.push(format!("{key}={value}"));
}

fn photon(endpoint: &str, params: &[(&str, &str)], country: &str, arabic: bool, limit: &str) -> Result<String, String> {
    let mut args: Vec<String> =
        ["-fsS", "--max-time", "10", "-A", USER_AGENT, "-G", endpoint].into_iter().map(str::to_owned).collect();
    for (key, value) in params {
        push_form(&mut args, key, value);
    }
    push_form(&mut args, "limit", limit);
    push_form(&mut args, "lang", if arabic { "default" } else { "en" });
    if !country.is_empty() {
        push_form(&mut args, "countrycode", country);
    }
    curl(&args).map_err(|e| format!("Photon: {e}"))
}

fn search(params: &[(&str, &str)], country: &str, arabic: bool) -> Result<String, String> {
    photon(PHOTON, params, &country.trim().to_uppercase(), arabic, "8")
}

/// Houses around a point (up to 50, within 1 km): Photon's search cannot list a street's houses.
fn houses_near((latitude, longitude): (f64, f64), arabic: bool) -> Result<String, String> {
    let (latitude, longitude) = (latitude.to_string(), longitude.to_string());
    let params = [("lat", latitude.as_str()), ("lon", longitude.as_str()), ("layer", "house"), ("radius", "1")];
    photon(PHOTON_REVERSE, &params, "", arabic, "50")
}

fn parse_features(json: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(json).map_err(|e| format!("invalid Photon response: {e}"))?;
    if !parsed["features"].is_array() {
        return Err("invalid Photon response: no features".to_owned());
    }
    Ok(parsed)
}

/// `((latitude, longitude), properties)` of each feature with coordinates.
fn located(parsed: &Value) -> impl Iterator<Item = ((f64, f64), &Value)> {
    parsed["features"].as_array().into_iter().flatten().filter_map(|feature| {
        let coordinates = &feature["geometry"]["coordinates"];
        Some(((coordinates[1].as_f64()?, coordinates[0].as_f64()?), &feature["properties"]))
    })
}

fn property(properties: &Value, key: &str) -> String {
    properties[key].as_str().map_or("", str::trim).to_owned()
}

fn normalized(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Street names match when one starts with the other, so `Hauptstr` finds `Hauptstraße`.
fn same_street(found: &str, typed: &str) -> bool {
    let (found, typed) = (normalized(found), normalized(typed));
    !found.is_empty() && !typed.is_empty() && (found.starts_with(&typed) || typed.starts_with(&found))
}

/// `[street house], [zip city]` with empty parts left out.
fn address(street: &str, house_number: &str, zip: &str, city: &str) -> String {
    let line = |a: &str, b: &str| format!("{} {}", a.trim(), b.trim()).trim().to_owned();
    [line(street, house_number), line(zip, city)].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(", ")
}

fn joined(parts: &[&str], separator: &str) -> String {
    parts.iter().map(|part| part.trim()).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(separator)
}

fn push_unique(suggestions: &mut Vec<PlaceSuggestion>, suggestion: PlaceSuggestion) {
    if !suggestions.iter().any(|existing| existing.label == suggestion.label) {
        suggestions.push(suggestion);
    }
}

/// City suggestions from a Photon response; `postcode` selects `place:postcode` results.
pub fn parse_photon(json: &str, postcode: bool) -> Result<Vec<PlaceSuggestion>, String> {
    let parsed = parse_features(json)?;
    let mut suggestions = Vec::new();
    for ((latitude, longitude), properties) in located(&parsed) {
        let text = |key: &str| property(properties, key);
        let name = text("name");
        if name.is_empty() {
            continue;
        }
        let (city, zip, label) = if postcode {
            let city = [text("city"), text("county")].into_iter().find(|part| !part.is_empty()).unwrap_or_default();
            let label = joined(&[&name, &city], " ");
            (city, name, label)
        } else {
            let mut parts: Vec<String> = Vec::new();
            for part in [name.clone(), text("county"), text("state")] {
                if !part.is_empty() && !parts.contains(&part) {
                    parts.push(part);
                }
            }
            (name, String::new(), parts.join(", "))
        };
        let country = text("countrycode").to_uppercase();
        let (street, house_number) = (String::new(), String::new());
        push_unique(&mut suggestions, PlaceSuggestion { city, zip, street, house_number, country, label, latitude, longitude });
    }
    Ok(suggestions)
}

/// Streets matching the typed `street` from a Photon `layer=street` response. Results in
/// another city than the typed one are dropped.
pub fn parse_streets(json: &str, street: &str, city: &str) -> Result<Vec<PlaceSuggestion>, String> {
    let parsed = parse_features(json)?;
    let mut suggestions = Vec::new();
    for ((latitude, longitude), properties) in located(&parsed) {
        let text = |key: &str| property(properties, key);
        let (name, found_city) = (text("name"), text("city"));
        if !same_street(&name, street) || (!city.trim().is_empty() && !found_city.is_empty() && normalized(&found_city) != normalized(city)) {
            continue;
        }
        let zip = text("postcode");
        let city = if found_city.is_empty() { city.trim().to_owned() } else { found_city };
        let label = joined(&[&name, &joined(&[&zip, &city], " ")], ", ");
        let country = text("countrycode").to_uppercase();
        let suggestion =
            PlaceSuggestion { city, zip, street: name, house_number: String::new(), country, label, latitude, longitude };
        push_unique(&mut suggestions, suggestion);
    }
    Ok(suggestions)
}

/// Houses on `street` whose number starts with `prefix`, from several Photon responses,
/// in house-number order (`2, 10, 10a, 12`), at most 12.
pub fn parse_houses(responses: &[String], street: &str, prefix: &str) -> Result<Vec<PlaceSuggestion>, String> {
    let prefix = normalized(prefix);
    let mut suggestions: Vec<PlaceSuggestion> = Vec::new();
    for json in responses {
        let parsed = parse_features(json)?;
        for ((latitude, longitude), properties) in located(&parsed) {
            let text = |key: &str| property(properties, key);
            let (name, number) = (text("street"), text("housenumber"));
            let key = normalized(&number);
            if properties["type"].as_str() != Some("house")
                || !same_street(&name, street)
                || key.is_empty()
                || !key.starts_with(&prefix)
                || suggestions.iter().any(|existing| normalized(&existing.house_number) == key)
            {
                continue;
            }
            let (zip, city) = (text("postcode"), text("city"));
            let label = joined(&[&format!("{name} {number}"), &joined(&[&zip, &city], " ")], ", ");
            let country = text("countrycode").to_uppercase();
            suggestions.push(PlaceSuggestion { city, zip, street: name, house_number: number, country, label, latitude, longitude });
        }
    }
    let order = |number: &str| {
        let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
        (digits.parse::<u64>().unwrap_or(u64::MAX), number.to_lowercase())
    };
    suggestions.sort_by_key(|suggestion| order(&suggestion.house_number));
    suggestions.truncate(12);
    Ok(suggestions)
}

/// Places for a ZIP code when one is given, else cities whose name starts with `city`.
fn cities(country: &str, city: &str, zip: &str, arabic: bool) -> Result<Vec<PlaceSuggestion>, String> {
    let zip = zip.trim();
    if zip.is_empty() {
        parse_photon(&search(&[("q", city.trim()), ("layer", "city")], country, arabic)?, false)
    } else {
        parse_photon(&search(&[("q", zip), ("osm_tag", "place:postcode")], country, arabic)?, true)
    }
}

/// Suggestions for the field being typed in:
/// - **city:** cities, or places for a whole ZIP code.
/// - **street:** streets in the typed city/ZIP code.
/// - **house:** house numbers on the typed street.
pub fn suggest_places(query: &OnlineQuery, field: SuggestField, arabic: bool) -> Result<Vec<PlaceSuggestion>, String> {
    let country = query.country.as_str();
    match field {
        SuggestField::City => cities(country, &query.city, &query.zip, arabic),
        SuggestField::Street => {
            let q = address(&query.street, "", &query.zip, &query.city);
            parse_streets(&search(&[("q", &q), ("layer", "street")], country, arabic)?, &query.street, &query.city)
        }
        SuggestField::House => {
            let q = address(&query.street, "", &query.zip, &query.city);
            let streets = parse_features(&search(&[("q", &q), ("layer", "street")], country, arabic)?)?;
            // A long street has several segments; houses are looked up around the first few.
            let segments: Vec<(f64, f64)> = located(&streets)
                .filter(|(_, properties)| same_street(&property(properties, "name"), &query.street))
                .map(|(coordinates, _)| coordinates)
                .take(3)
                .collect();
            let mut responses = Vec::new();
            if !query.house_number.trim().is_empty() {
                let q = address(&query.street, &query.house_number, &query.zip, &query.city);
                let exact = search(&[("q", &q), ("layer", "house")], country, arabic)?;
                // Photon matches whole house numbers only; the houses around the typed one
                // give the numbers that continue it (`1` → `12`, `14`, …).
                if let Some(point) = parse_address(&exact, &query.street, &query.house_number)? {
                    responses.push(houses_near(point, arabic)?);
                }
                responses.push(exact);
            }
            for segment in segments {
                responses.push(houses_near(segment, arabic)?);
            }
            parse_houses(&responses, &query.street, &query.house_number)
        }
    }
}

/// Coordinates of `street` (and `house_number` when given) in a Photon `house`/`street` response.
/// Results on other streets are ignored, since Photon also returns loose matches. Without the
/// house number, the street itself is used.
pub fn parse_address(json: &str, street: &str, house_number: &str) -> Result<Option<(f64, f64)>, String> {
    let parsed = parse_features(json)?;
    let number = normalized(house_number);
    let mut on_street = None;
    for (coordinates, properties) in located(&parsed) {
        let house = properties["type"].as_str() == Some("house");
        if !same_street(&property(properties, if house { "street" } else { "name" }), street) {
            continue;
        }
        if !number.is_empty() && house && normalized(&property(properties, "housenumber")) == number {
            return Ok(Some(coordinates));
        }
        on_street.get_or_insert(coordinates);
    }
    Ok(on_street)
}

/// The place at a pinned point, from a Photon reverse response: its country, city, ZIP code,
/// street and house number, with the pin's own coordinates. `None` when nothing is there
/// (open sea) or the place has no country.
pub fn parse_place_at(json: &str, (latitude, longitude): (f64, f64)) -> Result<Option<PlaceSuggestion>, String> {
    let parsed = parse_features(json)?;
    let Some((_, properties)) = located(&parsed).next() else { return Ok(None) };
    let text = |key: &str| property(properties, key);
    let country = text("countrycode").to_uppercase();
    if country.is_empty() {
        return Ok(None);
    }
    let kind = text("type");
    let street = if kind == "street" { text("name") } else { text("street") };
    let house_number = if street.is_empty() { String::new() } else { text("housenumber") };
    let zip = text("postcode");
    let city = [text("city"), if kind == "city" { text("name") } else { String::new() }, text("county")]
        .into_iter()
        .find(|part| !part.is_empty())
        .unwrap_or_default();
    let label = address(&street, &house_number, &zip, &city);
    Ok(Some(PlaceSuggestion { city, zip, street, house_number, country, label, latitude, longitude }))
}

/// The place at a point pinned on the map (Photon reverse geocoding).
pub fn place_at(point: (f64, f64), arabic: bool) -> Result<Option<PlaceSuggestion>, String> {
    let (latitude, longitude) = (point.0.to_string(), point.1.to_string());
    let json = photon(PHOTON_REVERSE, &[("lat", &latitude), ("lon", &longitude)], "", arabic, "1")?;
    parse_place_at(&json, point)
}

/// Latitude/longitude of the query: pinned coordinates, else the street address when given,
/// else chosen coordinates, else the ZIP code, else the city. An address Photon cannot find
/// falls back to the ZIP code/city.
pub fn geocode(query: &OnlineQuery, arabic: bool) -> Result<(f64, f64), DownloadError> {
    if let (true, Some(coordinates)) = (query.pinned, query.coordinates) {
        return Ok(coordinates);
    }
    let mut failure = None;
    if !query.street.trim().is_empty() {
        let address = address(&query.street, &query.house_number, &query.zip, &query.city);
        let params = [("q", address.as_str()), ("layer", "house"), ("layer", "street")];
        match search(&params, &query.country, arabic).and_then(|json| parse_address(&json, &query.street, &query.house_number)) {
            Ok(Some(coordinates)) => return Ok(coordinates),
            Ok(None) => eprintln!("Dulook: address {address:?} not found; using the ZIP code/city"),
            Err(error) => failure = Some(error),
        }
    }
    if let Some(coordinates) = query.coordinates {
        return Ok(coordinates);
    }
    if !query.zip.trim().is_empty() {
        match cities(&query.country, "", &query.zip, arabic) {
            Ok(found) if !found.is_empty() => return Ok((found[0].latitude, found[0].longitude)),
            Ok(_) => {}
            Err(error) => failure = Some(error),
        }
    }
    match cities(&query.country, &query.city, "", arabic) {
        Ok(found) => match (found.first(), failure) {
            (Some(first), _) => Ok((first.latitude, first.longitude)),
            (None, Some(error)) => Err(DownloadError::Network(error)),
            (None, None) => Err(DownloadError::NotFound),
        },
        Err(error) => Err(DownloadError::Network(error)),
    }
}

fn clean_cell(cell: &str) -> String {
    let text = TAG.replace_all(cell, "").replace("&nbsp;", " ").replace("&amp;", "&");
    SPACE.replace_all(&text, " ").trim().to_owned()
}

fn csv_cell(cell: &str) -> String {
    if cell.contains([',', '"']) { format!("\"{}\"", cell.replace('"', "\"\"")) } else { cell.to_owned() }
}

fn missing_time(value: &str) -> bool {
    value.is_empty() || value.chars().all(|c| c == '-')
}

/// Converts a `get_period` HTML table of one whole year into Dulook's CSV format.
pub fn period_csv(html: &str, year: i32) -> Result<String, String> {
    let date = Regex::new(&format!(r"^\d{{2}}/\d{{2}}/{year}$")).unwrap();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in ROW.captures_iter(html) {
        let mut cells: Vec<String> = CELL.captures_iter(&row[1]).map(|cell| clean_cell(&cell[1])).collect();
        if cells.len() < HEADER.len() || !date.is_match(&cells[1]) {
            continue;
        }
        cells.truncate(HEADER.len());
        cells[0] = cells[0].trim_matches(|c: char| c == '*' || c.is_whitespace()).to_owned();
        cells[2] = HIJRI_SUFFIX.replace(&cells[2], "").into_owned();
        rows.push(cells);
    }

    for column in 3..HEADER.len() {
        let mut previous = rows
            .iter()
            .map(|row| &row[column])
            .find(|value| !missing_time(value))
            .cloned()
            .ok_or_else(|| format!("no {} times for {year}", HEADER[column]))?;
        for row in &mut rows {
            if missing_time(&row[column]) {
                row[column] = previous.clone();
            } else {
                previous = row[column].clone();
            }
        }
    }

    let first = NaiveDate::from_ymd_opt(year, 1, 1).ok_or_else(|| format!("invalid year {year}"))?;
    let days = NaiveDate::from_ymd_opt(year + 1, 1, 1).map_or(0, |next| (next - first).num_days()) as usize;
    let complete = rows.len() == days
        && rows.iter().enumerate().all(|(index, row)| {
            first.checked_add_days(Days::new(index as u64)).is_some_and(|day| row[1] == day.format("%d/%m/%Y").to_string())
        });
    if !complete {
        return Err(format!("incomplete year {year}"));
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(HEADER.join(","));
    lines.extend(rows.iter().map(|row| row.iter().map(|cell| csv_cell(cell)).collect::<Vec<_>>().join(",")));
    let csv = lines.join("\r\n");
    model::parse_calendar(&csv).map_err(|e| format!("invalid calendar for {year}: {e}"))?;
    Ok(csv)
}

/// Private folder holding the session's cookie jar; removed on drop.
struct TempJar {
    dir: PathBuf,
    file: String,
}

impl TempJar {
    fn new() -> Result<TempJar, String> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("dulook-{}-{nanos}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join("cookies").to_string_lossy().into_owned();
        Ok(TempJar { dir, file })
    }
}

impl Drop for TempJar {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Anonymous salatcalendar.com session.
struct Session {
    jar: TempJar,
}

impl Session {
    fn request(&self, path: &str, form: &[(&str, &str)], fail: bool) -> Result<String, String> {
        let jar = self.jar.file.as_str();
        let mut args: Vec<String> = ["-sS", "--max-time", "30", "-A", USER_AGENT, "-b", jar, "-c", jar]
            .into_iter()
            .chain(["-H", "X-Requested-With: XMLHttpRequest"])
            .map(str::to_owned)
            .collect();
        if fail {
            args.push("-f".to_owned());
        }
        for (key, value) in form {
            push_form(&mut args, key, value);
        }
        args.push(format!("{SITE}/{path}"));
        curl(&args).map_err(|e| format!("{path}: {e}"))
    }

    fn json(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        let body = self.request(path, form, true)?;
        serde_json::from_str(&body).map_err(|e| format!("{path}: invalid response: {e}"))
    }

    /// POST that must answer `{"Status": true, ...}`.
    fn submit(&self, path: &str, form: &[(&str, &str)]) -> Result<(), String> {
        let reply = self.json(path, form)?;
        if reply["Status"] == Value::Bool(true) {
            return Ok(());
        }
        let message = [&reply["Message"], &reply["Errors"]]
            .iter()
            .filter_map(|value| value.as_str().map(str::trim))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Err(format!("{path}: {}", clean_cell(&message)))
    }
}

/// Absolute degrees, minutes and seconds of a coordinate.
fn dms(value: f64) -> [String; 3] {
    let seconds = (value.abs() * 3600.0).round() as i64;
    [seconds / 3600, seconds % 3600 / 60, seconds % 60].map(|part| part.to_string())
}

fn open_session(location: &OnlineLocation) -> Result<Session, String> {
    let session = Session { jar: TempJar::new()? };
    session.request("form", &[], true)?;
    // Answers 307 to the previous page; only the language cookie matters.
    session.request("app/lang/en", &[], false)?;

    let (latitude, longitude) = (format!("{:.6}", location.latitude), format!("{:.6}", location.longitude));
    let elevation = session
        .json(&format!("map/elevation?lat={latitude}&lng={longitude}"), &[])
        .ok()
        .and_then(|reply| reply["results"][0]["elevation"].as_f64())
        .map_or_else(|| "0".to_owned(), |meters| format!("{}", meters.round() as i64));
    let country = session.json(&format!("app/get_country/{}", location.country), &[]).ok();
    let field = |key: &str| country.as_ref().and_then(|reply| reply[key].as_str()).map(str::to_owned);
    let country_name = field("name").unwrap_or_else(|| location.country.clone());
    let calc_id = field("CalcMethodId").unwrap_or_default();

    let [lon_dd, lon_mm, lon_ss] = dms(location.longitude);
    let [lat_dd, lat_mm, lat_ss] = dms(location.latitude);
    session.submit(
        "app/save_location",
        &[
            ("City", &location.city),
            ("Country", &country_name),
            ("TimeZone", &location.timezone),
            ("Altitude", &elevation),
            ("countryCalcId", &calc_id),
            ("LonDD", &lon_dd),
            ("LonMM", &lon_mm),
            ("LonSS", &lon_ss),
            ("Lon_Dir", if location.longitude < 0.0 { "W" } else { "E" }),
            ("LatDD", &lat_dd),
            ("LatMM", &lat_mm),
            ("LatSS", &lat_ss),
            ("Lat_Dir", if location.latitude < 0.0 { "S" } else { "N" }),
            ("Latitude", &latitude),
            ("Longitude", &longitude),
        ],
    )?;
    session.submit("form/checkform", &CALC_SETTINGS)?;
    Ok(session)
}

/// Downloads `years` into `dir` as `<slug>_<year>.csv` and writes `location.json`.
/// `known` reuses a stored location's coordinates and time zone instead of geocoding.
pub fn download(
    query: &OnlineQuery,
    known: Option<OnlineLocation>,
    years: &[i32],
    dir: &Path,
    arabic: bool,
    progress: impl Fn(usize, usize),
) -> Result<OnlineLocation, DownloadError> {
    let location = match known {
        Some(location) => location,
        None => {
            let (latitude, longitude) = geocode(query, arabic)?;
            OnlineLocation {
                country: query.country.clone(),
                city: query.city.clone(),
                zip: query.zip.clone(),
                street: query.street.clone(),
                house_number: query.house_number.clone(),
                latitude,
                longitude,
                timezone: system_timezone().ok_or(DownloadError::Timezone)?,
            }
        }
    };
    let slug = slug(query);
    let session = open_session(&location).map_err(DownloadError::Network)?;
    for (index, year) in years.iter().enumerate() {
        let from = format!("01/01/{year}");
        let to = format!("31/12/{year}");
        let reply = session.json("app/get_period", &[("from", &from), ("to", &to)]).map_err(DownloadError::Network)?;
        let html = reply["html"].as_str().ok_or_else(|| DownloadError::Network(format!("get_period {year}: no html")))?;
        let csv = period_csv(html, *year).map_err(DownloadError::Network)?;
        let path = dir.join(format!("{slug}_{year}.csv"));
        try_write_atomic(&path, &csv).map_err(|e| DownloadError::Network(format!("{}: {e}", path.display())))?;
        progress(index + 1, years.len());
    }
    let text = serde_json::to_string_pretty(&location).map_err(|e| DownloadError::Network(e.to_string()))?;
    let path = dir.join(LOCATION_FILE);
    try_write_atomic(&path, &(text + "\n")).map_err(|e| DownloadError::Network(format!("{}: {e}", path.display())))?;
    Ok(location)
}
