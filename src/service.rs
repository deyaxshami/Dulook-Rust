//! Runtime state of the plugin: calendar, alerts, Adhan audio, and Hadith
//! (port of the original `Service.qml` logic).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::LazyLock;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};

use crate::jsutil::{js_number, js_string, js_trim, localized_digits, now_ms, pad2, timezone_offset_minutes};
use crate::model::{self, Calendar, Prayer, PrayerEvent, PrayerTimes};
use crate::protocol::{Command as Cmd, Event};
use crate::settings::{Settings, expand_path};
use crate::translations::{self, Lang};

const ALERT_GRACE_MS: i64 = 120_000;
const PRAYER_REMINDER_LEAD_MS: i64 = 15 * 60 * 1000;
const HADITH_HISTORY_LIMIT: usize = 20;
const HADITH_RETRY: Duration = Duration::from_secs(5 * 60);
const HADITH_API: &str = "https://cdn.jsdelivr.net/gh/fawazahmed0/hadith-api@df57907be35291c91ad6a6691180e22ca9920784/editions/";

static VARIANT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]+)\.([0-9]+)$").unwrap());

fn lenient_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let number = js_number(&value);
    Ok(if number.is_finite() { number as i64 } else { 0 })
}

fn lenient_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(if value.is_null() { String::new() } else { js_string(&value) })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hadith {
    #[serde(deserialize_with = "lenient_string")]
    pub id: String,
    #[serde(deserialize_with = "lenient_string")]
    pub collection: String,
    #[serde(deserialize_with = "lenient_string")]
    pub hadith_number: String,
    #[serde(deserialize_with = "lenient_number")]
    pub book_number: i64,
    #[serde(deserialize_with = "lenient_number")]
    pub in_book_hadith_number: i64,
    #[serde(deserialize_with = "lenient_string")]
    pub arabic: String,
    #[serde(deserialize_with = "lenient_string")]
    pub english: String,
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub source: Option<String>,
}

#[derive(Deserialize)]
struct FallbackFile {
    hadiths: Vec<Hadith>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StateFile {
    version: u32,
    #[serde(deserialize_with = "lenient_string")]
    last_alert_key: String,
    #[serde(deserialize_with = "lenient_string")]
    last_reminder_key: String,
    #[serde(deserialize_with = "lenient_string")]
    last_third_alert_key: String,
}

pub struct Paths {
    pub plugin_dir: PathBuf,
    pub omarchy_path: String,
    pub cache_file: PathBuf,
    pub state_file: PathBuf,
    pub home: String,
}

type FileSignature = Option<(SystemTime, u64, u64)>;

fn file_signature(path: &str) -> FileSignature {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len(), metadata.ino()))
}

/// Writes `text` atomically (temporary file + rename); errors are logged and ignored.
fn write_atomic(path: &Path, text: &str) {
    let result = (|| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        eprintln!("Dulook: could not write {}: {error}", path.display());
    }
}

/// Spawns a command without waiting for it; a helper thread reaps it.
fn exec_detached(command: &[String]) {
    let Some((program, args)) = command.split_first() else { return };
    match Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("Dulook: could not run {program}: {error}"),
    }
}

fn bundled_sounds(plugin_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(plugin_dir.join("adans")) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mp3")))
        .collect();
    names.sort();
    names.into_iter().map(|name| format!("adans/{name}")).collect()
}

/// JS truthiness for JSON values.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn record_from_response(raw: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(raw).map_err(|e| format!("Invalid Hadith response: {e}"))?;
    parsed["hadiths"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|record| truthy(record.get("text")) && !js_trim(&js_string(&record["text"])).is_empty())
        .cloned()
        .ok_or_else(|| "Empty Hadith".to_owned())
}

