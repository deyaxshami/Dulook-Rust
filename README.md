# Dulook

Dulook is a local Arabic prayer-time app for Nordhorn, Germany. It calculates
prayer times offline, checks the AlAdhan API twice daily, and displays the last
successful online update.

## Install

Requirements: Chromium, curl, jq, and systemd.

```bash
./install.sh
```

The installer adds the desktop launcher and enables online checks at 06:00 and
18:00. Prayer times continue to work when the API or internet is unavailable.
