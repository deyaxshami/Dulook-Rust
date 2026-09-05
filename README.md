# Dulook

Dulook is a bilingual prayer-time and Hadith widget for the Omarchy Shell bar.
Prayer times come only from a CSV file selected by the user and never require
network access. Internet access is used only to request Arabic and English
Hadith; a reviewed set of 100 bilingual Hadith is bundled for offline use.

## Features

- Native Quickshell and Omarchy bar UI
- Live `HH:MM:SS` countdown to the next prayer in the bar
- English interface by default with a complete Arabic mode
- Independent English or Arabic language selection for Hadith content
- System-local timezone and daylight-saving behavior
- User-selected CSV with automatic reload on file changes
- Native prayer notifications on every monitor
- Per-prayer notification controls and optional DND bypass
- Optional local MP3 playback through `mpv`
- Separate Arabic UI and Hadith font settings
- Exactly 100 paired Arabic and English offline Hadith

## Install

The repository is private. Authenticate Git once through GitHub CLI before
using Omarchy's Git-based plugin commands:

```bash
gh auth setup-git
```

Then install the plugin:

```bash
omarchy plugin add https://github.com/Karem-Shami/Dulook --enable
```

The repository root is the plugin root, so Git-managed installations can be
updated with:

```bash
omarchy plugin update karem.dulook
```

## Prayer Calendar

Dulook starts without prayer data. Open its settings and choose a local CSV.
The file is read directly: it is not uploaded, copied, or cached. If it is
moved or deleted, prayer times and alerts remain unavailable until a valid
path is selected.

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

Adhan audio is disabled by default. Select a local MP3 and enable sound in the
settings panel. Dulook uses a private `mpv` process, respects the system sink
and volume, and never stops unrelated media players.

## Hadith Data

Runtime Hadith requests use the pinned `fawazahmed0/hadith-api` revision listed
in `sources/hadith-api-selection.json`. When requests fail, Dulook rotates
through `data/hadith-fallback.json`, containing 50 Bukhari and 50 Muslim
records with paired Arabic and English text.

Regenerate and verify the reviewed fallback data with:

```bash
npm run build:hadith
npm run check:hadith
npm test
npm run validate
```

See `THIRD_PARTY_NOTICES.md` for provenance and licensing.
