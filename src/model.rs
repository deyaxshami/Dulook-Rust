//! Calendar parsing and prayer scheduling (port of the original `Model.js`).

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use chrono::{Days, Local, NaiveDate, TimeZone};
use chrono_tz::Tz;
use regex::Regex;
use serde::Serialize;

use crate::jsutil::{js_trim, pad2};

/// Daily times Dulook schedules: the five prayers plus Sunrise, the sun's yellowing and
/// Islamic midnight. Declaration order is [`Prayer::ALL`] order; settings index by it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Prayer {
    Fajr,
    Sunrise,
    Dhuhr,
    Asr,
    Yellowing,
    Maghrib,
    Isha,
    Midnight,
}

impl Prayer {
    pub const ALL: [Prayer; 8] = [
        Prayer::Fajr,
        Prayer::Sunrise,
        Prayer::Dhuhr,
        Prayer::Asr,
        Prayer::Yellowing,
        Prayer::Maghrib,
        Prayer::Isha,
        Prayer::Midnight,
    ];

    /// Times a calendar row lists; Yellowing and Midnight are derived.
    pub const ROW: [Prayer; 6] = [Prayer::Fajr, Prayer::Sunrise, Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib, Prayer::Isha];

    pub fn name(self) -> &'static str {
        match self {
            Prayer::Fajr => "Fajr",
            Prayer::Sunrise => "Sunrise",
            Prayer::Dhuhr => "Dhuhr",
            Prayer::Asr => "Asr",
            Prayer::Yellowing => "Yellowing",
            Prayer::Maghrib => "Maghrib",
            Prayer::Isha => "Isha",
            Prayer::Midnight => "Midnight",
        }
    }

    pub fn from_name(name: &str) -> Option<Prayer> {
        Prayer::ALL.into_iter().find(|p| p.name() == name)
    }

    /// False for Sunrise, Yellowing and Midnight, which get no Adhan and are not called prayers.
    pub fn is_prayer(self) -> bool {
        !matches!(self, Prayer::Sunrise | Prayer::Yellowing | Prayer::Midnight)
    }

    /// Position in a calendar row ([`Prayer::ROW`]); `None` for the derived Yellowing and Midnight.
    fn row_index(self) -> Option<usize> {
        Prayer::ROW.iter().position(|&p| p == self)
    }
}

/// Times of one calendar row, `HH:MM` in 24-hour format, indexed like [`Prayer::ROW`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrayerTimes(pub [String; 6]);

impl PrayerTimes {
    /// Panics for `Prayer::Yellowing` and `Prayer::Midnight`, which no row lists.
    pub fn get(&self, prayer: Prayer) -> &str {
        &self.0[prayer.row_index().expect("Yellowing and Midnight are not calendar columns")]
    }
}

#[derive(Clone, Debug, Default)]
pub struct Calendar {
    pub dates: BTreeMap<String, PrayerTimes>,
    /// Latitude estimated from the rows' day lengths; see `estimate_latitude`.
    pub latitude: Option<f64>,
    source_timezone: Option<Tz>,
}

impl Calendar {
    pub fn row_count(&self) -> usize {
        self.dates.len()
    }

    pub fn first_date(&self) -> &str {
        self.dates.keys().next().map(String::as_str).unwrap_or("")
    }

    pub fn last_date(&self) -> &str {
        self.dates.keys().next_back().map(String::as_str).unwrap_or("")
    }

    pub(crate) fn source_timezone(&self) -> Option<Tz> {
        self.source_timezone
    }
}

const REQUIRED_HEADERS: [&str; 7] = ["date", "fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha"];

fn header_alias(normalized: &str) -> Option<&'static str> {
    Some(match normalized {
        "date" | "التاريخ" => "date",
        "fajr" | "الفجر" => "fajr",
        "sunrise" | "shuruq" | "الشروق" => "sunrise",
        "dhuhr" | "zuhr" | "الظهر" => "dhuhr",
        "asr" | "العصر" => "asr",
        "maghrib" | "المغرب" => "maghrib",
        "isha" | "ishaa" | "العشاء" => "isha",
        _ => return None,
    })
}

