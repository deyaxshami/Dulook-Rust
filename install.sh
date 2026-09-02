#!/bin/bash

set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
app_dir="$HOME/.local/share/dulook"
desktop_dir="$HOME/.local/share/applications"

install -d "$app_dir" "$desktop_dir"
if [[ "$repo_dir" != "$app_dir" ]]; then
  install -m 644 "$repo_dir/index.html" "$app_dir/index.html"
  install -m 644 "$repo_dir/SalatCalender2026_Nordhorn.csv" "$app_dir/SalatCalender2026_Nordhorn.csv"
fi
sed "s|@APP_DIR@|$app_dir|g" "$repo_dir/dulook.desktop.in" > "$desktop_dir/dulook.desktop"

systemctl --user disable --now dulook-update.timer 2>/dev/null || true
rm -f "$HOME/.local/bin/dulook-update" \
  "$HOME/.config/systemd/user/dulook-update.service" \
  "$HOME/.config/systemd/user/dulook-update.timer" \
  "$app_dir/online_prayer_times.json"
systemctl --user daemon-reload

echo "Dulook installed. Open it from the application launcher."
