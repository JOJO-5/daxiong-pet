#!/usr/bin/env bash
set -euo pipefail
binary="$PWD/src-tauri/target/x86_64-unknown-linux-gnu/release/daxiong-pet"
smoke_test_dir="$(mktemp -d)"
export XDG_CONFIG_HOME="$smoke_test_dir/config"
export CODEX_HOME="$smoke_test_dir/codex"
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
first_pid=""
second_pid=""
cleanup() {
  result=$?
  if [[ "$result" -ne 0 ]]; then
    echo 'First launch log:' >&2
    cat "$smoke_test_dir/first.log" >&2 || true
    echo 'Second launch log:' >&2
    cat "$smoke_test_dir/second.log" >&2 || true
  fi
  if [[ -n "$second_pid" ]]; then kill "$second_pid" 2>/dev/null || true; fi
  if [[ -n "$first_pid" ]]; then kill "$first_pid" 2>/dev/null || true; fi
}
trap cleanup EXIT
"$binary" >"$smoke_test_dir/first.log" 2>&1 &
first_pid=$!
sleep 5
kill -0 "$first_pid"
"$binary" >"$smoke_test_dir/second.log" 2>&1 &
second_pid=$!
for _ in {1..20}; do
  if ! kill -0 "$second_pid" 2>/dev/null; then break; fi
  sleep 0.5
done
if kill -0 "$second_pid" 2>/dev/null; then
  cat "$smoke_test_dir/second.log"
  echo 'Duplicate process did not exit' >&2
  exit 1
fi
wait "$second_pid"
second_pid=""
kill -0 "$first_pid"
echo 'App stays running; duplicate launch exits successfully.'