fn fetch_record(prefix: &str, collection: &str, number: u32) -> Result<Value, String> {
    let url = format!("{HADITH_API}{prefix}-{collection}/{number}.json");
    let output = Command::new("/usr/bin/curl")
        .args(["-fLsS", "--max-time", "10", &url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("Could not run curl: {e}"))?;
    record_from_response(&String::from_utf8_lossy(&output.stdout))
}

/// Downloads one random Arabic/English Hadith pair (runs on a worker thread).
pub fn fetch_hadith(collection: &str, number: u32) -> Result<Hadith, String> {
    let arabic = fetch_record("ara", collection, number)?;
    let english = fetch_record("eng", collection, number)?;
    let interp = |value: Option<&Value>| value.map_or_else(|| "undefined".to_owned(), js_string);
    if interp(english.get("hadithnumber")) != interp(arabic.get("hadithnumber")) {
        return Err("Hadith pair mismatch".to_owned());
    }
    let mut canonical = if truthy(english.get("arabicnumber")) {
        js_string(&english["arabicnumber"])
    } else {
        interp(english.get("hadithnumber"))
    };
    if let Some(variant) = VARIANT.captures(&canonical) {
        let suffix: u32 = variant[2].parse().unwrap_or(0);
        let letter = char::from_u32(96 + suffix).unwrap_or('?');
        canonical = format!("{}{letter}", &variant[1]);
    }
    let reference = english.get("reference");
    let reference_number = |key: &str| {
        let value = reference.and_then(|r| r.get(key));
        let number = if truthy(value) { js_number(value.unwrap()) } else { 0.0 };
        if number.is_finite() { number as i64 } else { 0 }
    };
    Ok(Hadith {
        id: format!("{collection}-{canonical}"),
        collection: collection.to_owned(),
        hadith_number: canonical,
        book_number: reference_number("book"),
        in_book_hadith_number: reference_number("hadith"),
        arabic: js_trim(&js_string(&arabic["text"])).to_owned(),
        english: js_trim(&js_string(&english["text"])).to_owned(),
        source: None,
    })
}

pub struct Service {
    paths: Paths,
    events: Sender<Event>,
    settings: Option<Settings>,
    pub dnd: bool,
    bundled_sounds: Vec<String>,

    calendar: Option<Calendar>,
    calendar_ready: bool,
    calendar_error: Option<&'static str>,
    calendar_text: String,
    calendar_signature: FileSignature,
    next_prayer: Option<PrayerEvent>,
    remaining_seconds: i64,
    prayer_rows: Vec<(Prayer, String, String)>,
    timezone_offset: i32,
    previous_tick: i64,
    last_alert_key: String,
    last_reminder_key: String,
    last_third_alert_key: String,

    audio: Option<Child>,
    stopping_sound: bool,
    playing_alert_key: String,
    sound_error: Option<&'static str>,

    fallback: Vec<Hadith>,
    fallback_order: Vec<usize>,
    fallback_position: usize,
    fallback_index: Option<usize>,
    history: Vec<Hadith>,
    hadith_index: i64,
    hadith_loading: bool,
    hadith_source: String,
    pub hadith_retry_at: Option<Instant>,
}

impl Service {
    pub fn new(paths: Paths, events: Sender<Event>) -> Service {
        Service {
            bundled_sounds: bundled_sounds(&paths.plugin_dir),
            paths,
            events,
            settings: None,
            dnd: false,
            calendar: None,
            calendar_ready: false,
            calendar_error: None,
            calendar_text: String::new(),
            calendar_signature: None,
            next_prayer: None,
            remaining_seconds: 0,
            prayer_rows: Vec::new(),
            timezone_offset: timezone_offset_minutes(now_ms()),
            previous_tick: 0,
            last_alert_key: String::new(),
            last_reminder_key: String::new(),
            last_third_alert_key: String::new(),
            audio: None,
            stopping_sound: false,
            playing_alert_key: String::new(),
            sound_error: None,
            fallback: Vec::new(),
            fallback_order: Vec::new(),
            fallback_position: 0,
            fallback_index: None,
            history: Vec::new(),
            hadith_index: -1,
            hadith_loading: false,
            hadith_source: "offline".to_owned(),
            hadith_retry_at: None,
        }
    }

    pub fn initialized(&self) -> bool {
        self.settings.is_some()
    }

    /// First `settings` command: load persisted state, Hadith sources, and the calendar.
    pub fn init(&mut self, raw: &Map<String, Value>) {
        self.load_state();
        self.load_cached_hadith();
        self.load_fallback_hadiths();
        self.apply_settings(raw);
        self.update_clock(true);
    }

    pub fn handle(&mut self, command: Cmd) {
        match command {
            Cmd::Settings { settings } => self.apply_settings(&settings),
            Cmd::Dnd { value } => self.dnd = value,
            Cmd::ReloadCalendar => self.reload_calendar(),
            Cmd::RefreshClock => self.update_clock(false),
            Cmd::NextHadith => self.next_hadith(),
            Cmd::PreviousHadith => self.previous_hadith(),
            Cmd::ShowFallback => self.show_fallback(),
            Cmd::RequestHadith => self.request_hadith(),
            Cmd::TestNotification => self.test_notification(),
            Cmd::TestSound => {
                self.play_sound("");
            }
            Cmd::StopSound => self.stop_sound(),
            Cmd::StopPrayerSound { key } => self.stop_prayer_sound(&key),
        }
    }

    /// Clock timer: audio exit, calendar file changes, countdown and alerts.
    pub fn tick(&mut self) {
        self.poll_audio();
        self.check_calendar_file();
        self.update_clock(true);
    }

    fn settings(&self) -> &Settings {
        self.settings.as_ref().expect("settings are applied before use")
    }

    fn lang(&self) -> Lang {
        self.settings.as_ref().map_or(Lang::En, |s| s.language)
    }

    fn tr(&self, key: &'static str) -> &'static str {
        translations::text(key, self.lang())
    }

    fn digits(&self, text: &str) -> String {
        localized_digits(text, self.lang().is_arabic())
    }

    pub fn apply_settings(&mut self, raw: &Map<String, Value>) {
        let next = Settings::from_raw(raw, &self.paths.plugin_dir, &self.paths.home);
        let calendar_changed = self.settings.as_ref().is_none_or(|s| s.calendar_path != next.calendar_path);
        self.settings = Some(next);
        if calendar_changed {
            let empty = self.settings().calendar_path.is_empty();
            self.clear_calendar(if empty { Some("noCalendar") } else { None });
            if !empty {
                self.reload_calendar();
            }
        }
    }

    // ---- calendar -------------------------------------------------------

    fn clear_calendar(&mut self, error_key: Option<&'static str>) {
        self.calendar = None;
        self.calendar_ready = false;
        self.calendar_error = error_key;
        self.prayer_rows.clear();
        self.next_prayer = None;
        self.remaining_seconds = 0;
    }

    pub fn reload_calendar(&mut self) {
        if self.settings().calendar_path.is_empty() {
            self.clear_calendar(Some("noCalendar"));
            return;
        }
        let path = self.settings().effective_calendar_path.clone();
        self.calendar_signature = file_signature(&path);
        match fs::read(&path) {
            Ok(bytes) => {
                self.calendar_text = String::from_utf8_lossy(&bytes).into_owned();
                self.load_calendar();
            }
            Err(_) => self.clear_calendar(Some("fileUnavailable")),
        }
    }

    fn check_calendar_file(&mut self) {
        if self.settings().calendar_path.is_empty() {
            return;
        }
        if file_signature(&self.settings().effective_calendar_path) != self.calendar_signature {
            self.reload_calendar();
        }
    }

    fn load_calendar(&mut self) {
        match model::parse_calendar(&self.calendar_text) {
            Ok(calendar) => {
                self.calendar = Some(calendar);
                self.calendar_ready = true;
                self.calendar_error = None;
                self.previous_tick = now_ms();
                self.update_clock(false);
            }
            Err(error) => {
                eprintln!("Dulook calendar validation failed: Error: {error}");
                self.clear_calendar(Some("invalidCalendar"));
            }
        }
    }

    pub fn update_clock(&mut self, check_alerts: bool) {
        let now = now_ms();
        let current_offset = timezone_offset_minutes(now);
        if !self.calendar_ready || self.calendar.is_none() {
            self.timezone_offset = current_offset;
            self.previous_tick = now;
            return;
        }
        if current_offset != self.timezone_offset {
            self.timezone_offset = current_offset;
            self.previous_tick = now;
            self.load_calendar();
            return;
        }

        let Some(calendar) = self.calendar.as_ref() else { return };
        let snapshot = model::snapshot(calendar, now);
        let display_date = snapshot.next.as_ref().map_or_else(|| snapshot.date_key.clone(), |n| n.prayer_date.clone());
        let display_prayers: Option<&PrayerTimes> =
            calendar.dates.get(&display_date).or_else(|| calendar.dates.get(&snapshot.date_key));
        self.prayer_rows = display_prayers
            .map(|prayers| {
                Prayer::ALL.iter().map(|&p| (p, prayers.get(p).to_owned(), display_date.clone())).collect()
            })
            .unwrap_or_default();
        self.remaining_seconds = snapshot.remaining_seconds;
        self.next_prayer = snapshot.next;

        if check_alerts {
            let lower = if self.previous_tick > 0 { self.previous_tick } else { now - ALERT_GRACE_MS };
            let due = model::due_events(calendar, lower, now, ALERT_GRACE_MS, 0);
            let reminders = model::due_events(calendar, lower, now, ALERT_GRACE_MS, PRAYER_REMINDER_LEAD_MS);
            let last_third = model::due_last_third_events(calendar, lower, now, ALERT_GRACE_MS);
            for event in &due {
                self.trigger_prayer(event);
            }
            for event in &reminders {
                self.trigger_prayer_reminder(event);
            }
            for event in &last_third {
                self.trigger_last_third_reminder(&event.key);
            }
        }
        self.previous_tick = now;
    }

    // ---- alerts ---------------------------------------------------------

    fn trigger_prayer(&mut self, event: &PrayerEvent) {
        if event.key == self.last_alert_key {
            return;
        }
        self.last_alert_key = event.key.clone();
        self.save_state();
        let settings = self.settings();
        if !settings.notify[event.name as usize] {
            return;
        }
        let wants_sound = settings.sound_enabled && (!self.dnd || settings.bypass_dnd);
        let notify = settings.prayer_time_notifications_enabled;
        let adhan_started = wants_sound && self.play_sound(&event.key);
        if notify {
            self.send_prayer_notification(event.name, &event.key, adhan_started);
        }
    }

    fn trigger_prayer_reminder(&mut self, event: &PrayerEvent) {
        if event.key == self.last_reminder_key {
            return;
        }
        self.last_reminder_key = event.key.clone();
        self.save_state();
        let settings = self.settings();
        if settings.prayer_reminders_enabled && settings.remind[event.name as usize] {
            self.send_reminder_notification(event.name);
        }
    }

    fn trigger_last_third_reminder(&mut self, key: &str) {
        if key == self.last_third_alert_key {
            return;
        }
        self.last_third_alert_key = key.to_owned();
        self.save_state();
        if self.settings().last_third_reminder_enabled {
            self.send_last_third_notification();
        }
    }

    pub fn notification_command(&self, title: &str, body: &str, click_key: Option<&str>) -> Vec<String> {
        let omarchy = &self.paths.omarchy_path;
        let mut command = vec![format!("{omarchy}/bin/omarchy-notification-send")];
        if !self.settings().bypass_dnd {
            command.extend(["--app-name".to_owned(), "karem.dulook".to_owned()]);
        }
        command.extend(["-u", "normal", "-t", "8000", "-g", "☾"].map(str::to_owned));
        command.extend([title.to_owned(), body.to_owned()]);
        if let Some(key) = click_key {
            command.extend([
                "--exec".to_owned(),
                format!("{omarchy}/bin/omarchy-shell"),
                "-q".to_owned(),
                "karem.dulook".to_owned(),
                "stopPrayerSound".to_owned(),
                key.to_owned(),
            ]);
        }
        command
    }

    fn prayer_body(&self, key: &'static str, prayer: Prayer) -> String {
        translations::interpolate(key, self.lang(), &[("prayer", self.tr(prayer.name()))])
    }

    fn send_prayer_notification(&self, prayer: Prayer, key: &str, adhan_started: bool) {
        let mut body = self.prayer_body("notificationBody", prayer);
        if adhan_started {
            body.push(' ');
            body.push_str(self.tr("stopAdhanHint"));
        }
        exec_detached(&self.notification_command(self.tr("notificationTitle"), &body, adhan_started.then_some(key)));
    }

    fn test_notification(&self) {
        exec_detached(&self.notification_command(self.tr("notificationTitle"), self.tr("testNotificationBody"), None));
    }

    fn send_reminder_notification(&self, prayer: Prayer) {
        let body = self.prayer_body("prayerReminderBody", prayer);
        exec_detached(&self.notification_command(self.tr("prayerReminderTitle"), &body, None));
    }

    fn send_last_third_notification(&self) {
        exec_detached(&self.notification_command(self.tr("lastThirdTitle"), self.tr("lastThirdBody"), None));
    }

    // ---- audio ----------------------------------------------------------

    /// Starts the Adhan; returns false when nothing was started.
    pub fn play_sound(&mut self, alert_key: &str) -> bool {
        self.poll_audio();
        self.sound_error = None;
        let path = self.settings().effective_sound_path.clone();
        if path.is_empty() || self.audio.is_some() {
            return false;
        }
        self.stopping_sound = false;
        let mut command = Command::new("/usr/bin/mpv");
        command
            .args(["--no-config", "--no-load-scripts", "--no-video", "--no-terminal", "--really-quiet"])
            .arg("--audio-client-name=karem.dulook")
            .arg(format!("--volume={}", self.settings().sound_volume))
            .arg("--")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: prctl is async-signal-safe; mpv must not outlive dulook.
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        match command.spawn() {
            Ok(child) => {
                self.audio = Some(child);
                self.playing_alert_key = alert_key.to_owned();
                true
            }
            Err(error) => {
                eprintln!("Dulook: could not start mpv: {error}");
                self.sound_error = Some("soundPlaybackFailed");
                false
            }
        }
    }

    fn on_audio_exit(&mut self, status: Option<ExitStatus>) {
        if !status.is_some_and(|s| s.success()) && !self.stopping_sound {
            self.sound_error = Some("soundPlaybackFailed");
        }
        self.stopping_sound = false;
        self.playing_alert_key.clear();
        self.audio = None;
    }

    pub fn poll_audio(&mut self) {
        let Some(child) = self.audio.as_mut() else { return };
        match child.try_wait() {
            Ok(Some(status)) => self.on_audio_exit(Some(status)),
            Ok(None) => {}
            Err(_) => self.on_audio_exit(None),
        }
    }

    pub fn stop_sound(&mut self) {
        if let Some(child) = self.audio.as_mut() {
            self.stopping_sound = true;
            // SAFETY: plain kill(2) on our own child's pid.
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
            }
            let status = child.wait().ok();
            self.on_audio_exit(status);
        }
        self.playing_alert_key.clear();
    }

    fn stop_prayer_sound(&mut self, key: &str) {
        self.poll_audio();
        if key == self.playing_alert_key {
            self.stop_sound();
        }
    }

    // ---- hadith ---------------------------------------------------------

    fn current_hadith(&self) -> Option<&Hadith> {
        usize::try_from(self.hadith_index).ok().and_then(|i| self.history.get(i))
    }

    fn append_hadith(&mut self, mut item: Hadith, source: &str) {
        item.source = Some(source.to_owned());
        let keep = usize::try_from(self.hadith_index + 1).unwrap_or(0).min(self.history.len());
        self.history.truncate(keep);
        self.history.push(item);
        if self.history.len() > HADITH_HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.hadith_index = self.history.len() as i64 - 1;
        self.hadith_source = source.to_owned();
    }

    pub fn show_fallback(&mut self) {
        if self.fallback.is_empty() {
            return;
        }
        if self.fallback_position >= self.fallback_order.len() {
            self.fallback_order = model::shuffled_indices(self.fallback.len(), self.fallback_index, fastrand::f64);
            self.fallback_position = 0;
        }
        let index = self.fallback_order[self.fallback_position];
        self.fallback_position += 1;
        self.fallback_index = Some(index);
        self.append_hadith(self.fallback[index].clone(), "offline");
    }

    pub fn request_hadith(&mut self) {
        if self.hadith_loading {
            return;
        }
        self.hadith_loading = true;
        let collection = if fastrand::f64() < 0.5 { "bukhari" } else { "muslim" };
        let number = (fastrand::f64() * 7563.0).floor() as u32 + 1;
        let events = self.events.clone();
        std::thread::spawn(move || {
            let _ = events.send(Event::HadithFinished(fetch_hadith(collection, number)));
        });
    }

    pub fn finish_hadith(&mut self, result: Result<Hadith, String>) {
        match result {
            Ok(item) => {
                match serde_json::to_string(&item) {
                    Ok(text) => write_atomic(&self.paths.cache_file, &text),
                    Err(error) => eprintln!("Dulook: could not cache Hadith: {error}"),
                }
                self.append_hadith(item, "online");
                self.hadith_loading = false;
            }
            Err(error) => {
                eprintln!("Dulook: Hadith request failed: {error}");
                self.hadith_loading = false;
                self.show_fallback();
                self.hadith_retry_at = Some(Instant::now() + HADITH_RETRY);
            }
        }
    }

    fn sync_hadith_source(&mut self) {
        if let Some(source) = self.current_hadith().and_then(|h| h.source.clone()) {
            self.hadith_source = source;
        }
    }

    fn next_hadith(&mut self) {
        if self.hadith_index < self.history.len() as i64 - 1 {
            self.hadith_index += 1;
            self.sync_hadith_source();
            return;
        }
        self.request_hadith();
    }

    fn previous_hadith(&mut self) {
        if self.hadith_index > 0 {
            self.hadith_index -= 1;
            self.sync_hadith_source();
        }
    }

    fn load_fallback_hadiths(&mut self) {
        let path = self.paths.plugin_dir.join("data/hadith-fallback.json");
        let parsed = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<FallbackFile>(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(file) => {
                self.fallback = file.hadiths;
                if self.current_hadith().is_none() {
                    self.show_fallback();
                }
                self.request_hadith();
            }
            Err(error) => eprintln!("Dulook: could not load {}: {error}", path.display()),
        }
    }

    fn load_cached_hadith(&mut self) {
        let Ok(text) = fs::read_to_string(&self.paths.cache_file) else { return };
        if let Ok(item) = serde_json::from_str::<Hadith>(&text)
            && !item.arabic.is_empty()
            && !item.english.is_empty()
        {
            self.append_hadith(item, "cached");
        }
    }

    fn load_state(&mut self) {
        let Ok(text) = fs::read_to_string(&self.paths.state_file) else { return };
        if let Ok(state) = serde_json::from_str::<StateFile>(&text) {
            self.last_alert_key = state.last_alert_key;
            self.last_reminder_key = state.last_reminder_key;
            self.last_third_alert_key = state.last_third_alert_key;
        }
    }

    fn save_state(&self) {
        let state = StateFile {
            version: 1,
            last_alert_key: self.last_alert_key.clone(),
            last_reminder_key: self.last_reminder_key.clone(),
            last_third_alert_key: self.last_third_alert_key.clone(),
        };
        match serde_json::to_string(&state) {
            Ok(text) => write_atomic(&self.paths.state_file, &(text + "\n")),
            Err(error) => eprintln!("Dulook: could not save state: {error}"),
        }
    }

    // ---- payloads -------------------------------------------------------

    fn reference_label(hadith: &Hadith, lang: Lang) -> String {
        let arabic = lang.is_arabic();
        let digits = |text: &str| localized_digits(text, arabic);
        format!(
            "{} {} · {}{} · {}{}",
            translations::text(&hadith.collection, lang),
            digits(&hadith.hadith_number),
            if arabic { "الكتاب " } else { "Book " },
            digits(&hadith.book_number.to_string()),
            if arabic { "الحديث " } else { "Hadith " },
            digits(&hadith.in_book_hadith_number.to_string()),
        )
    }

    fn sound_choice(&self) -> String {
        let settings = self.settings();
        if settings.sound_path.is_empty() {
            return "custom".to_owned();
        }
        let selected = &settings.effective_sound_path;
        let selected_canonical = fs::canonicalize(selected).ok();
        self.bundled_sounds
            .iter()
            .find(|bundled| {
                let path = expand_path(bundled, &self.paths.plugin_dir, &self.paths.home);
                path == *selected || (selected_canonical.is_some() && fs::canonicalize(&path).ok() == selected_canonical)
            })
            .cloned()
            .unwrap_or_else(|| "custom".to_owned())
    }

    fn per_prayer(flags: &[bool; 5]) -> Value {
        Value::Object(Prayer::ALL.iter().map(|p| (p.name().to_owned(), Value::Bool(flags[*p as usize]))).collect())
    }

    pub fn state_payload(&self) -> Value {
        let settings = self.settings();
        let lang = settings.language;
        let coverage = match (&self.calendar, self.calendar_ready) {
            (Some(calendar), true) => format!(
                "{} {}",
                self.digits(&format!("{} — {} · {}", calendar.first_date(), calendar.last_date(), calendar.row_count())),
                self.tr("rows")
            ),
            _ => String::new(),
        };
        let current = self.current_hadith().map(|hadith| {
            let mut value = serde_json::to_value(hadith).unwrap_or(Value::Null);
            if let Value::Object(map) = &mut value {
                map.insert("referenceLabel".into(), json!(Self::reference_label(hadith, settings.hadith_language)));
            }
            value
        });
        let hadith_source = self.current_hadith().and_then(|h| h.source.clone()).unwrap_or_else(|| self.hadith_source.clone());
        let bundled: Vec<Value> = self
            .bundled_sounds
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let number = self.digits(&(index + 1).to_string());
                json!({ "value": value, "label": translations::interpolate("adhanOption", lang, &[("number", &number)]) })
            })
            .collect();

        json!({
            "type": "state",
            "language": lang.code(),
            "hadithLanguage": settings.hadith_language.code(),
            "calendarPath": settings.calendar_path,
            "prayerTimeNotificationsEnabled": settings.prayer_time_notifications_enabled,
            "prayerRemindersEnabled": settings.prayer_reminders_enabled,
            "lastThirdReminderEnabled": settings.last_third_reminder_enabled,
            "bypassDnd": settings.bypass_dnd,
            "soundEnabled": settings.sound_enabled,
            "soundPath": settings.sound_path,
            "soundVolume": settings.sound_volume,
            "arabicUiFont": settings.arabic_ui_font,
            "arabicHadithFont": settings.arabic_hadith_font,
            "notifyEnabled": Self::per_prayer(&settings.notify),
            "remindEnabled": Self::per_prayer(&settings.remind),
            "strings": translations::strings(lang),
            "calendarReady": self.calendar_ready,
            "calendarError": self.calendar_error.map_or("", |key| self.tr(key)),
            "coverage": coverage,
            "soundError": self.sound_error.map_or("", |key| self.tr(key)),
            "soundPlaying": self.audio.is_some(),
            "currentHadith": current,
            "hadithIndex": self.hadith_index,
            "hadithLoading": self.hadith_loading,
            "hadithSource": hadith_source,
            "fallbackCount": self.fallback.len(),
            "bundledSounds": bundled,
            "soundChoice": self.sound_choice(),
        })
    }

    fn timezone_label(&self) -> String {
        let offset = -timezone_offset_minutes(now_ms());
        let sign = if offset >= 0 { '+' } else { '-' };
        let absolute = i64::from(offset.abs());
        self.digits(&format!("UTC{sign}{}:{}", pad2(absolute / 60), pad2(absolute % 60)))
    }

    fn time_label(&self, time: &str) -> String {
        self.digits(&model::format_time_12_hour(time))
    }

    pub fn tick_payload(&self) -> Value {
        let lang = self.lang();
        let (countdown, bar_label, next) = match &self.next_prayer {
            Some(next) => {
                let countdown = self.digits(&model::format_countdown(self.remaining_seconds));
                let prayer = self.tr(next.name.name());
                let label = translations::interpolate("countdownToPrayer", lang, &[("countdown", &countdown), ("prayer", prayer)]);
                let mut value = serde_json::to_value(next).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut value {
                    map.insert("label".into(), json!(format!("{prayer} · {}", self.time_label(&next.time))));
                }
                (countdown, label, value)
            }
            None => ("--:--:--".to_owned(), self.tr("appName").to_owned(), Value::Null),
        };
        let rows: Vec<Value> = self
            .prayer_rows
            .iter()
            .map(|(prayer, time, date)| {
                json!({
                    "name": prayer.name(),
                    "prayerDate": date,
                    "time": time,
                    "nameLabel": self.tr(prayer.name()),
                    "timeLabel": self.time_label(time),
                })
            })
            .collect();
        json!({
            "type": "tick",
            "countdown": countdown,
            "barLabel": bar_label,
            "timezoneLabel": self.timezone_label(),
            "nextPrayer": next,
            "prayerRows": rows,
        })
    }
}
