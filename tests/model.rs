use std::collections::BTreeSet;
use std::path::Path;

use chrono::{Datelike, Local, TimeZone};
use dulook::locations;
use dulook::model::{self, Calendar, Prayer};
use dulook::translations::{self, AR, EN, Lang};

const ENGLISH_CSV: &str = "Day,Date,Hijri,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha
Monday,01/06/2026,,03:00 am,05:00 am,01:15 pm,05:30 pm,10:00 pm,12:10 am
Tuesday,02/06/2026,,02:59 am,04:59 am,01:15 pm,05:31 pm,10:01 pm,12:11 am
Wednesday,03/06/2026,,02:58 am,04:58 am,01:15 pm,05:32 pm,10:02 pm,12:12 am";

fn local_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    Local.with_ymd_and_hms(year, month, day, hour, minute, second).single().unwrap().timestamp_millis()
}

fn plugin_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn bundled_calendar(location: &str, year: i32) -> Calendar {
    let path = locations::calendar_file(&plugin_dir().join("data"), location, year);
    let csv = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    model::parse_calendar(&csv).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn one_day_calendar(row: &str) -> Calendar {
    model::parse_calendar(&format!("Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n{row}")).unwrap()
}

#[test]
fn parses_english_headers_quoted_fields_and_overnight_times() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    assert_eq!(calendar.row_count(), 3);
    assert_eq!(calendar.first_date(), "2026-06-01");
    let row = &calendar.dates["2026-06-01"];
    assert_eq!(row.get(Prayer::Dhuhr), "13:15");
    assert_eq!(row.get(Prayer::Isha), "00:10");
    let events = model::events_for_row("2026-06-01", row).unwrap();
    assert_eq!(Local.timestamp_millis_opt(events[5].timestamp).unwrap().day(), 2);
}

#[test]
fn parses_arabic_headers_and_iso_dates() {
    let calendar = model::parse_calendar(
        "التاريخ,الفجر,الشروق,الظهر,العصر,المغرب,العشاء\n2026-09-03,05:27,06:58,13:36,17:11,20:17,21:34",
    )
    .unwrap();
    assert_eq!(calendar.dates["2026-09-03"].get(Prayer::Fajr), "05:27");
    assert_eq!(calendar.dates["2026-09-03"].get(Prayer::Sunrise), "06:58");
    assert_eq!(calendar.dates["2026-09-03"].get(Prayer::Isha), "21:34");
}

#[test]
fn manifest_lists_bundled_and_custom_locations_with_complete_years() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../manifest.json")).unwrap();
    let schema = manifest["barWidget"]["schema"].as_array().unwrap();
    let options = schema.iter().find(|entry| entry["key"] == "location").unwrap()["options"].clone();
    let mut expected = locations::bundled_locations(plugin_dir());
    expected.push(locations::CUSTOM.to_owned());
    assert_eq!(options, serde_json::json!(expected));

    for location in locations::bundled_locations(plugin_dir()) {
        for year in 2026..=2037 {
            let calendar = bundled_calendar(&location, year);
            assert_eq!(calendar.first_date(), format!("{year}-01-01"), "{location} {year}");
            assert_eq!(calendar.last_date(), format!("{year}-12-31"), "{location} {year}");
        }
    }
}

#[test]
fn merged_years_carry_the_next_prayer_across_new_year() {
    let december = || one_day_calendar("31/12/2026,07:17 am,08:39 am,12:40 pm,02:12 pm,04:29 pm,05:53 pm");
    let january = one_day_calendar("01/01/2027,07:17 am,08:39 am,12:40 pm,02:13 pm,04:30 pm,05:54 pm");
    let calendar = model::merge_calendars([december(), january]).unwrap();
    let midnight = model::snapshot(&calendar, local_ms(2026, 12, 31, 23, 0, 0)).next.unwrap();
    assert_eq!(midnight.name, Prayer::Midnight);
    assert_eq!(midnight.prayer_date, "2026-12-31");
    assert_eq!(midnight.time, "23:53");
    let next = model::snapshot(&calendar, local_ms(2026, 12, 31, 23, 59, 0)).next.unwrap();
    assert_eq!(next.name, Prayer::Fajr);
    assert_eq!(next.prayer_date, "2027-01-01");
    assert!(model::merge_calendars([december(), december()]).is_err());
}

#[test]
fn bundles_valid_mp3_defaults_and_references_them_from_the_manifest() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../manifest.json")).unwrap();
    let defaults = &manifest["barWidget"]["defaults"];
    assert_eq!(defaults["location"], locations::DEFAULT_LOCATION);
    assert_eq!(defaults["calendarPath"], "");
    assert_eq!(defaults["soundEnabled"], true);
    assert_eq!(defaults["soundPath"], "adans/adan_04.mp3");
    for name in ["adan_01.mp3", "adan_02.mp3", "adan_03.mp3", "adan_04.mp3"] {
        let audio = std::fs::read(format!("{}/adans/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let has_id3_header = audio.starts_with(b"ID3");
        let has_mpeg_frame = audio[0] == 0xff && (audio[1] & 0xe0) == 0xe0;
        assert!(audio.len() > 1000, "{name} is empty");
        assert!(has_id3_header || has_mpeg_frame, "{name} is not an MP3");
    }
}

#[test]
fn uses_tomorrows_actual_fajr_after_the_final_prayer() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let state = model::snapshot(&calendar, local_ms(2026, 6, 2, 0, 30, 0));
    let next = state.next.unwrap();
    assert_eq!(next.name, Prayer::Fajr);
    assert_eq!(next.prayer_date, "2026-06-02");
    assert_eq!(next.time, "02:59");
}

