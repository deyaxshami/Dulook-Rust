//! Calendar parsing and prayer scheduling (port of the original `Model.js`).

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};
use regex::Regex;
use serde::Serialize;

use crate::jsutil::{js_trim, pad2};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Prayer {
    Fajr,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
}

impl Prayer {
    pub const ALL: [Prayer; 5] = [Prayer::Fajr, Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib, Prayer::Isha];

    pub fn name(self) -> &'static str {
        match self {
            Prayer::Fajr => "Fajr",
            Prayer::Dhuhr => "Dhuhr",
            Prayer::Asr => "Asr",
            Prayer::Maghrib => "Maghrib",
            Prayer::Isha => "Isha",
        }
    }

    pub fn from_name(name: &str) -> Option<Prayer> {
        Prayer::ALL.into_iter().find(|p| p.name() == name)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Prayer times of one calendar row, `HH:MM` in 24-hour format, indexed like [`Prayer::ALL`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrayerTimes(pub [String; 5]);

impl PrayerTimes {
    pub fn get(&self, prayer: Prayer) -> &str {
        &self.0[prayer.index()]
    }
}

#[derive(Clone, Debug, Default)]
pub struct Calendar {
    pub dates: BTreeMap<String, PrayerTimes>,
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
}

const REQUIRED_HEADERS: [&str; 6] = ["date", "fajr", "dhuhr", "asr", "maghrib", "isha"];

fn header_alias(normalized: &str) -> Option<&'static str> {
    Some(match normalized {
        "date" | "التاريخ" => "date",
        "fajr" | "الفجر" => "fajr",
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

fn validate_prayer_order(prayers: &PrayerTimes, key: &str) -> Result<(), String> {
    let invalid = || format!("Invalid prayer order: {key}");
    let mut previous = minute_of_day(prayers.get(Prayer::Fajr));
    for prayer in [Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib] {
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

pub fn parse_calendar(csv: &str) -> Result<Calendar, String> {
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

    let mut calendar = Calendar::default();
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
            parse_time(cell("dhuhr"))?,
            parse_time(cell("asr"))?,
            parse_time(cell("maghrib"))?,
            parse_time(cell("isha"))?,
        ]);
        validate_prayer_order(&prayers, &key)?;
        events_for_row(&key, &prayers)?;
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
    Ok(calendar)
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

pub fn events_for_row(key: &str, prayers: &PrayerTimes) -> Result<Vec<PrayerEvent>, String> {
    let base = date_from_key(key).ok_or_else(|| format!("Invalid date: {key}"))?;
    let mut events = Vec::with_capacity(Prayer::ALL.len());
    let mut day_offset = 0u64;
    let mut previous_minute: Option<u32> = None;
    for prayer in Prayer::ALL {
        let time = prayers.get(prayer);
        let minute = minute_of_day(time);
        if previous_minute.is_some_and(|previous| minute < previous) {
            day_offset += 1;
        }
        let invalid = || format!("Invalid local time: {key} {time}");
        let target = base.checked_add_days(Days::new(day_offset)).ok_or_else(invalid)?;
        let naive = target.and_hms_opt(minute / 60, minute % 60, 0).ok_or_else(invalid)?;
        let instant = match Local.from_local_datetime(&naive) {
            LocalResult::Single(t) => t,
            LocalResult::Ambiguous(earliest, _) => earliest,
            LocalResult::None => return Err(invalid()),
        };
        events.push(PrayerEvent {
            key: format!("{key}|{}", prayer.name()),
            prayer_date: key.to_owned(),
            name: prayer,
            time: time.to_owned(),
            timestamp: instant.timestamp_millis(),
        });
        previous_minute = Some(minute);
    }
    Ok(events)
}

fn local_today(now_ms: i64) -> String {
    let date = Local
        .timestamp_millis_opt(now_ms)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default();
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

pub fn events_around(calendar: &Calendar, now_ms: i64) -> Vec<PrayerEvent> {
    let today = local_today(now_ms);
    let mut events = Vec::new();
    for offset in -1..=2 {
        let key = shifted_key(&today, offset);
        if let Some(prayers) = calendar.dates.get(&key)
            && let Ok(row) = events_for_row(&key, prayers)
        {
            events.extend(row);
        }
    }
    events.sort_by_key(|event| event.timestamp);
    events
}

fn prayer_event_for_row(key: &str, prayers: &PrayerTimes, prayer: Prayer) -> Option<PrayerEvent> {
    events_for_row(key, prayers).ok()?.into_iter().find(|event| event.name == prayer)
}

pub fn last_third_events_around(calendar: &Calendar, now_ms: i64) -> Vec<LastThirdEvent> {
    let today = local_today(now_ms);
    let mut events = Vec::new();
    for offset in -2..=1 {
        let night_date = shifted_key(&today, offset);
        let next_date = shifted_key(&night_date, 1);
        let (Some(prayers), Some(next_prayers)) = (calendar.dates.get(&night_date), calendar.dates.get(&next_date)) else {
            continue;
        };
        let maghrib = prayer_event_for_row(&night_date, prayers, Prayer::Maghrib);
        let fajr = prayer_event_for_row(&next_date, next_prayers, Prayer::Fajr);
        let (Some(maghrib), Some(fajr)) = (maghrib, fajr) else { continue };
        if fajr.timestamp <= maghrib.timestamp {
            continue;
        }
        let timestamp = (maghrib.timestamp as f64 + (fajr.timestamp - maghrib.timestamp) as f64 * 2.0 / 3.0).round() as i64;
        events.push(LastThirdEvent {
            key: format!("{night_date}|last-third"),
            night_date,
            timestamp,
            maghrib_timestamp: maghrib.timestamp,
            fajr_timestamp: fajr.timestamp,
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
    Snapshot { date_key: local_today(now_ms), next, remaining_seconds }
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
