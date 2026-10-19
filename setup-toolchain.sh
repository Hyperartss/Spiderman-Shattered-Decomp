#!/usr/bin/env bash
# Installs the toolchain a ReXGlue recompilation needs: ninja + clang 20.
#
# This edits system packages, so it is NOT run automatically. Review it, then:
#
#     ./setup-toolchain.sh
#
# Idempotent: safe to re-run.
#
# ---------------------------------------------------------------------------
# WHY THIS IS NOT JUST "add apt.llvm.org"
#
# The obvious approach (add apt.llvm.org, install clang-20) FAILS on
# Ubuntu 24.04 "noble" / Mint 22.x, because noble ships libllvm20 pinned by
# the Mesa graphics stack. Mesa (mesa-vulkan-drivers, libosmesa6, libxatracker2,
# mesa-libgallium) depends on an exact libllvm20 version from noble-updates.
#
# Installing clang-20 from apt.llvm.org therefore fails with:
#     clang-20 : Depends: libllvm20 (= 1:20.1.8~++...) but 1:20.1.2-... is to
#                be installed
# and "fixing" it by upgrading libllvm20 would break Mesa -- and Vulkan, which
# this project needs at runtime.
#
# noble-updates DOES ship clang-20, built against that same libllvm20, so the
# correct move is to install clang-20 straight from Ubuntu and leave Mesa alone.
# We only fall back to apt.llvm.org if Ubuntu has no clang >= 20.
# ---------------------------------------------------------------------------

set -euo pipefail

WANT_MAJOR="${WANT_MAJOR:-20}"

step() { printf '\n==> %s\n' "$1"; }

step "Refreshing package lists"
sudo apt-get update

step "Installing ninja-build"
sudo apt-get install -y ninja-build

step "Finding an available clang >= ${WANT_MAJOR} from the distro repos"
# Ask apt what it would install from the distro alone, without mutating anything.
CANDIDATE="$(apt-cache policy "clang-${WANT_MAJOR}" 2>/dev/null \
  | awk '/Candidate:/ {print $2}')"

if [ -n "$CANDIDATE" ] && [ "$CANDIDATE" != "(none)" ]; then
  echo "  Distro offers clang-${WANT_MAJOR} at ${CANDIDATE} - using it (keeps Mesa intact)."
  sudo apt-get install -y "clang-${WANT_MAJOR}"
else
  echo "  No clang-${WANT_MAJOR} in the distro repos; falling back to apt.llvm.org."
  LLVM_SCRIPT="$(mktemp -t llvm.XXXXXX.sh)"
  trap 'rm -f "$LLVM_SCRIPT"' EXIT
  wget -q -O "$LLVM_SCRIPT" https://apt.llvm.org/llvm.sh
  chmod +x "$LLVM_SCRIPT"
  sudo "$LLVM_SCRIPT" "$WANT_MAJOR"
fi

step "Pointing the unversioned 'clang' at clang-${WANT_MAJOR}"
# check-env.sh tests plain `clang`, so clang-${WANT_MAJOR} alone is not enough.
if [ -x "/usr/bin/clang-${WANT_MAJOR}" ]; then
  sudo update-alternatives --install /usr/bin/clang clang "/usr/bin/clang-${WANT_MAJOR}" 100
else
  echo "  [WARN] /usr/bin/clang-${WANT_MAJOR} not found; skipping"
fi

step "Verifying"
clang --version | head -1
ninja --version

printf '\nDone. Now run ./check-env.sh to confirm.\n'