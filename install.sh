#!/bin/bash

set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
app_dir="$HOME/.local/share/dulook"
bin_dir="$HOME/.local/bin"
desktop_dir="$HOME/.local/share/applications"
systemd_dir="$HOME/.config/systemd/user"

install -d "$app_dir" "$bin_dir" "$desktop_dir" "$systemd_dir"
if [[ "$repo_dir" != "$app_dir" ]]; then
  install -m 644 "$repo_dir/index.html" "$app_dir/index.html"
fi
install -m 755 "$repo_dir/scripts/dulook-update" "$bin_dir/dulook-update"
install -m 644 "$repo_dir/systemd/dulook-update.service" "$systemd_dir/dulook-update.service"
install -m 644 "$repo_dir/systemd/dulook-update.timer" "$systemd_dir/dulook-update.timer"
sed "s|@APP_DIR@|$app_dir|g" "$repo_dir/dulook.desktop.in" > "$desktop_dir/dulook.desktop"

systemctl --user daemon-reload
systemctl --user enable --now dulook-update.timer
systemctl --user start dulook-update.service

echo "Dulook installed. Open it from the application launcher."
