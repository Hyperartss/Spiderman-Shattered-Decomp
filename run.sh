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
#   4. REX_RENDER_TARGET_PATH_VULKAN=fsi. The default host-render-target path
#      mishandles this game's EDRAM aliasing: every movie (Marvel/Activision/
#      Beenox logos, intro) renders black. The fragment-shader-interlock path
#      emulates EDRAM directly and plays them. Override with
#      REX_RENDER_TARGET_PATH_VULKAN= to get the host path back.
#
#   5. PC controls are built into the binary (src/pc_controls.h), so no extra
#      config file is needed. Any --keybind_* / --mnk_* flag given on the
#      command line still overrides the built-in layout.
#
#   7. --gpu_allow_invalid_fetch_constants=true. This title uploads texture
#      fetch constants whose type field (dword 0 bits 0:1) is 0 =
#      kInvalidTexture, while everything else in the constant is a well-formed
#      texture (format, base pages, pitch). The emulator's strict check skips
#      every draw using them, which is why the main-menu background renders
#      black and the dimension switch draws garbage. Measured 2026-11-02:
#      748 "has \"invalid\" type" warnings in a menu run, 0 with this flag, and
#      the title background + museum intro then render correctly. Truly empty
#      constants (base page 0) are still ignored by the texture cache.
#
#   6. The GPU plugin in the RelWithDebInfo build dir is built from
#      tools/rexglue-sdk with patches/0001 and 0002 applied; without them the
#      title-screen 3D is white (0001 missing) or black (0002 missing).
#
# Usage:
#   ./run.sh                 run with defaults
#   ./run.sh --window_width 1280 --window_height 720
#   ./run.sh <any sd360 flag...>

set -uo pipefail

cd "$(dirname "$0")"

# The optimised build is the default (PRESET=linux-amd64-debug ./run.sh for the
# debug one). The debug SDK runtime aborts on harmless asserts
# (texture_cache.cpp:876 at the title screen, and the fsi path asserts too);
# RelWithDebInfo has them compiled out and reaches the menu.
BUILD_DIR="$PWD/sd360/out/build/${PRESET:-linux-amd64-relwithdebinfo}"

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
  REX_RENDER_TARGET_PATH_VULKAN="${REX_RENDER_TARGET_PATH_VULKAN-fsi}" \
  "$BUILD_DIR/sd360" --game_data_root "$PWD/run" \
  --gpu_allow_invalid_fetch_constants=true "$@"