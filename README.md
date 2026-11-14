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

Updating from a build that bundled the Nordhorn calendar keeps your settings,
but that bundled calendar is gone. Open Dulook's settings, set **Location** to
**Online (country / city)**, pick a country, type a city or use **Pick on map**,
and press **Find & download**. Downloaded calendars live in
`~/.local/share/dulook/calendars/` on each computer and are not synced.

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

Choose a location in Settings → Prayer calendar. **Online (country / city)** is
the default. Dulook loads the location's calendar for the current year,
together with the neighbouring years so prayers around New Year are scheduled,
and switches files automatically when the year changes. Folders under `data/`
(`data/<location>/prayer-times-<year>-<location>.csv`) also appear as
locations; none are bundled at the moment.

Choose **Custom CSV…** to use any local calendar instead. The selected file is
read directly: it is not uploaded, copied, or cached. If it is moved or
deleted, prayer times and alerts remain unavailable until a valid path is
selected. Settings saved before locations existed keep their custom CSV.

### Online location

Choose **Online (country / city)**, pick a country, and type a city, plus an
optional ZIP code, street and house number. Suggestions for the field you are
typing in come from [Photon](https://photon.komoot.io) (OpenStreetMap). Choosing
a city also fixes the coordinates. Choosing a street lists house numbers on it.
House-number suggestions only cover houses OpenStreetMap knows near the street,
so any number can still be typed. **Find & download**
resolves the place to latitude/longitude and downloads the current year plus the
next 10 years from [salatcalendar.com](https://www.salatcalendar.com). It uses an
anonymous session whose cookie jar is a private temporary file, deleted
afterwards; no account or browser cookies are used.

**Pick on map** opens an [OpenStreetMap](https://www.openstreetmap.org/copyright)
map in the panel: drag to move, scroll or use +/− to zoom, and click to drop a
pin. The pin's exact coordinates are used for the calendar, and the country,
city, ZIP code, street and house number at that point are filled in from
Photon. **Remove pin** goes back to the typed address. Map tiles are downloaded
with Dulook's User-Agent and cached for a week in `$XDG_CACHE_HOME/dulook/tiles/`
(default `~/.cache/dulook/tiles/`), as the
[OSM tile policy](https://operations.osmfoundation.org/policies/tiles/) asks.

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
example `DE_48527_Nordhorn/DE_48527_Nordhorn_2027.csv` or
`EG_سوهاج/EG_سوهاج_2027.csv`. A `location.json` describes the place.

A pinned location is named `[Country]_[ZIP]_[City]_[Latitude]_[Longitude]`
instead, with 6 decimals and without street and house number, for example
`DE_48529_Nordhorn_52.433010_7.069130`.

The optional street and house number make the coordinates more precise. If
Photon cannot find that street, Dulook uses the ZIP code or the city instead. A
house number is only used together with a street. A pin wins over both.

**When Dulook downloads**
- A new country, city, ZIP code, street or house number gets a new folder and its own 11 years (this year and the next 10).
- Switching back to a location that already has a folder reuses its files; changing only the ZIP code (or street/house number) counts as a new location.
- A folder whose current year is missing gets only its missing years, for example after the last downloaded year ends.
- When the chosen coordinates move more than 5 km from the stored ones.
- On **Download again**.

**When something fails**
- If the place cannot be found or the download fails, Dulook keeps showing the last location that worked and retries after 30 minutes.
- Nothing is detected automatically; the location only changes when you change it.

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
