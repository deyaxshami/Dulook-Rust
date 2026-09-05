import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function loadScript(relativePath) {
  const context = { Date, Math, Number, String, Error, isFinite };
  vm.createContext(context);
  vm.runInContext(readFileSync(new URL(relativePath, import.meta.url), "utf8"), context);
  return context;
}

const Model = loadScript("../Model.js");
const Translations = loadScript("../Translations.js");

const englishCsv = `Day,Date,Hijri,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha
Monday,01/06/2026,,03:00 am,,01:15 pm,05:30 pm,10:00 pm,12:10 am
Tuesday,02/06/2026,,02:59 am,,01:15 pm,05:31 pm,10:01 pm,12:11 am
Wednesday,03/06/2026,,02:58 am,,01:15 pm,05:32 pm,10:02 pm,12:12 am`;

test("parses English headers, quoted CSV fields, and overnight times", () => {
  const calendar = Model.parseCalendar(englishCsv);
  assert.equal(calendar.rowCount, 3);
  assert.equal(calendar.firstDate, "2026-06-01");
  assert.equal(calendar.dates["2026-06-01"].Dhuhr, "13:15");
  assert.equal(calendar.dates["2026-06-01"].Isha, "00:10");
  const events = Model.eventsForRow("2026-06-01", calendar.dates["2026-06-01"]);
  assert.equal(new Date(events[4].timestamp).getDate(), 2);
});

test("parses Arabic headers and ISO dates", () => {
  const calendar = Model.parseCalendar("التاريخ,الفجر,الظهر,العصر,المغرب,العشاء\n2026-09-03,05:27,13:36,17:11,20:17,21:34");
  assert.equal(calendar.dates["2026-09-03"].Fajr, "05:27");
  assert.equal(calendar.dates["2026-09-03"].Isha, "21:34");
});

test("uses tomorrow's actual Fajr after the final prayer", () => {
  const calendar = Model.parseCalendar(englishCsv);
  const now = new Date(2026, 5, 2, 0, 10, 30).getTime();
  const state = Model.snapshot(calendar, now);
  assert.equal(state.next.name, "Fajr");
  assert.equal(state.next.prayerDate, "2026-06-02");
  assert.equal(state.next.time, "02:59");
});

test("returns prayer events crossed during a wake grace window", () => {
  const calendar = Model.parseCalendar(englishCsv);
  const now = new Date(2026, 5, 1, 13, 16, 0).getTime();
  const due = Model.dueEvents(calendar, now - 120000, now, 120000);
  assert.deepEqual(Array.from(due, event => event.name), ["Dhuhr"]);
});

test("rejects missing columns, duplicate dates, and invalid times", () => {
  assert.throws(() => Model.parseCalendar("Date,Fajr\n01/01/2026,05:00"), /Missing column/);
  assert.throws(() => Model.parseCalendar("Date,Fajr,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,12:00,15:00,18:00,20:00\n01/01/2026,05:00,12:00,15:00,18:00,20:00"), /Duplicate date/);
  assert.throws(() => Model.parseCalendar("Date,Fajr,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,25:00,12:00,15:00,18:00,20:00"), /Invalid time/);
  assert.throws(() => Model.parseCalendar("Date,Fajr,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,01:00,15:00,18:00,20:00"), /Invalid prayer order/);
  assert.throws(() => Model.parseCalendar("Date,Fajr,Dhuhr,Asr,Maghrib,Isha\n01/01/2026,05:00,12:00,15:00,20:00,18:00\n02/01/2026,05:00,12:00,15:00,20:00,22:00"), /Invalid overnight Isha/);
});

test("formats prayer times with a 12-hour clock", () => {
  assert.equal(Model.formatTime12Hour("00:10"), "12:10 AM");
  assert.equal(Model.formatTime12Hour("05:27"), "5:27 AM");
  assert.equal(Model.formatTime12Hour("12:00"), "12:00 PM");
  assert.equal(Model.formatTime12Hour("21:34"), "9:34 PM");
});

test("formats prayer countdowns for the bar", () => {
  assert.equal(Model.formatCountdown(9030), "02:30:30");
  assert.equal(Model.formatCountdown(0), "00:00:00");
});

test("translation dictionaries have complete matching keys", () => {
  assert.deepEqual(Object.keys(Translations.STRINGS.ar).sort(), Object.keys(Translations.STRINGS.en).sort());
  assert.equal(Translations.text("settings", "en"), "Settings");
  assert.equal(Translations.text("settings", "ar"), "الإعدادات");
  assert.equal(Translations.interpolate("notificationBody", "en", { prayer: "Fajr" }), "It is time for Fajr.");
  assert.equal(Translations.interpolate("countdownToPrayer", "en", { countdown: "02:30:30", prayer: "Fajr" }), "02:30:30 to Fajr");
});

test("fallback selection does not immediately repeat", () => {
  assert.equal(Model.nextFallbackIndex(100, 25, 0.25), 26);
  assert.equal(Model.nextFallbackIndex(1, 0, 0), 0);
  assert.equal(Model.nextFallbackIndex(0, -1, 0), -1);
});

test("fallback shuffle visits every record once and avoids a cycle-boundary repeat", () => {
  const order = Array.from(Model.shuffledIndices(100, 99, Array(99).fill(0)));
  assert.equal(order.length, 100);
  assert.equal(new Set(order).size, 100);
  assert.notEqual(order[0], 99);
  assert.deepEqual([...order].sort((left, right) => left - right), Array.from({ length: 100 }, (_, index) => index));
});
