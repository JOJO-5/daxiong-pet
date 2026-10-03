#!/usr/bin/env bash
set -euo pipefail
test_dir="$(mktemp -d)"
export XDG_CONFIG_HOME="$test_dir/config"
export XDG_DATA_HOME="$test_dir/data"
export XDG_CACHE_HOME="$test_dir/cache"
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1
# WebKitWebDriver's legacy-window/restart scenarios need the compatibility path.
# It can leave stale transparent pixels: use the default renderer for visual proof.
compositor_pid=""
if [[ "${E2E_RECORD:-0}" == 1 || "${E2E_VISUAL:-0}" == 1 ]]; then
  unset WEBKIT_DISABLE_DMABUF_RENDERER
else
  export WEBKIT_DISABLE_DMABUF_RENDERER=1
fi
# Containers may not allow nested namespaces. This affects the isolated test process only.
if [[ "$(id -u)" == 0 ]]; then export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1; fi
openbox >"$test_dir/wm.log" 2>&1 &
wm_pid=$!
# Transparent windows need a compositor; otherwise Xvfb leaves stale pixels.
if [[ "${E2E_RECORD:-0}" == 1 || "${E2E_VISUAL:-0}" == 1 ]]; then
  picom --backend xrender --no-use-damage --config /dev/null >"$test_dir/compositor.log" 2>&1 &
  compositor_pid=$!
fi
recorder_pid=""
if [[ "${E2E_RECORD:-0}" == 1 ]]; then
  mkdir -p "${E2E_OUT:-test-results}"
  ffmpeg -y -loglevel error -f x11grab -framerate 25 -video_size 1280x800 -i "$DISPLAY" -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p "${E2E_OUT:-test-results}/desktop-validation.mp4" >"$test_dir/record.log" 2>&1 &
  recorder_pid=$!
fi
tauri-driver --port "${E2E_PORT:-4444}" --native-port "${E2E_NATIVE_PORT:-4445}" >"$test_dir/driver.log" 2>&1 &
driver_pid=$!
cleanup() {
  result=$?
  if [[ -n "$recorder_pid" ]]; then
    kill -INT "$recorder_pid" 2>/dev/null || true
    wait "$recorder_pid" 2>/dev/null || true
  fi
  if [[ "$result" != 0 ]]; then cat "$test_dir/driver.log" >&2; fi
  # Both the native WebKit driver and the application are children of tauri-driver.
  # Terminate them as well, so the next run cannot connect to a previous X server.
  pkill -TERM -P "$driver_pid" 2>/dev/null || true
  if [[ -n "$compositor_pid" ]]; then
    kill "$compositor_pid" 2>/dev/null || true
    wait "$compositor_pid" 2>/dev/null || true
  fi
  kill "$driver_pid" "$wm_pid" 2>/dev/null || true
  wait "$driver_pid" "$wm_pid" 2>/dev/null || true
  # Keep the actual driver/window-manager logs with both passing and failing runs.
  mkdir -p "${E2E_OUT:-test-results}"
  cp "$test_dir"/*.log "${E2E_OUT:-test-results}/" 2>/dev/null || true
}
trap cleanup EXIT
for _ in {1..50}; do
  if curl --silent "http://127.0.0.1:${E2E_PORT:-4444}/status" >/dev/null; then break; fi
  sleep 0.1
done
python3 "${E2E_SCRIPT:-scripts/e2e-desktop.py}"