static DMY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]{2})/([0-9]{2})/([0-9]{4})$").unwrap());
static YMD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$").unwrap());
static TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]{1,2}):([0-9]{2})\s*(am|pm)?$").unwrap());
static TIME_24: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]{1,2}):([0-9]{2})$").unwrap());

pub fn parse_csv_row(line: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut value = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if quoted && chars.peek() == Some(&'"') {
                value.push('"');
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if c == ',' && !quoted {
            values.push(js_trim(&value).to_owned());
            value.clear();
        } else {
            value.push(c);
        }
    }
    if quoted {
        return Err("Unclosed quoted field".to_owned());
    }
    values.push(js_trim(&value).to_owned());
    Ok(values)
}

pub fn canonical_header(value: &str) -> String {
    let trimmed = js_trim(value);
    let normalized = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed).to_lowercase();
    match header_alias(&normalized) {
        Some(alias) => alias.to_owned(),
        None => normalized,
    }
}

pub fn date_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn parse_date(value: &str) -> Result<NaiveDate, String> {
    let text = js_trim(value);
    let invalid = || format!("Invalid date: {text}");
    let (year, month, day) = if let Some(m) = DMY.captures(text) {
        (m[3].parse::<i32>().unwrap(), m[2].parse::<u32>().unwrap(), m[1].parse::<u32>().unwrap())
    } else if let Some(m) = YMD.captures(text) {
        (m[1].parse::<i32>().unwrap(), m[2].parse::<u32>().unwrap(), m[3].parse::<u32>().unwrap())
    } else {
        return Err(invalid());
    };
    // JS maps years below 100 to 19xx, which then fails its round-trip check.
    if year < 100 {
        return Err(invalid());
    }
    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(invalid)
}

pub fn parse_time(value: &str) -> Result<String, String> {
    let text = js_trim(value).to_lowercase();
    let invalid = || format!("Invalid time: {value}");
    let m = TIME.captures(&text).ok_or_else(invalid)?;
    let mut hour: u32 = m[1].parse().unwrap();
    let minute: u32 = m[2].parse().unwrap();
    let meridiem = m.get(3).map_or("", |x| x.as_str());
    if minute > 59 || (!meridiem.is_empty() && !(1..=12).contains(&hour)) || (meridiem.is_empty() && hour > 23) {
        return Err(invalid());
    }
    if !meridiem.is_empty() {
        hour %= 12;
        if meridiem == "pm" {
            hour += 12;
        }
    }
    Ok(format!("{}:{}", pad2(hour.into()), pad2(minute.into())))
}

pub fn minute_of_day(time: &str) -> u32 {
    let mut parts = time.split(':');
    let hour: u32 = parts.next().and_then(|h| h.parse().ok()).unwrap_or(0);
    let minute: u32 = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    hour * 60 + minute
}

/// Altitude of the sun's centre at sunrise/sunset: refraction plus the solar semi-diameter.
const SUNSET_ALTITUDE_DEG: f64 = -0.833;
/// Altitude at which the sun yellows: where it stands 15 minutes before sunset at Qassim
/// (26.33°N) at the equinox, the Saudi-latitude estimate of Dr. Abdullah Al-Misnid.
const YELLOWING_ALTITUDE_DEG: f64 = 2.5;
/// Rows closer to an equinox say almost nothing about latitude through their day length.
const MIN_FIT_DECLINATION_DEG: f64 = 10.0;

