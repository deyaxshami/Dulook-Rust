# Dulook

Dulook is a bilingual prayer-time and Hadith widget for the Omarchy Shell bar,
with a Rust backend behind a native Quickshell UI. Prayer times come from a
bundled or user-selected CSV file and never require network access. Internet
access is used only to request Arabic and English Hadith; a reviewed set of
100 bilingual Hadith is bundled for offline use.

## Features

- Native Quickshell and Omarchy bar UI
- Live `HH:MM:SS` countdown to the next prayer in the bar
- English interface by default with a complete Arabic mode
- Independent English or Arabic language selection for Hadith content
- System-local timezone and daylight-saving behavior
- Online location (country, city, optional ZIP code) downloaded for this year and the next 10,
  or a calendar bundled under `data/`
- Optional custom CSV with automatic reload on file changes
- Standalone Gregorian and Hijri prayer timetables, with a year overview for every covered year
- Four bundled Adhan recordings
- Native prayer notifications on every monitor, including a reminder 15 minutes before each prayer
- Sunrise, the sun's yellowing (end of the chosen Asr time), and Islamic midnight shown, counted down to, and notified like prayers, without the Adhan
- A notification when the last third of the night begins
- Independent prayer-time, 15-minute, last-third, per-prayer, and DND controls
- Adhan dropdown with four bundled recordings, or your own local MP3, played through `mpv`
- Separate Arabic UI and Hadith font settings
- Exactly 100 paired Arabic and English offline Hadith

## Last Third of the Night

Dulook treats the night as the interval from Maghrib to the following day's
Fajr. It divides that duration into three equal parts and sends the configured
notification when two-thirds have elapsed:

```text
last third begins = Maghrib + 2/3 * (next Fajr - Maghrib)
```

