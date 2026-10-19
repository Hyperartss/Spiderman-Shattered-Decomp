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
# Build parallelism is deliberately capped. Generated translation units reach
# 1.2 MB and clang++ takes 1-3 GB on each; ninja would otherwise default to all
# 24 cores and demand far more RAM than the machine has, which gets the OOM
# killer started. Override with BUILD_JOBS if you know the machine is idle.
BUILD_JOBS="${BUILD_JOBS:-4}"

GAME_ROOT="$PWD/run"
RUNTIME_LOG_DIR="$BUILD_DIR/logs"

log() { printf '\n=== %s ===\n' "$1"; }

warn_on_pressure() {
  local avail
  avail="$(free -m | awk '/^Mem:/{print $7}')"
  if [ "${avail:-0}" -lt 1500 ]; then
    echo "  WARNING: only ${avail} MB available RAM."
    echo "  Free /dev/shm (orphan emulator segments) before building:"
    echo "      rm -f /dev/shm/xenia_memory_*"
  fi
}

# Print the guest address from the last SDK log's unregistered-function error.
last_bad_address() {
  local latest
  latest="$(ls -t "$RUNTIME_LOG_DIR"/sd360_*.log 2>/dev/null | head -1)"
  [ -n "$latest" ] || return 1
  grep -oE 'unregistered function at guest address 0x[0-9A-Fa-f]+' "$latest" \
    | tail -1 | grep -oE '0x[0-9A-Fa-f]+' | sed 's/^0x//'
}

# Classify how the last run ended. Absence of the unregistered-function error is
# NOT convergence: a different crash (bus error, segfault, abort) also removes
# it, and reporting "converged" there hides real progress behind a false win.
last_run_outcome() {
  local latest
  latest="$(ls -t "$RUNTIME_LOG_DIR"/sd360_*.log 2>/dev/null | head -1)"
  if [ -z "$latest" ]; then echo "no-log"; return; fi
  if grep -qE 'unregistered function at guest address' "$latest"; then
    echo "unregistered-function"
  elif grep -qiE 'FATAL|fatal error' "$latest"; then
    echo "other-fatal"
  else
    echo "clean"
  fi
}

is_declared() {
  grep -qiE "^[[:space:]]*0x?$1[[:space:]]*=" "$MANIFEST"
}

add_entry() {
  local addr="$1"   # bare hex, no 0x prefix
  if is_declared "$addr"; then
    echo "  0x$addr already declared; nothing to add"
    return 1
  fi
  printf '0x%s = { size = 0x8 }\n' "$addr" >> "$MANIFEST"
  echo "  added 0x$addr"
  return 0
}

run_once() {
  LD_LIBRARY_PATH="$BUILD_DIR" timeout -s KILL 90 "$BUILD_DIR/sd360" \
    --game_data_root "$GAME_ROOT" > /dev/null 2>&1
}

for i in $(seq 1 "$MAX_ITER"); do
  log "iteration $i/$MAX_ITER"
  run_once
  outcome="$(last_run_outcome)"

  case "$outcome" in
    unregistered-function)
      addr="$(last_bad_address || true)"
      echo "  runtime reports unregistered call at 0x$addr"
      if ! add_entry "$addr"; then
        echo "  0x$addr is already declared and still traps."
        echo "  Stopping: the entry needs a real size or a parent, not another pass."
        exit 2
      fi
      ;;
    other-fatal)
      echo "  no unregistered-function error, but the run died another way."
      echo "  This script cannot fix that class of failure. Latest log:"
      latest="$(ls -t "$RUNTIME_LOG_DIR"/sd360_*.log | head -1)"
      grep -iE 'FATAL|critical' "$latest" | tail -5 | sed 's/^/    /'
      echo "  NOT converged -- a different crash is now the blocker."
      exit 4
      ;;
    clean)
      echo "  run completed with no fatal error."
      echo "  Converged."
      exit 0
      ;;
    *)
      echo "  could not classify the run (no SDK log produced). Stopping."
      exit 5
      ;;
  esac

  echo "  re-running codegen"
  warn_on_pressure
  ./tools/linux-amd64/bin/rexglue codegen "$MANIFEST" > /dev/null 2>&1
  echo "  rebuilding (-j$BUILD_JOBS)"
  if ! (cd sd360 && cmake --build --preset linux-amd64-debug -j "$BUILD_JOBS") > /dev/null 2>&1; then
    echo "  build failed; stopping"
    exit 3
  fi
done

log "hit iteration limit ($MAX_ITER)"
echo "Re-run to continue."
exit 1