#!/usr/bin/env bash
# Checks the toolchain needed for a ReXGlue recompilation project.
# Prints OK / MISSING / WARN per item. Installs nothing.

ok()   { printf '  [ OK ]   %s\n' "$1"; }
miss() { printf '  [MISSING] %s\n' "$1"; FAIL=1; }
warn() { printf '  [WARN]   %s\n' "$1"; }
FAIL=0

ver_ge() { # ver_ge have need  -> true if have >= need
  [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" = "$2" ]
}

echo "ReXGlue environment check"

# CMake 3.25+
if command -v cmake >/dev/null 2>&1; then
  v=$(cmake --version | head -n1 | awk '{print $3}')
  if ver_ge "$v" 3.25; then ok "cmake $v"; else miss "cmake $v (need 3.25+)"; fi
else miss "cmake (need 3.25+)"; fi

# Ninja
if command -v ninja >/dev/null 2>&1; then ok "ninja $(ninja --version)"; else miss "ninja"; fi

# Clang 20+
if command -v clang >/dev/null 2>&1; then
  v=$(clang --version | head -n1 | sed -E 's/.*version ([0-9]+(\.[0-9]+)*).*/\1/')
  if ver_ge "$v" 20; then ok "clang $v"; else miss "clang $v (need 20+; see apt.llvm.org)"; fi
else
  # versioned binary fallback, e.g. clang-20
  found=""
  for c in clang-20 clang-21 clang-22; do command -v "$c" >/dev/null 2>&1 && found="$c" && break; done
  if [ -n "$found" ]; then warn "found $found but plain 'clang' is not on PATH"; else miss "clang 20+"; fi
fi

# GTK3 headers
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gtk+-3.0; then
  ok "gtk3 dev headers"
else miss "gtk3 dev headers (libgtk-3-dev)"; fi

# git
if command -v git >/dev/null 2>&1; then ok "git $(git --version | awk '{print $3}')"; else miss "git"; fi

# ReXGlue CLI
if command -v rexglue >/dev/null 2>&1; then ok "rexglue CLI on PATH"; else warn "rexglue not on PATH (install the SDK and add its bin/ to PATH)"; fi

# Vulkan (optional but needed for the runtime renderer)
if command -v vulkaninfo >/dev/null 2>&1; then
  if vulkaninfo --summary >/dev/null 2>&1; then ok "vulkan works"; else warn "vulkaninfo failed; check GPU drivers"; fi
else warn "vulkaninfo not installed (package: vulkan-tools); can't verify Vulkan"; fi

# ISO extraction
if command -v extract-xiso >/dev/null 2>&1; then ok "extract-xiso"; else warn "extract-xiso not found (needed to unpack the ISO)"; fi

# Optional analysis tools
if command -v ghidraRun >/dev/null 2>&1; then ok "ghidra"; else warn "ghidraRun not on PATH (fine if you launch it another way)"; fi

echo
if [ "$FAIL" -eq 0 ]; then echo "Required tools present."; else echo "Some required tools are missing."; fi
exit "$FAIL"
