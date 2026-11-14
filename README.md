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
- Four bundled Adhan recordings
- Native prayer notifications on every monitor, including a reminder 15 minutes before each prayer
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

## Requirements

- Omarchy Shell on x86_64 Linux (the committed `bin/dulook` is an x86_64 build)
- `mpv` for Adhan playback and `curl` for online Hadith, calendars and map tiles, at `/usr/bin`

## Install

```bash
omarchy plugin add https://github.com/deyaxshami/Dulook-Rust --enable
```

No build step is needed; the prebuilt backend ships in `bin/dulook`.

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
loads the location's calendar for the current year, together with the
neighbouring years so prayers around New Year are scheduled, and switches files
automatically when the year changes.

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
copied, or cached. If it is moved or deleted, prayer times and alerts remain
unavailable until a valid path is selected. Settings saved before locations
existed keep their custom CSV.

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

Times are calculated for the computer's system time zone.

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
`prayer-times-<year>-<name>.csv` per year and add `<name>` to the `location`
options in `manifest.json`. The same scraper downloads them:

```bash
cargo run --release --bin dulook-data -- calendar --country DE --city Nordhorn --zip 48527 \
  [--lat 52.4722 --lon 7.0476] [--from 2026] [--years 11] [--out data/<name>]
```

Files are named `[Country]_[ZIP]_[City]_<year>.csv`. Rename them to
`prayer-times-<year>-<name>.csv` for a bundled location.

CSV times are interpreted in the computer's current system timezone. Use a
calendar prepared for the same location and timezone as the computer.

Required columns can use English or Arabic headers:

| English | Arabic |
| --- | --- |
| `Date` | `التاريخ` |
| `Fajr` | `الفجر` |
| `Dhuhr` | `الظهر` |
| `Asr` | `العصر` |
| `Maghrib` | `المغرب` |
| `Isha` | `العشاء` |

Additional columns are ignored. Dates may use `DD/MM/YYYY` or `YYYY-MM-DD`.
Times may use 24-hour `HH:mm` or 12-hour `hh:mm am/pm` notation. A time after
midnight that appears after a later evening prayer is treated as belonging to
the following civil day. Prayer times are displayed using a 12-hour clock with
`AM`/`PM`.

## Notifications And Sound

Notifications are enabled for all five prayers by default and use Omarchy's
native normal-duration popup. One logical notification is sent and Omarchy
mirrors it to every monitor. DND is respected unless bypass is enabled.

Adhan audio is enabled by default with `adans/adan_04.mp3`. Pick one of the
bundled recordings (Adhan 1–4, from `adans/`) in the Settings dropdown, or
choose "Custom file…" to select or type the path of your own local MP3. Dulook
uses a private `mpv` process, respects the system
sink and volume, and never stops unrelated media players. Left-click a
prayer-time notification to stop its playing Adhan. Right-click dismisses the
notification without stopping the audio.

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
the repository, so the stripped release binary is committed as `bin/dulook`.
After changing anything under `src/`, rebuild the binary (needs a Rust
toolchain), run the tests, and commit the updated `bin/dulook`:

```bash
make
make test
```

See `THIRD_PARTY_NOTICES.md` for provenance and licensing.