#[test]
fn computes_islamic_midnight_halfway_from_maghrib_to_the_next_fajr() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let events = model::day_events(&calendar, "2026-06-01");
    assert_eq!(events.iter().map(|e| e.name).collect::<Vec<_>>(), Prayer::ALL);
    let midnight = events.last().unwrap();
    assert_eq!(midnight.time, "00:29");
    assert_eq!(midnight.timestamp, local_ms(2026, 6, 2, 0, 29, 0));
    assert_eq!(midnight.prayer_date, "2026-06-01");
    let last_day = model::day_events(&calendar, "2026-06-03");
    assert_eq!(last_day.len(), 7);
    assert!(last_day.iter().all(|e| e.name != Prayer::Midnight));
}

#[test]
fn counts_down_to_midnight_after_isha() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let next = model::snapshot(&calendar, local_ms(2026, 6, 2, 0, 10, 30)).next.unwrap();
    assert_eq!(next.name, Prayer::Midnight);
    assert_eq!(next.prayer_date, "2026-06-01");
}

const EQUINOX_ONLY_CSV: &str = "Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n20/03/2026,05:00,06:00,12:00,15:00,18:00,19:30";

fn yellowing_time(calendar: &Calendar, key: &str) -> String {
    let events = model::day_events(calendar, key);
    let at = events.iter().position(|e| e.name == Prayer::Yellowing).unwrap();
    assert!(events[at - 1].name == Prayer::Asr && events[at + 1].name == Prayer::Maghrib);
    assert!(events[at - 1].timestamp < events[at].timestamp && events[at].timestamp < events[at + 1].timestamp);
    events[at].time.clone()
}

#[test]
fn estimates_calendar_latitude_from_day_lengths() {
    let nordhorn = bundled_calendar("nordhorn", 2026).latitude.unwrap();
    assert!((nordhorn - 52.43).abs() < 0.2, "{nordhorn}");
    let cairo = bundled_calendar("cairo", 2026).latitude.unwrap();
    assert!((cairo - 30.04).abs() < 0.2, "{cairo}");
    assert_eq!(model::parse_calendar(EQUINOX_ONLY_CSV).unwrap().latitude, None);
}

#[test]
fn yellows_at_two_and_a_half_degrees_for_the_calendars_latitude() {
    assert_eq!(yellowing_time(&bundled_calendar("nordhorn", 2026), "2026-11-23"), "16:01");
    assert_eq!(yellowing_time(&bundled_calendar("cairo", 2026), "2026-11-23"), "16:39");
    assert_eq!(yellowing_time(&model::parse_calendar(ENGLISH_CSV).unwrap(), "2026-06-01"), "21:29");
}

#[test]
fn skips_yellowing_without_a_latitude() {
    let calendar = model::parse_calendar(EQUINOX_ONLY_CSV).unwrap();
    let events = model::day_events(&calendar, "2026-03-20");
    assert_eq!(events.iter().map(|e| e.name).collect::<Vec<_>>(), Prayer::ROW);
}

#[test]
fn counts_down_to_yellowing_after_asr() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let next = model::snapshot(&calendar, local_ms(2026, 6, 1, 18, 0, 0)).next.unwrap();
    assert_eq!(next.name, Prayer::Yellowing);
    assert_eq!(next.prayer_date, "2026-06-01");
}

#[test]
fn notifies_sunrise_like_a_prayer() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let now = local_ms(2026, 6, 1, 5, 0, 30);
    let due = model::due_events(&calendar, now - 120_000, now, 120_000, 0);
    assert_eq!(due.iter().map(|e| e.name).collect::<Vec<_>>(), [Prayer::Sunrise]);
    let reminder_time = local_ms(2026, 6, 1, 4, 45, 0);
    let due = model::due_events(&calendar, reminder_time - 120_000, reminder_time, 120_000, 15 * 60 * 1000);
    assert_eq!(due.iter().map(|e| e.name).collect::<Vec<_>>(), [Prayer::Sunrise]);
}

#[test]
fn returns_prayer_events_crossed_during_a_wake_grace_window() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let now = local_ms(2026, 6, 1, 13, 16, 0);
    let due = model::due_events(&calendar, now - 120_000, now, 120_000, 0);
    assert_eq!(due.iter().map(|e| e.name).collect::<Vec<_>>(), [Prayer::Dhuhr]);
}

