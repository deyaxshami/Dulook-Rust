use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("dulook-calendar-{}-{:016x}", std::process::id(), fastrand::u64(..)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Backend {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
    request_id: u64,
}

impl Backend {
    fn start(scratch: &Scratch) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dulook"))
            .args(["--plugin-dir", env!("CARGO_MANIFEST_DIR"), "--omarchy-path", "/usr/share/omarchy"])
            .arg("--cache-file").arg(scratch.0.join("hadith.json"))
            .arg("--state-file").arg(scratch.0.join("state.json"))
            .env("TZ", "Europe/Berlin")
            .env("HOME", &scratch.0)
            .env("XDG_DATA_HOME", scratch.0.join("data"))
            .env("XDG_CACHE_HOME", scratch.0.join("cache"))
            .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, messages) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                if tx.send(serde_json::from_str(&line).expect("backend writes JSON lines")).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, messages, reader: Some(reader), request_id: 0 }
    }

    fn send(&mut self, command: Value) {
        writeln!(self.stdin, "{command}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn receive(&self, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let message = self.messages.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("expected backend reply within 5 seconds");
            if predicate(&message) {
                return message;
            }
        }
    }

    fn select(&mut self, location: &str, path: Option<&Path>) {
        self.send(json!({"cmd":"settings", "settings": {
            "location": location, "calendarPath": path, "soundEnabled": false,
            "prayerTimeNotificationsEnabled": false, "prayerRemindersEnabled": false, "lastThirdReminderEnabled": false
        }}));
    }

    fn browse(&mut self, selection: Value) -> Value {
        self.request_id += 1;
        self.send(json!({"cmd":"browseCalendar", "requestId":self.request_id, "selection":selection}));
        self.receive(|message| message["type"] == "calendarBrowser" && message["requestId"] == self.request_id)
    }

    fn open(&mut self) -> Value {
        let reply = self.browse(json!({"view":"open"}));
        assert_eq!(reply["view"], "open");
        reply
    }

    fn month(&mut self, basis: &str, year: i32, month: u8) -> Value {
        let reply = self.browse(json!({"view":"month", "basis":basis, "year":year, "month":month}));
        assert_eq!(reply["view"], "month");
        reply
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.reader.take().unwrap().join();
    }
}

fn years(reply: &Value, key: &str) -> Vec<i64> {
    assert!(reply.get("error").is_some_and(Value::is_null), "{reply}");
    reply[key].as_array().unwrap().iter().map(|year| year["year"].as_i64().unwrap()).collect()
}

fn rows(reply: &Value) -> &[Value] {
    assert!(reply.get("error").is_some_and(Value::is_null), "{reply}");
    reply["rows"].as_array().unwrap()
}

#[test]
fn bundled_ramadan_crosses_gregorian_files_and_leap_years_are_browsable() {
    let scratch = Scratch::new();
    let mut backend = Backend::start(&scratch);
    backend.select("nordhorn", None);
    let initial = backend.receive(|message| message["type"] == "state");
    let revision = initial["calendarRevision"].as_u64().unwrap();
    assert!(revision > 0);

    let catalog = backend.open();
    assert_eq!(years(&catalog, "gregorianYears"), (2026..=2037).collect::<Vec<_>>());
    let ramadan = &catalog["hijriYears"].as_array().unwrap().iter()
        .find(|year| year["year"] == 1452).unwrap()["months"][8];
    assert_eq!(ramadan, &json!({"otherStart":"2030-12-26", "otherEnd":"2031-01-24"}));

    let reply = backend.month("hijri", 1452, 9);
    let days = rows(&reply);
    assert_eq!(days.len(), 30);
    assert_eq!(days[0]["gregorianDate"], "2030-12-26");
    assert_eq!(days[0]["hijriDate"], "1452-09-01");
    assert_eq!(days[29]["gregorianDate"], "2031-01-24");
    assert_eq!(days[29]["hijriDate"], "1452-09-30");
    assert!(days.windows(2).all(|pair| pair[0]["gregorianDate"].as_str() < pair[1]["gregorianDate"].as_str()));
    let times = days.iter().find(|day| day["gregorianDate"] == "2030-12-31").unwrap()["times"].as_array().unwrap();
    assert_eq!(times.len(), 8);
    assert_eq!(times[0], "07:17");
    assert_eq!(times[5], "16:31");
    assert_eq!(times[7], "23:54");
    let clock = |index: usize| times[index].as_str().unwrap();
    assert!(clock(3) < clock(4) && clock(4) < clock(5));

    let february = backend.month("gregorian", 2032, 2);
    assert_eq!(rows(&february).len(), 29);
    assert_eq!(rows(&february)[28]["gregorianDate"], "2032-02-29");
    assert!(rows(&backend.month("gregorian", 2038, 1)).is_empty());
    assert!(rows(&backend.month("gregorian", 2032, 13)).is_empty());

    backend.send(json!({"cmd":"reloadCalendar"}));
    let reloaded = backend.receive(|message| message["type"] == "state" && message["calendarRevision"].as_u64() > Some(revision));
    assert_eq!(reloaded["calendarRevision"], revision + 1);
    assert_eq!(backend.month("gregorian", 2032, 2)["error"], "calendarBrowserReload");
}

