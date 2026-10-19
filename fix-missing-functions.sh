#!/usr/bin/env bash
# Converge on the missing XEX hotpatch function entries.
#
# Symptom this fixes:
#   [FATAL] Call to invalid or unregistered function at guest address 0x88xxxxxx
#
# The XEX patch scheme rewrites each function's first instruction into an
# unconditional branch to a per-region hotpatch stub. Discovery does not treat
# those as function entries, so calling one traps. Every such address the
# runtime reports is added to [modules.functions], then codegen and the build
# are re-run. The runtime is an exact oracle, so this converges even though
# static enumeration cannot (code that was never decoded is invisible to a
# listing scan, and every branch-target heuristic also matches ordinary hot
# branches to real functions).
#
# Usage:
#   ./fix-missing-functions.sh [max-iterations]
#
# Stops when a run reports no unregistered-function error, or when the limit is
# hit. Re-running continues from whatever is already in the manifest.

set -uo pipefail

cd "$(dirname "$0")"

MAX_ITER="${1:-10}"
BUILD_DIR="$PWD/sd360/out/build/linux-amd64-debug"
MANIFEST="$PWD/sd360/sd360_manifest.toml"
GAME_ROOT="$PWD/run"
RUNTIME_LOG_DIR="$BUILD_DIR/logs"

log() { printf '\n=== %s ===\n' "$1"; }

# Print the guest address from the last SDK log's unregistered-function error.
last_bad_address() {
  local latest
  latest="$(ls -t "$RUNTIME_LOG_DIR"/sd360_*.log 2>/dev/null | head -1)"
  [ -n "$latest" ] || return 1
  grep -oE 'unregistered function at guest address 0x[0-9A-Fa-f]+' "$latest" \
    | tail -1 | grep -oE '0x[0-9A-Fa-f]+'
}

is_declared() {
  grep -qiE "^[[:space:]]*$1[[:space:]]*=" "$MANIFEST"
}

add_entry() {
  local addr="$1"
  # [modules.functions] entries must be lowercase hex keys to match the table.
  local key
  key="$(printf '%s' "$addr" | tr 'A-F' 'a-f')"
  if is_declared "$key"; then
    echo "  $key already declared; nothing to add"
    return 1
  fi
  printf '0x%s = { size = 0x8 }\n' "$key" >> "$MANIFEST"
  echo "  added 0x$key"
  return 0
}

run_once() {
  LD_LIBRARY_PATH="$BUILD_DIR" timeout -s KILL 90 "$BUILD_DIR/sd360" \
    --game_data_root "$GAME_ROOT" > /dev/null 2>&1
}

for i in $(seq 1 "$MAX_ITER"); do
  log "iteration $i/$MAX_ITER"
  run_once
  addr="$(last_bad_address || true)"

  if [ -z "$addr" ]; then
    echo "No unregistered-function error. Converged."
    exit 0
  fi

  echo "  runtime reports $addr"
  if ! add_entry "$addr"; then
    echo "  $addr was already declared and still traps."
    echo "  Stopping: further iterations would not change anything."
    exit 2
  fi

  echo "  re-running codegen"
  ./tools/linux-amd64/bin/rexglue codegen "$MANIFEST" > /dev/null 2>&1
  echo "  rebuilding"
  (cd sd360 && cmake --build --preset linux-amd64-debug) > /dev/null 2>&1
  if [ $? -ne 0 ]; then
    echo "  build failed; stopping"
    exit 3
  fi
done

log "hit iteration limit ($MAX_ITER)"
echo "Re-run to continue."
exit 1