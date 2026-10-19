#!/usr/bin/env bash
# Launch the recompilation.
#
# Two things this handles that are easy to get wrong by hand:
#
#   1. LD_LIBRARY_PATH. The guest module is dlopen()'d from librexruntimed.so,
#      and DT_RUNPATH is not inherited for dlopen, so the build directory has to
#      be on the loader path or the module silently fails to load
#      ("Failed to load shared library for module 'gamelogic.dll'").
#
#   2. --game_data_root. The game data root is a flag, not a positional
#      argument, and it must point at ./run rather than ./assets -- the runtime
#      opens "default.xex" in lowercase while the extracted file is
#      "Default.xex". run/ is a hardlink mirror, so it costs no extra disk and
#      leaves the game files untouched.
#
#   3. REX_VULKAN_ASYNC_SKIP_INCOMPLETE_FRAMES=false. Both
#      async_shader_compilation and vulkan_async_skip_incomplete_frames default
#      to true, so while shaders compile asynchronously every frame that drew
#      with a placeholder pipeline is dropped instead of presented. This game's
#      shaders take a while to compile, so the result is a black screen while
#      audio plays normally. Setting the flag to false presents those frames.
#
#      This must be an environment variable, not rex::cvar::SetFlagByName: the
#      in-code set runs too early to affect the present-time read.
#
# Usage:
#   ./run.sh                 run with defaults
#   ./run.sh --window_width 1280 --window_height 720
#   ./run.sh <any sd360 flag...>

set -uo pipefail

cd "$(dirname "$0")"

BUILD_DIR="$PWD/sd360/out/build/linux-amd64-debug"

if [ ! -x "$BUILD_DIR/sd360" ]; then
  echo "No binary at $BUILD_DIR/sd360"
  echo "Build it first:"
  echo "  (cd sd360 && cmake --preset linux-amd64-debug \\"
  echo "     -Drexglue_DIR=\"$PWD/tools/linux-amd64/lib/cmake/rexglue\" \\"
  echo "   && cmake --build --preset linux-amd64-debug -j 4)"
  exit 1
fi

if [ ! -e run/default.xex ]; then
  echo "run/ is missing. It is a hardlink mirror of assets/ built with:"
  echo "  mkdir -p run && cp -al assets run && ln run/Default.xex run/default.xex"
  exit 1
fi

exec env \
  LD_LIBRARY_PATH="$BUILD_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  REX_VULKAN_ASYNC_SKIP_INCOMPLETE_FRAMES=false \
  "$BUILD_DIR/sd360" --game_data_root "$PWD/run" "$@"