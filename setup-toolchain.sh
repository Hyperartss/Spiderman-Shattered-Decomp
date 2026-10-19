#!/usr/bin/env bash
# Installs the toolchain a ReXGlue recompilation needs: ninja + clang 20.
#
# This edits system packages and adds an apt repository, so it is NOT run
# automatically. Review it, then run it yourself:
#
#     ./setup-toolchain.sh
#
# Idempotent: safe to re-run. Prints OK / DONE per step.
#
# Why clang 20 and not apt's default: Mint 22.3 is Ubuntu "noble" based and
# ships clang 18. The SDK requires clang 20+, so we add apt.llvm.org.
# After this, ./check-env.sh should pass.

set -euo pipefail

LLVM_VERSION="${LLVM_VERSION:-20}"

step() { printf '\n==> %s\n' "$1"; }
have_root() { sudo -n true 2>/dev/null && return 0 || return 1; }

if [ "$(id -u)" -ne 0 ] && ! have_root; then
  echo "This script needs sudo and will prompt for your password."
fi

step "Refreshing package lists"
sudo apt-get update

step "Installing ninja-build"
sudo apt-get install -y ninja-build

step "Installing clang-${LLVM_VERSION} from apt.llvm.org"
LLVM_SCRIPT="$(mktemp -t llvm.XXXXXX.sh)"
trap 'rm -f "$LLVM_SCRIPT"' EXIT
wget -q -O "$LLVM_SCRIPT" https://apt.llvm.org/llvm.sh
chmod +x "$LLVM_SCRIPT"
sudo "$LLVM_SCRIPT" "$LLVM_VERSION"

step "Pointing the unversioned 'clang' at clang-${LLVM_VERSION}"
# check-env.sh tests plain `clang`, so clang-20 alone is not enough.
if [ -x "/usr/bin/clang-${LLVM_VERSION}" ]; then
  sudo update-alternatives --install /usr/bin/clang clang "/usr/bin/clang-${LLVM_VERSION}" 100
else
  echo "  [WARN] /usr/bin/clang-${LLVM_VERSION} not found; skipping"
fi

step "Verifying"
clang --version | head -1
ninja --version

printf '\nDone. Now run ./check-env.sh to confirm.\n'