This follows the description in [Sahih al-Bukhari 1145](https://sunnah.com/bukhari:1145)
and the calculation explained by [IslamWeb](https://www.islamweb.net/en/fatwa/376972/determining-the-last-third-part-of-the-night).

## Yellowing of the Sun

Yellowing (اصفرار الشمس) ends the chosen time for Asr. Salafi scholars note that
it varies with the season and latitude, and put it about 15 minutes before
sunset in Saudi latitudes: Dr. Abdullah Al-Misnid of Qassim University
(almisnid.com, article 343) and IslamQA ([175386](https://islamqa.info/ar/answers/175386),
[67911](https://islamqa.info/ar/answers/67911)).

Dulook turns that into a solar altitude: 15 minutes before sunset at Qassim
(26.33°N) at the equinox the sun's centre stands 2.5° high. Yellowing is when
the sun descends to 2.5° at the calendar's own latitude, so it moves with place
and season (about 17–18 minutes before Maghrib in Cairo and 25–32 in
Nordhorn). Calendars carry no coordinates, so the latitude is fitted from the
calendar's own Sunrise→Maghrib day lengths, and solar noon is taken as the
midpoint of Sunrise and Maghrib. A calendar covering only the weeks around an
equinox cannot be fitted and shows no Yellowing.

## Islamic Midnight

Midnight is the midpoint of the same night, truncated to the minute:

```text
midnight = Maghrib + 1/2 * (next Fajr - Maghrib)
```

After Isha, the panel keeps showing that day until midnight passes. On a
calendar's last day, which has no next Fajr, Midnight is not shown.

## Requirements

- Omarchy Shell on x86_64 or aarch64 (ARM) Linux
- `mpv` for Adhan playback and `curl` for online Hadith, calendars and map tiles, at `/usr/bin`

## Install

```bash
omarchy plugin add https://github.com/deyaxshami/Dulook-Rust --enable
```

No build step is needed; prebuilt backends for x86_64 and aarch64 ship in
`bin/`, and `bin/dulook` starts the one matching the machine.

The repository root is the plugin root, so Git-managed installations can be
updated with:

```bash
omarchy plugin update deyaxshami.dulook
omarchy-restart-shell
```

### Upgrading from the old Dulook

The old `Karem-Shami/Dulook` plugin and earlier builds of this repository use
the id `karem.dulook`. Replace it with `deyaxshami.dulook` and carry over its
settings:

```bash
old=$(jq -c '[.bar.layout[][]? | select(.id == "karem.dulook")][0] // {} | del(.id)' ~/.config/omarchy/shell.json)
omarchy plugin remove karem.dulook --yes
omarchy plugin add https://github.com/deyaxshami/Dulook-Rust --enable --yes
jq --argjson s "$old" '.bar.layout |= map_values(if type == "array" then map(if .id == "deyaxshami.dulook" then . + $s else . end) else . end)' \
  ~/.config/omarchy/shell.json > /tmp/shell.json && cat /tmp/shell.json > ~/.config/omarchy/shell.json && rm /tmp/shell.json
omarchy-restart-shell
```

## Prayer Calendar

Choose where prayer times come from in Settings → Prayer calendar → **Location**:

| Source | Uses |
| --- | --- |
| **Online / Address** | Finds a place by country, city, optional ZIP code, street and house number, and installs its calendar |
| **Online / Map pin** | Finds a place by a pin dropped on an OpenStreetMap map, and installs its calendar |
| **Online / Coordinates** | Finds a place by typed latitude and longitude, and installs its calendar |
| **Offline / Installed Calendars** | A bundled calendar or an installed download; works without internet |
| **Offline / Custom CSV…** | Any local calendar file |

New installations start on **Offline / Installed Calendars** with Nordhorn. Dulook
loads the calendar for the current year in its source time zone, together with the
neighbouring years so prayers around New Year are scheduled, and switches files
automatically when the year changes.

Dulook shows prayer times in the computer's current time zone and schedules
countdowns and alerts for those same instants. For example, when Cairo is
one hour ahead of Nordhorn, a Cairo Fajr at 6:00 AM shows as 5:00 AM in Nordhorn.
The difference follows daylight-saving rules for the date of each prayer.

**Offline / Installed Calendars** lists the bundled calendars, then every
calendar installed by an online source. A downloaded calendar is read as it is:
nothing is downloaded while it is selected, so it stops at the last downloaded
year.

A selected download shows its folder and three controls; bundled calendars are
read-only:

- **Calendar name**: an optional friendly name. The list then shows
  `<name> · <folder>`, for example `Home · DE_48527_Nordhorn`.
- **Edit location**: opens the online source that found it, with its address,
  pin or coordinates filled in. **Find & download** replaces the calendar,
  keeps its name, and removes the old folder if the folder name changed.
- **Delete**: removes the folder after a second click and selects another
  installed calendar.

Bundled calendars cover 2026–2037 for 3p Services, Alexandria, Cairo,
Nordhorn, Qalyub, Ricklingen, Sohag and Veldhausen. They live in
`data/<location>/prayer-times-<year>-<location>.csv`.

**Offline / Custom CSV…** reads the selected file directly: it is not uploaded,
copied, or cached on disk. If it is moved or deleted, prayer times and alerts remain
unavailable until a valid path is selected. Settings saved before locations
existed keep their custom CSV.

### Browse the prayer calendar

Open the Dulook panel and click **▦ Prayer calendar** beside Settings. On
Hyprland, the calendar opens as a centered floating window, sized to fit
the current display. It starts on the current Gregorian month. Switch to
**Hijri** and select **Ramadan**, the ninth month, to plan for any covered Ramadan.

**Year overview** shows all 12 months and each month's covered date range in
the other calendar. Select a covered month to see both dates and all eight
times, including Yellowing and Islamic Midnight, in 12-hour system-local time with AM/PM.
The browser reads every available year of the selected source without
downloading anything or changing the alert schedule. Missing dates and times
are not estimated. Hijri browsing is unavailable if the source has no valid
Hijri dates.

### Online location

The online sources only find a place and install its calendar: **Find &
download** resolves the place to latitude/longitude, downloads the current year
plus the next 10 years from [salatcalendar.com](https://www.salatcalendar.com),
and then switches to **Offline / Installed Calendars** with the new calendar
selected. An optional **Calendar name** is stored with it. Downloads use an
anonymous session whose cookie jar is a private temporary file, deleted
afterwards; no account or browser cookies are used.

**Online / Address**: pick a country and type a city, plus an optional ZIP code,
street and house number. Suggestions for the field you are typing in come from
[Photon](https://photon.komoot.io) (OpenStreetMap). Choosing a city also fixes
the coordinates. Choosing a street lists house numbers on it. House-number
suggestions only cover houses OpenStreetMap knows near the street, so any number
can still be typed.

**Online / Map pin** shows an [OpenStreetMap](https://www.openstreetmap.org/copyright)
map: drag to move, scroll or use +/− to zoom, and click to drop a pin. The pin's
exact coordinates are used for the calendar, and the country, city, ZIP code,
street and house number at that point are filled in from Photon. Map tiles are
downloaded with Dulook's User-Agent and cached for a week in
`$XDG_CACHE_HOME/dulook/tiles/` (default `~/.cache/dulook/tiles/`), as the
[OSM tile policy](https://operations.osmfoundation.org/policies/tiles/) asks.

**Online / Coordinates** takes only a latitude and a longitude in decimal
degrees, for example `52.43301` and `7.06913`. No place name is looked up.

Every download uses the same calculation settings:

- Fajr and Isha angles: 12°/12°.
- Dhuhr: +5 minutes.
- Altitude, Asr madhhab and daylight saving: automatic.
- Refraction: standard.

Times are calculated in the computer's time zone at download, then read in
the zone saved in `location.json` even if the computer's time zone changes.

Each location gets its own folder in `$XDG_DATA_HOME/dulook/calendars/`
(default `~/.local/share/dulook/calendars/`), named
`[Country]_[ZIP]_[City]_[Street]_[HouseNumber]`, with one
`<folder>_<year>.csv` per year. Optional parts that are empty are left out, for
example `DE_48527_Nordhorn/DE_48527_Nordhorn_2027.csv`. Folder names use ASCII
only: `ß` becomes `ss`, accents are dropped, and Arabic and other scripts are
spelled in Latin letters, so `EG` + `سوهاج` is `EG_swhaj`. A `location.json`
describes the place with its original spelling and the calendar's name.

A map pin is named `[Country]_[ZIP]_[City]_[Latitude]_[Longitude]` instead,
with 6 decimals and without street and house number, for example
`DE_48529_Nordhorn_52.433010_7.069130`. Typed coordinates are named by the
numbers alone, for example `52.433010_7.069130`.

The optional street and house number make the coordinates more precise. If
Photon cannot find that street, Dulook uses the ZIP code or the city instead. A
house number is only used together with a street.

**When Dulook downloads** (only on **Find & download**)
- A new country, city, ZIP code, street or house number gets a new folder and its own 11 years (this year and the next 10).
- A place that already has a complete folder is selected right away; changing only the ZIP code (or street/house number) counts as a new place.
- A folder missing some of those years gets only the missing ones, for example after the last downloaded year ends.
- When the chosen coordinates move more than 5 km from the stored ones, all 11 years are downloaded again.

**When something fails**
- If the place cannot be found or the download fails, the message stays under **Find & download** and the selected calendar does not change. Nothing is retried automatically.
- Settings saved while online sources were a calendar source of their own select their downloaded folder once.

### Adding a bundled location

To add a bundled location, create `data/<name>/` with one
`prayer-times-<year>-<name>.csv` per year, add `<name>` to the `location`
options in `manifest.json`, and register the CSV's source time zone in
`src/locations.rs`. The same scraper downloads the CSVs:

```bash
cargo run --release --bin dulook-data -- calendar --country DE --city Nordhorn --zip 48527 \
  [--lat 52.4722 --lon 7.0476] [--from 2026] [--years 11] [--out data/<name>]
```

Files are named `[Country]_[ZIP]_[City]_<year>.csv`. Rename them to
`prayer-times-<year>-<name>.csv` for a bundled location.

Custom CSVs and extra folders without a registered time zone are interpreted
in the computer's current time zone. Use a CSV prepared for that time zone.

Required columns can use English or Arabic headers:

| English | Arabic |
| --- | --- |
| `Date` | `التاريخ` |
| `Fajr` | `الفجر` |
| `Sunrise` | `الشروق` |
| `Dhuhr` | `الظهر` |
| `Asr` | `العصر` |
| `Maghrib` | `المغرب` |
| `Isha` | `العشاء` |

Gregorian dates may use `DD/MM/YYYY` or `YYYY-MM-DD`. An optional `Hijri`
(`التاريخ الهجري`) column supplies the paired Hijri date: `DD/MM/YYYY`, optionally
with a suffix such as `(Ram)`, or `YYYY-MM-DD`. Hijri dates also accept
Arabic-Indic digits. Other columns are ignored.

Times may use 24-hour `HH:mm` or 12-hour `hh:mm am/pm` notation. A time after
midnight that appears after a later evening prayer is treated as belonging to
the following civil day. The panel and calendar table display times using a
12-hour clock with `AM`/`PM`.

## Notifications And Sound

Notifications are enabled for all five prayers, Sunrise, Yellowing, and
Midnight by default and use Omarchy's native normal-duration popup. One logical
notification is sent and Omarchy mirrors it to every monitor. DND is respected
unless bypass is enabled. Sunrise, Yellowing, and Midnight never play the Adhan.

Adhan audio is enabled by default with `adans/adan_04.mp3`. Pick one of the
bundled recordings (Adhan 1–4, from `adans/`) in the Settings dropdown, or
choose "Custom file…" to select or type the path of your own local MP3. Dulook
uses a private `mpv` process, respects the system
sink and volume, and never stops unrelated media players. Left-click a
prayer-time notification to stop its playing Adhan. Right-click dismisses the
notification without stopping the audio.

The bar label blinks red when Yellowing or Midnight begins and 15 minutes
before each of the five prayers, regardless of the notification switches and
DND. It stops when the next time begins (Maghrib, Fajr, or the prayer itself)
or when you click the label; that click only stops the blinking.

## Hadith Data

Runtime Hadith requests use the pinned `fawazahmed0/hadith-api` revision listed
in `sources/hadith-api-selection.json`. When requests fail, Dulook rotates
through `data/hadith-fallback.json`, containing 50 Bukhari and 50 Muslim
records with paired Arabic and English text.

Regenerate and verify the reviewed fallback data with:

```bash
cargo run --release --bin dulook-data -- build
cargo run --release --bin dulook-data -- build --check
cargo run --release --bin dulook-data -- validate
cargo test
```

## Development

The QML files are a thin view. All logic (calendar parsing, scheduling,
notifications, Adhan playback, Hadith requests, translations) lives in the
Rust `dulook` backend under `src/`, which the shell starts as a child process
and talks to over JSON lines on stdin/stdout. `omarchy plugin add` only clones
the repository, so stripped release binaries are committed as
`bin/dulook-x86_64` and `bin/dulook-aarch64`; `bin/dulook` is a launcher that
runs the one matching `uname -m`. After changing anything under `src/`,
rebuild both binaries, run the tests, and commit the updated binaries. `make`
cross-builds with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild),
so it needs `zig` and a one-time setup:

```bash
cargo install --locked cargo-zigbuild
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
make
make test
```

See `THIRD_PARTY_NOTICES.md` for provenance and licensing.