#[test]
fn returns_prayer_reminders_15_minutes_before_each_event() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let reminder_time = local_ms(2026, 6, 1, 13, 0, 0);
    let due = model::due_events(&calendar, reminder_time - 2000, reminder_time, 120_000, 15 * 60 * 1000);
    assert_eq!(due.iter().map(|e| e.name).collect::<Vec<_>>(), [Prayer::Dhuhr]);
}

#[test]
fn starts_the_last_third_after_two_thirds_of_the_maghrib_to_fajr_night() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let expected = local_ms(2026, 6, 2, 1, 19, 20);
    let events = model::last_third_events_around(&calendar, expected);
    let event = events.iter().find(|e| e.night_date == "2026-06-01").unwrap();
    assert_eq!(event.timestamp, expected);
    assert_eq!(event.maghrib_timestamp, local_ms(2026, 6, 1, 22, 0, 0));
    assert_eq!(event.fajr_timestamp, local_ms(2026, 6, 2, 2, 59, 0));
}

#[test]
fn returns_the_start_of_the_last_third_when_its_notification_becomes_due() {
    let calendar = model::parse_calendar(ENGLISH_CSV).unwrap();
    let now = local_ms(2026, 6, 2, 1, 19, 20);
    let due = model::due_last_third_events(&calendar, now - 2000, now, 120_000);
    assert_eq!(due.iter().map(|e| e.night_date.as_str()).collect::<Vec<_>>(), ["2026-06-01"]);
}

#[test]
fn rejects_missing_columns_duplicate_dates_and_invalid_times() {
    let cases = [
        ("Date,Fajr\n01/01/2026,05:00", "Missing column"),
        ("Date,Fajr,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,12:00,15:00,18:00,20:00", "Missing column: sunrise"),
        (
            "Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,06:00,12:00,15:00,18:00,20:00\n01/01/2026,05:00,06:00,12:00,15:00,18:00,20:00",
            "Duplicate date",
        ),
        ("Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,25:00,06:00,12:00,15:00,18:00,20:00", "Invalid time"),
        ("Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,,12:00,15:00,18:00,20:00", "Invalid time"),
        ("Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,06:00,01:00,15:00,18:00,20:00", "Invalid prayer order"),
        ("Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,04:00,12:00,15:00,18:00,20:00", "Invalid prayer order"),
        (
            "Date,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,06:00,12:00,15:00,20:00,18:00\n02/01/2026,05:00,06:00,12:00,15:00,20:00,22:00",
            "Invalid overnight Isha",
        ),
    ];
    for (csv, message) in cases {
        let error = model::parse_calendar(csv).unwrap_err();
        assert!(error.contains(message), "{error} does not mention {message}");
    }
}

#[test]
fn formats_prayer_times_with_a_12_hour_clock() {
    assert_eq!(model::format_time_12_hour("00:10"), "12:10 AM");
    assert_eq!(model::format_time_12_hour("05:27"), "5:27 AM");
    assert_eq!(model::format_time_12_hour("12:00"), "12:00 PM");
    assert_eq!(model::format_time_12_hour("21:34"), "9:34 PM");
}

#[test]
fn formats_prayer_countdowns_for_the_bar() {
    assert_eq!(model::format_countdown(9030), "02:30:30");
    assert_eq!(model::format_countdown(0), "00:00:00");
}

#[test]
fn translation_dictionaries_have_complete_matching_keys() {
    let keys = |table: &'static [(&'static str, &'static str)]| table.iter().map(|(k, _)| *k).collect::<BTreeSet<_>>();
    assert_eq!(keys(AR), keys(EN));
    assert_eq!(translations::text("settings", Lang::En), "Settings");
    assert_eq!(translations::text("settings", Lang::Ar), "الإعدادات");
    assert_eq!(translations::interpolate("notificationBody", Lang::En, &[("prayer", "Fajr")]), "It is time for Fajr.");
    assert_eq!(translations::text("stopAdhanHint", Lang::En), "Left-click to stop the Adhan.");
    assert_eq!(translations::interpolate("prayerReminderBody", Lang::En, &[("prayer", "Fajr")]), "Fajr begins in 15 minutes.");
    assert_eq!(translations::interpolate("timeBeginsBody", Lang::En, &[("time", "Sunrise")]), "Sunrise has begun.");
    assert_eq!(translations::text("Yellowing", Lang::Ar), "الاصفرار");
    assert_eq!(translations::text("lastThirdBody", Lang::En), "The last third of the night has begun.");
    assert_eq!(
        translations::interpolate("countdownToPrayer", Lang::En, &[("countdown", "02:30:30"), ("prayer", "Fajr")]),
        "02:30:30 to Fajr"
    );
}

#[test]
fn fallback_shuffle_visits_every_record_once_and_avoids_a_cycle_boundary_repeat() {
    let order = model::shuffled_indices(100, Some(99), || 0.0);
    assert_eq!(order.len(), 100);
    assert_ne!(order[0], 99);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..100).collect::<Vec<_>>());
}
