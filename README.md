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
- User-selected CSV with automatic reload on file changes
- Bundled 2026 Nordhorn prayer calendar and three Adhan recordings
- Native prayer notifications on every monitor, including a reminder 15 minutes before each prayer
- A notification when the last third of the night begins
- Independent prayer-time, 15-minute, last-third, per-prayer, and DND controls
- Adhan dropdown with three bundled recordings, or your own local MP3, played through `mpv`
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
- `mpv` for Adhan playback and `curl` for online Hadith, at `/usr/bin`

## Install

The repository is private. Authenticate Git once through GitHub CLI before
using Omarchy's Git-based plugin commands:

```bash
gh auth setup-git
```

Then install the plugin:

```bash
omarchy plugin add https://github.com/deyaxshami/Dulook-Rust --enable
```

No build step is needed; the prebuilt backend ships in `bin/dulook`.

The repository root is the plugin root, so Git-managed installations can be
updated with:

```bash
omarchy plugin update karem.dulook
```

### Upgrading from the old Dulook

The old `Karem-Shami/Dulook` plugin uses the same id, `karem.dulook`, so
`omarchy plugin add` refuses to install over it and `omarchy plugin update`
keeps pulling the old repository. Point the existing install at this
repository instead; this keeps your Dulook settings:

```bash
git -C ~/.config/omarchy/plugins/karem.dulook remote set-url origin https://github.com/deyaxshami/Dulook-Rust
omarchy plugin update karem.dulook --yes
omarchy-restart-shell
```

## Prayer Calendar

Dulook starts with the bundled `data/prayer-times-2026-nordhorn.csv` calendar.
It covers Nordhorn for 2026 only. Choose a local CSV in Settings when using
Dulook in another location or year. The selected file is read directly: it is
not uploaded, copied, or cached. If it is moved or deleted, prayer times and
alerts remain unavailable until a valid path is selected.

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

Adhan audio is enabled by default with `adans/adan_01.mp3`. Pick one of the
bundled recordings (Adhan 1–3, from `adans/`) in the Settings dropdown, or
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
