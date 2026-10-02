#!/usr/bin/env bash
set -euo pipefail
test_dir="$(mktemp -d)"
export XDG_CONFIG_HOME="$test_dir/config"
export XDG_DATA_HOME="$test_dir/data"
export XDG_CACHE_HOME="$test_dir/cache"
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
# Containers may not allow nested namespaces. This affects the isolated test process only.
if [[ "$(id -u)" == 0 ]]; then export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1; fi
openbox >"$test_dir/wm.log" 2>&1 &
wm_pid=$!
tauri-driver >"$test_dir/driver.log" 2>&1 &
driver_pid=$!
cleanup() {
  result=$?
  if [[ "$result" != 0 ]]; then cat "$test_dir/driver.log" >&2; fi
  # Both the native WebKit driver and the application are children of tauri-driver.
  # Terminate them as well, so the next run cannot connect to a previous X server.
  pkill -TERM -P "$driver_pid" 2>/dev/null || true
  kill "$driver_pid" "$wm_pid" 2>/dev/null || true
  wait "$driver_pid" "$wm_pid" 2>/dev/null || true
}
trap cleanup EXIT
for _ in {1..50}; do
  if curl --silent http://127.0.0.1:4444/status >/dev/null; then break; fi
  sleep 0.1
done
python3 "${E2E_SCRIPT:-scripts/e2e-desktop.py}"
