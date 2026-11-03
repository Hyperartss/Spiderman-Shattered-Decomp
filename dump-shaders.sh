#!/usr/bin/env bash
# Dump and decompile the game's Xbox 360 shaders.
#
# ReXGlue's Shader::AnalyzeUcode writes two files per unique shader into the
# --dump_shaders directory when the shader is first analysed:
#
#   shader_<HASH>.ucode.bin.vert / .frag   raw Xenos ucode, 3 dwords/instruction
#   shader_<HASH>.ucode.vert      / .frag   readable disassembly
#
# The disassembly is the "decompile" step: opcodes, source/dest swizzles,
# tfetch slots, filters, interpolators, alloc/exec structure.
#
# IMPORTANT: a shader is only dumped when it is analysed. With a warm pipeline
# cache the game reuses stored shaders and dumps nothing, so this script uses a
# private cache root and disables store_shaders to force fresh translation.
#
# IMPORTANT: the ucode disassembly does NOT contain num_format / exp_adjust.
# Those live in the fetch constant bound to a tf slot at draw time, not in the
# shader instruction, so decompiling alone cannot explain a wrong texture read.
# Pair a dump with the fetch constants for the same draw (tfN base/format/
# exp_adjust) to reason about format handling.
#
# Usage:
#   ./dump-shaders.sh                 dump to logs/shaders-dump, then play
#   ./dump-shaders.sh /tmp/sh        dump to /tmp/sh
#   ./dump-shaders.sh /tmp/sh --window_width 1280 --window_height 720
#
# Then search the disassembly, e.g.:
#   grep -l tfetch logs/shaders-dump/*.ucode.frag
#   grep -n "log\|exp" logs/shaders-dump/shader_XXXX.ucode.frag
#   grep -h "tfetch" logs/shaders-dump/*.ucode.frag | sort | uniq -c | sort -rn

set -uo pipefail
cd "$(dirname "$0")"

OUT="${1:-$PWD/logs/shaders-dump}"
if [ $# -gt 0 ]; then shift; fi

rm -rf "$OUT"
mkdir -p "$OUT"

pgrep -x sd360 >/dev/null && { echo "sd360 is already running - close it first"; exit 1; }
rm -f /dev/shm/xenia_memory_*

echo "dumping shaders to $OUT"
echo "(run, reach the scene you care about, then quit; a summary prints on exit)"

./run.sh \
  --dump_shaders="$OUT" \
  --store_shaders=false \
  --cache_root="$OUT/cache" \
  "$@"

rc=$?
echo
echo "exit=$rc"
printf 'dumped: %s binaries, %s disassemblies\n' \
  "$(find "$OUT" -maxdepth 1 -name '*.ucode.bin.*' | wc -l)" \
  "$(find "$OUT" -maxdepth 1 -name '*.ucode.vert' -o -maxdepth 1 -name '*.ucode.frag' | wc -l)"
echo
echo "largest fragment shaders (most work per pixel):"
find "$OUT" -maxdepth 1 -name '*.ucode.frag' -printf '%s %f\n' 2>/dev/null |
  sort -rn | head -10