/// Solar declination in degrees at noon of `key` (U.S. Naval Observatory approximation).
fn declination(key: &str) -> Option<f64> {
    let d = (date_from_key(key)? - NaiveDate::from_ymd_opt(2000, 1, 1)?).num_days() as f64;
    let g = (357.529 + 0.98560028 * d).to_radians();
    let q = 280.459 + 0.98564736 * d;
    let l = (q + 1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let e = (23.439 - 0.00000036 * d).to_radians();
    Some((e.sin() * l.sin()).asin().to_degrees())
}

/// Hour angle in degrees at which the sun's centre stands at `altitude`; `None` when it never does that day.
fn hour_angle(altitude: f64, latitude: f64, declination: f64) -> Option<f64> {
    let (alt, lat, dec) = (altitude.to_radians(), latitude.to_radians(), declination.to_radians());
    let cos = (alt.sin() - lat.sin() * dec.sin()) / (lat.cos() * dec.cos());
    (-1.0..=1.0).contains(&cos).then(|| cos.acos().to_degrees())
}

/// Latitude in degrees that best explains the rows' Sunrise→Maghrib day lengths; `None`
/// when no row is at least `MIN_FIT_DECLINATION_DEG` from an equinox.
fn estimate_latitude(dates: &BTreeMap<String, PrayerTimes>) -> Option<f64> {
    let samples: Vec<(f64, f64)> = dates
        .iter()
        .filter_map(|(key, row)| {
            let dec = declination(key)?;
            let length = minute_of_day(row.get(Prayer::Maghrib)) as f64 - minute_of_day(row.get(Prayer::Sunrise)) as f64;
            (dec.abs() >= MIN_FIT_DECLINATION_DEG).then_some((dec, length))
        })
        .collect();
    if samples.is_empty() {
        return None;
    }
    // A day spans twice the sunset hour angle, and each degree of it is 4 minutes.
    let error = |latitude: f64| {
        samples.iter().try_fold(0.0, |sum, &(dec, length)| {
            hour_angle(SUNSET_ALTITUDE_DEG, latitude, dec).map(|h| sum + (8.0 * h - length).powi(2))
        })
        .unwrap_or(f64::INFINITY)
    };
    let best_of = |candidates: &mut dyn Iterator<Item = f64>| {
        let mut best = (f64::NAN, f64::INFINITY);
        for latitude in candidates {
            let err = error(latitude);
            if err < best.1 {
                best = (latitude, err);
            }
        }
        best.0
    };
    let coarse = best_of(&mut (-65..=65).map(f64::from));
    let fine = best_of(&mut (0..=200).map(|i| coarse - 1.0 + f64::from(i) * 0.01));
    (!fine.is_nan()).then_some(fine)
}

fn validate_prayer_order(prayers: &PrayerTimes, key: &str) -> Result<(), String> {
    let invalid = || format!("Invalid prayer order: {key}");
    let mut previous = minute_of_day(prayers.get(Prayer::Fajr));
    for prayer in [Prayer::Sunrise, Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib] {
        let current = minute_of_day(prayers.get(prayer));
        if current <= previous {
            return Err(invalid());
        }
        previous = current;
    }
    if minute_of_day(prayers.get(Prayer::Isha)) == previous {
        return Err(invalid());
    }
    Ok(())
}

pub fn parse_calendar(csv: &str, source_timezone: Option<Tz>) -> Result<Calendar, String> {
    let text = csv.replace('\r', "");
    let meaningful: Vec<&str> = text.split('\n').filter(|line| !js_trim(line).is_empty()).collect();
    if meaningful.len() < 2 {
        return Err("The calendar is empty".to_owned());
    }

    let headers = parse_csv_row(meaningful[0])?;
    let mut columns: HashMap<String, usize> = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        columns.insert(canonical_header(header), index);
    }
    for header in REQUIRED_HEADERS {
        if !columns.contains_key(header) {
            return Err(format!("Missing column: {header}"));
        }
    }

    let mut calendar = Calendar { source_timezone, ..Calendar::default() };
    for line in &meaningful[1..] {
        let row = parse_csv_row(line)?;
        let cell = |name: &str| row.get(columns[name]).map(String::as_str).unwrap_or("");
        let date = parse_date(cell("date"))?;
        let key = date_key(date);
        if calendar.dates.contains_key(&key) {
            return Err(format!("Duplicate date: {key}"));
        }
        let prayers = PrayerTimes([
            parse_time(cell("fajr"))?,
            parse_time(cell("sunrise"))?,
            parse_time(cell("dhuhr"))?,
            parse_time(cell("asr"))?,
            parse_time(cell("maghrib"))?,
            parse_time(cell("isha"))?,
        ]);
        validate_prayer_order(&prayers, &key)?;
        row_timestamps(date, &prayers, source_timezone)?;
        calendar.dates.insert(key, prayers);
    }

    let keys: Vec<&String> = calendar.dates.keys().collect();
    for pair in keys.windows(2) {
        let (key, next_key) = (pair[0], pair[1]);
        let row = &calendar.dates[key];
        let isha = minute_of_day(row.get(Prayer::Isha));
        if *next_key == shifted_key(key, 1)
            && isha < minute_of_day(row.get(Prayer::Maghrib))
            && isha >= minute_of_day(calendar.dates[next_key].get(Prayer::Fajr))
        {
            return Err(format!("Invalid overnight Isha: {key}"));
        }
    }
    calendar.latitude = estimate_latitude(&calendar.dates);
    Ok(calendar)
}

