#!/usr/bin/env bash
# Compile once, then retry only DMG packaging. Keep diagnostics for every attempt.
set -euo pipefail
target=${1:?target triple required}
config=${2:?Tauri config required}
variant=${3:?full or light required}
logs="test-results/macos-packaging/$variant"
mkdir -p "$logs"
npm run tauri build -- --target "$target" --bundles app --config "$config" --ci --verbose 2>&1 | tee "$logs/app.log"
for attempt in 1 2 3; do
  if npm run tauri bundle -- --target "$target" --bundles dmg --config "$config" --ci --verbose 2>&1 | tee "$logs/dmg-$attempt.log"; then
    exit 0
  fi
  if [[ "$attempt" -lt 3 ]]; then
    echo "DMG packaging attempt $attempt failed; retrying in 10 seconds."
    sleep 10
  fi
done
exit 1