#[test]
fn installed_downloads_use_only_canonical_files_and_keep_a_read_only_snapshot() {
    let scratch = Scratch::new();
    let directory = "data/dulook/calendars/TEST";
    let csv = "Date,Hijri,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n2041-02-01,29/02/1463,06:00,07:00,12:00,15:00,18:00,20:00\n";
    scratch.write(&format!("{directory}/location.json"), r#"{"country":"DE","city":"Test","zip":"","latitude":52.43,"longitude":7.07,"timezone":"Europe/Berlin"}"#);
    scratch.write(&format!("{directory}/TEST_2041.csv"), csv);
    scratch.write(&format!("{directory}/TEST_2042.csv.bak"), &csv.replace("2041", "2042"));
    scratch.write(&format!("{directory}/TEST_2044.csv"), "File unavailable to parse");
    let mut backend = Backend::start(&scratch);
    backend.select("downloaded/TEST", None);

    let catalog = backend.open();
    assert_eq!(years(&catalog, "gregorianYears"), [2041]);
    assert_eq!(years(&catalog, "hijriYears"), [1463]);
    assert_eq!(catalog["warning"], "calendarBrowserSomeFilesUnavailable");
    scratch.write(&format!("{directory}/TEST_2045.csv"), csv);
    let reply = backend.month("hijri", 1463, 2);
    let days = rows(&reply);
    assert_eq!(days.len(), 1);
    assert_eq!(days[0]["gregorianDate"], "2041-02-01");
    assert_eq!(days[0]["hijriDate"], "1463-02-29");
    assert!(days[0]["times"][7].is_null());
    assert_eq!(backend.open()["error"], "invalidCalendar"); // Duplicate across files on explicit reopen.
}

#[test]
fn custom_calendar_keeps_prayer_rows_without_hijri_metadata() {
    let scratch = Scratch::new();
    let mut backend = Backend::start(&scratch);
    backend.select("nordhorn", None);
    assert_eq!(years(&backend.open(), "gregorianYears").first(), Some(&2026));

    for (name, csv) in [
        ("no-hijri.csv", "Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n2040-06-01,06:00,07:00,12:00,15:00,18:00,20:00\n"),
        ("invalid-hijri.csv", "Date,Hijri,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n2040-06-01,31/09/1452,06:00,07:00,12:00,15:00,18:00,20:00\n"),
    ] {
        let path = scratch.write(name, csv);
        backend.select("custom", Some(&path));
        let catalog = backend.open();
        assert_eq!(years(&catalog, "gregorianYears"), [2040]);
        assert!(years(&catalog, "hijriYears").is_empty());
        assert!(catalog["defaultHijri"].is_null());
        let reply = backend.month("gregorian", 2040, 6);
        let days = rows(&reply);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0]["gregorianDate"], "2040-06-01");
        assert_eq!(days[0]["times"][0], "06:00");
        assert!(days[0]["hijriDate"].is_null());
        assert!(days[0]["times"][7].is_null());
    }
}

#[test]
fn hijri_month_filters_unpaired_days_between_its_endpoints() {
    let scratch = Scratch::new();
    let path = scratch.write("paired.csv", "Date,التاريخ الهجري,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n\
        2040-06-01,٠١/٠٩/١٤٦٢ (Ram),06:00,07:00,12:00,15:00,18:00,20:00\n\
        2040-06-02,,06:00,07:00,12:00,15:00,18:00,20:00\n\
        2040-06-03,1462-09-03,06:00,07:00,12:00,15:00,18:00,20:00\n");
    let mut backend = Backend::start(&scratch);
    backend.select("custom", Some(&path));
    let catalog = backend.open();
    assert_eq!(catalog["gregorianYears"][0]["months"][5], json!({"otherStart":"1462-09-01", "otherEnd":"1462-09-03"}));
    let reply = backend.month("hijri", 1462, 9);
    let dates: Vec<_> = rows(&reply).iter().map(|day| day["hijriDate"].as_str().unwrap()).collect();
    assert_eq!(dates, ["1462-09-01", "1462-09-03"]);
    assert_eq!(rows(&reply)[0]["times"][7], "00:00"); // The unpaired next day still supplies Fajr.
}