/// Joins calendars of consecutive periods (one bundled CSV per year) into one.
pub fn merge_calendars(calendars: impl IntoIterator<Item = Calendar>) -> Result<Calendar, String> {
    let mut calendars = calendars.into_iter();
    let mut merged = calendars.next().unwrap_or_default();
    for calendar in calendars {
        if calendar.source_timezone != merged.source_timezone {
            return Err("Mismatched calendar time zones".to_owned());
        }
        for (key, prayers) in calendar.dates {
            if merged.dates.contains_key(&key) {
                return Err(format!("Duplicate date: {key}"));
            }
            merged.dates.insert(key, prayers);
        }
    }
    merged.latitude = estimate_latitude(&merged.dates);
    Ok(merged)
}

fn date_from_key(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%Y-%m-%d").ok()
}

pub fn shifted_key(key: &str, amount: i64) -> String {
    let Some(date) = date_from_key(key) else { return String::new() };
    let shifted = if amount >= 0 {
        date.checked_add_days(Days::new(amount as u64))
    } else {
        date.checked_sub_days(Days::new(amount.unsigned_abs()))
    };
    shifted.map(date_key).unwrap_or_default()
}

pub fn format_time_12_hour(time: &str) -> String {
    let text = js_trim(time);
    let Some(m) = TIME_24.captures(text) else { return text.to_owned() };
    let hour: u32 = m[1].parse().unwrap();
    let minute: u32 = m[2].parse().unwrap();
    if hour > 23 || minute > 59 {
        return text.to_owned();
    }
    let display_hour = if hour.is_multiple_of(12) { 12 } else { hour % 12 };
    format!("{display_hour}:{}{}", pad2(minute.into()), if hour < 12 { " AM" } else { " PM" })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrayerEvent {
    pub key: String,
    pub prayer_date: String,
    pub name: Prayer,
    pub time: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastThirdEvent {
    pub key: String,
    pub night_date: String,
    pub timestamp: i64,
    pub maghrib_timestamp: i64,
    pub fajr_timestamp: i64,
}

pub trait Timed {
    fn timestamp(&self) -> i64;
}

impl Timed for PrayerEvent {
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl Timed for LastThirdEvent {
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

fn row_timestamps(date: NaiveDate, prayers: &PrayerTimes, source_timezone: Option<Tz>) -> Result<[i64; 6], String> {
    let mut timestamps = [0; Prayer::ROW.len()];
    let mut day_offset = 0u64;
    let mut previous_minute: Option<u32> = None;
    for (index, time) in prayers.0.iter().enumerate() {
        let minute = minute_of_day(time);
        if previous_minute.is_some_and(|previous| minute < previous) {
            day_offset += 1;
        }
        let target = date.checked_add_days(Days::new(day_offset)).ok_or_else(|| format!("Invalid local time: {date} {time}"))?;
        let invalid = || format!("Invalid local time: {target} {time}");
        let naive = target.and_hms_opt(minute / 60, minute % 60, 0).ok_or_else(invalid)?;
        let timestamp = match source_timezone {
            Some(zone) => zone.from_local_datetime(&naive).earliest().map(|t| t.timestamp_millis()),
            None => Local.from_local_datetime(&naive).earliest().map(|t| t.timestamp_millis()),
        };
        timestamps[index] = timestamp.ok_or_else(invalid)?;
        previous_minute = Some(minute);
    }
    Ok(timestamps)
}

fn local_time(timestamp: i64) -> Option<String> {
    Some(Local.timestamp_millis_opt(timestamp).single()?.format("%H:%M").to_string())
}

pub(crate) fn calendar_date_at(timestamp: i64, source_timezone: Option<Tz>) -> NaiveDate {
    match source_timezone {
        Some(zone) => zone.timestamp_millis_opt(timestamp).single().map(|t| t.date_naive()),
        None => Local.timestamp_millis_opt(timestamp).single().map(|t| t.date_naive()),
    }
    .unwrap_or_default()
}

pub fn events_around(calendar: &Calendar, now_ms: i64) -> Vec<PrayerEvent> {
    let today = date_key(calendar_date_at(now_ms, calendar.source_timezone));
    let mut events = Vec::new();
    for offset in -1..=2 {
        events.extend(day_events(calendar, &shifted_key(&today, offset)));
    }
    events.sort_by_key(|event| event.timestamp);
    events
}

/// Maghrib of `night_date` and Fajr of the following day, when both rows exist and Fajr is later.
fn night_bounds(calendar: &Calendar, night_date: &str) -> Option<(i64, i64)> {
    let date = date_from_key(night_date)?;
    let next_date = date.succ_opt()?;
    let night = row_timestamps(date, calendar.dates.get(night_date)?, calendar.source_timezone).ok()?;
    let morning = row_timestamps(next_date, calendar.dates.get(&date_key(next_date))?, calendar.source_timezone).ok()?;
    let maghrib = night[Prayer::Maghrib.row_index()?];
    let fajr = morning[Prayer::Fajr.row_index()?];
    (fajr > maghrib).then_some((maghrib, fajr))
}

/// Islamic midnight of the night that begins at Maghrib on `night_date`: halfway to the
/// next day's Fajr, truncated to the minute.
pub fn midnight_event(calendar: &Calendar, night_date: &str) -> Option<PrayerEvent> {
    let (maghrib, fajr) = night_bounds(calendar, night_date)?;
    let middle = maghrib + (fajr - maghrib) / 2;
    let timestamp = middle - middle.rem_euclid(60_000);
    let time = local_time(timestamp)?;
    Some(PrayerEvent {
        key: format!("{night_date}|Midnight"),
        prayer_date: night_date.to_owned(),
        name: Prayer::Midnight,
        time,
        timestamp,
    })
}

/// When the sun yellows on `key` (the end of the chosen Asr time): the sun's centre at
/// `YELLOWING_ALTITUDE_DEG`, measured from solar noon (midway between Sunrise and Maghrib),
/// truncated to the minute. `None` without a latitude, when the sun never gets that high,
/// or when the result is not strictly between Asr and Maghrib.
fn yellowing_event(latitude: Option<f64>, key: &str, row: &[PrayerEvent]) -> Option<PrayerEvent> {
    let find = |prayer: Prayer| row.iter().find(|event| event.name == prayer);
    let (sunrise, asr, maghrib) = (find(Prayer::Sunrise)?, find(Prayer::Asr)?, find(Prayer::Maghrib)?);
    let hours = hour_angle(YELLOWING_ALTITUDE_DEG, latitude?, declination(key)?)?;
    let noon = sunrise.timestamp + (maghrib.timestamp - sunrise.timestamp) / 2;
    // One degree of hour angle is 4 minutes.
    let middle = noon + (hours * 240_000.0) as i64;
    let timestamp = middle - middle.rem_euclid(60_000);
    if !(asr.timestamp < timestamp && timestamp < maghrib.timestamp) {
        return None;
    }
    let time = local_time(timestamp)?;
    Some(PrayerEvent {
        key: format!("{key}|Yellowing"),
        prayer_date: key.to_owned(),
        name: Prayer::Yellowing,
        time,
        timestamp,
    })
}

/// The first time after `event` (e.g. Maghrib after Yellowing, the next Fajr after Midnight).
pub fn following_event(calendar: &Calendar, event: &PrayerEvent) -> Option<PrayerEvent> {
    events_around(calendar, event.timestamp).into_iter().find(|next| next.timestamp > event.timestamp)
}

/// Every time of `key` in [`Prayer::ALL`] order; Yellowing only when computable, Midnight
/// only when the next day's row exists.
pub fn day_events(calendar: &Calendar, key: &str) -> Vec<PrayerEvent> {
    let Some(prayers) = calendar.dates.get(key) else { return Vec::new() };
    let Some(date) = date_from_key(key) else { return Vec::new() };
    let Ok(timestamps) = row_timestamps(date, prayers, calendar.source_timezone) else { return Vec::new() };
    let mut events = Vec::with_capacity(Prayer::ALL.len());
    for (index, (name, timestamp)) in Prayer::ROW.into_iter().zip(timestamps).enumerate() {
        let time = if calendar.source_timezone.is_some() {
            let Some(time) = local_time(timestamp) else { return Vec::new() };
            time
        } else {
            prayers.0[index].clone()
        };
        events.push(PrayerEvent {
            key: format!("{key}|{}", name.name()),
            prayer_date: key.to_owned(),
            name,
            time,
            timestamp,
        });
    }
    if let Some(yellowing) = yellowing_event(calendar.latitude, key, &events) {
        let at = events.iter().position(|event| event.name == Prayer::Maghrib).unwrap_or(events.len());
        events.insert(at, yellowing);
    }
    events.extend(midnight_event(calendar, key));
    events
}

pub fn last_third_events_around(calendar: &Calendar, now_ms: i64) -> Vec<LastThirdEvent> {
    let today = date_key(calendar_date_at(now_ms, calendar.source_timezone));
    let mut events = Vec::new();
    for offset in -2..=1 {
        let night_date = shifted_key(&today, offset);
        let Some((maghrib, fajr)) = night_bounds(calendar, &night_date) else { continue };
        let timestamp = (maghrib as f64 + (fajr - maghrib) as f64 * 2.0 / 3.0).round() as i64;
        events.push(LastThirdEvent {
            key: format!("{night_date}|last-third"),
            night_date,
            timestamp,
            maghrib_timestamp: maghrib,
            fajr_timestamp: fajr,
        });
    }
    events
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub date_key: String,
    pub next: Option<PrayerEvent>,
    pub remaining_seconds: i64,
}

pub fn snapshot(calendar: &Calendar, now_ms: i64) -> Snapshot {
    let next = events_around(calendar, now_ms).into_iter().find(|event| event.timestamp >= now_ms);
    let remaining_seconds = next.as_ref().map_or(0, |event| ((event.timestamp - now_ms) / 1000).max(0));
    Snapshot { date_key: date_key(calendar_date_at(now_ms, calendar.source_timezone)), next, remaining_seconds }
}

pub fn events_due<T: Timed + Clone>(events: &[T], previous_ms: i64, now_ms: i64, grace_ms: i64, advance_ms: i64) -> Vec<T> {
    let lower_bound = previous_ms.max(now_ms - grace_ms);
    let advance = advance_ms.max(0);
    events
        .iter()
        .filter(|event| {
            let due_at = event.timestamp() - advance;
            due_at > lower_bound && due_at <= now_ms
        })
        .cloned()
        .collect()
}

pub fn due_events(calendar: &Calendar, previous_ms: i64, now_ms: i64, grace_ms: i64, advance_ms: i64) -> Vec<PrayerEvent> {
    events_due(&events_around(calendar, now_ms), previous_ms, now_ms, grace_ms, advance_ms)
}

pub fn due_last_third_events(calendar: &Calendar, previous_ms: i64, now_ms: i64, grace_ms: i64) -> Vec<LastThirdEvent> {
    events_due(&last_third_events_around(calendar, now_ms), previous_ms, now_ms, grace_ms, 0)
}

pub fn format_countdown(seconds: i64) -> String {
    let value = seconds.max(0);
    format!("{}:{}:{}", pad2(value / 3600), pad2((value % 3600) / 60), pad2(value % 60))
}

pub fn shuffled_indices(len: usize, avoid_first: Option<usize>, mut random: impl FnMut() -> f64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    for j in (1..order.len()).rev() {
        let r = random().clamp(0.0, 0.999_999);
        let swap_index = (r * (j + 1) as f64).floor() as usize;
        order.swap(j, swap_index);
    }
    if order.len() > 1 && Some(order[0]) == avoid_first {
        order.swap(0, 1);
    }
    order
}
