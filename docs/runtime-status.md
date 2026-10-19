# Runtime status

Progress log for building and launching the 360 recompilation.

## Status ladder

| Milestone | State |
| --- | --- |
| Compiles | **YES** — 0 warnings, 0 errors |
| Codegen passes | **YES** — 9,564 functions, 0 validation errors |
| Boots | **PARTIAL** — GPU, audio and pipelines initialise; stops on XEX module lookup |
| Playable | No |

## Launch invocation

```
./sd360/out/build/linux-amd64-debug/sd360 --game_data_root ./run
```

Two corrections versus README/AGENTS.md:

- The data root is **`--game_data_root <path>`**, not a positional argument.
- The runtime opens **`<game_data_root>/default.xex` in lowercase**, but the
  extracted file is `Default.xex`.

## `run/` must be a hardlink mirror, not symlinks

This is the subtle one, and it caused a hard failure.

`HostPathDevice::ResolvePath()` (Virtual File System wiki) **walks the host
directory tree** to build its entry table. An earlier version of `run/` used
symlinks into `assets/`, so the walker saw a symlink where a directory should
be and never descended into it. Result:

```
[NtCreateFile] FAILED: path='D:\Data\Streams.dat' -> 0xc000000f
XamShowDirtyDiscErrorUI called! -> FATAL: Disc Read Error
```

`Data/Streams.dat` exists and is 1.55 GB; it was simply invisible to the VFS.

Fix: `cp -al assets run` builds a mirror of **real directories containing
hardlinked files**, plus one extra hardlink `default.xex` -> `Default.xex`.
Inodes are shared (verified: same inode, link count 2), so this costs
**zero additional disk** and never writes to the game files, keeping AGENTS.md
rule 1 intact.

## GPU emulation

Initially every `Vd*` kernel call was dropped:

```
VdSetGraphicsInterruptCallback: no GPU emulation loaded (gpu_plugin not set)
```

CMake's `rexglue_setup_target(sd360 GPU_PLUGINS xenos)` only **stages**
`librexgpu-xenosd.so` next to the binary — plugins are runtime-loaded, never
linked, so staging alone does nothing. The runtime is told via
`RuntimeConfig::gpu_plugin` (`runtime.h:70`), which is set by overriding
`OnPreSetup` in `Sd360App` (`src/sd360_app.h`).

After this the runtime brings up Vulkan (1.3.275), picks the RX 5700 XT, and
starts creating graphics pipeline states with compiled VS/PS shaders.

## Current blocker: XEX module ordinal 9

The boot path calls, from `sub_8218DD48`:

```
sub_82220370() -> XexLoadImage(ordinal=9, flags=0, &buf)       -> handle
sub_82220330() -> XexGetProcedureAddress(handle, ordinal)      -> proc addr
mtctr <proc addr> ; bctrl
```

`sub_82220330` is a one-line tail-call wrapper around the kernel import
`__imp__XexGetProcedureAddress`. Traced with temporary hooks:

```
XexLoadImage(ordinal=9)                                -> 0x30099000
XexGetProcedureAddress(handle=0x30099000, ordinal=1)   -> 0x880D0040
XexGetProcedureAddress(handle=0x30099000, ordinal=2)   -> 0x880DCE78
```

`0x880DCE78` is what lands in CTR. The runtime reports its function table as
`code=82080000-82300AEC, image=82000000-82520000`, so `0x880DCE78` is outside
the image entirely. It is a host-function trampoline the SDK minted, and the
indirect-call dispatcher cannot resolve it:

```
Execute(88678BB8): function not in function table
```

### Why the obvious fixes do not work

1. **`REX_EXPORT_STUB_RETURN(XexGetProcedureAddress, 0)`** — does not take
   effect. `__imp__XexGetProcedureAddress` is a strong (`T`) symbol in
   `librexruntimed.so`, so the linker binds the generated call straight to the
   SDK. `REX_EXPORT` populates the name/ordinal registry; it does not override
   an already-bound import. Overriding it means patching the SDK.
2. **Overriding the guest wrapper** `sub_82220330` with `REX_HOOK_RAW` to
   return NULL *does* work (recompiled code is a weak alias), and it removes
   the trap — but it converts it into a different fatal:

   ```
   [FATAL] Call to invalid or unregistered function at guest address 0x00000000
   ```

   So the guest does **not** tolerate a missing procedure everywhere. Only the
   call site inside `sub_8218DD48` checks for NULL; another site calls the
   result unconditionally.

### What this means

The game genuinely requires the exports of whatever module ordinal 9 refers
to. Stubbing the lookup cannot get past it — the correct answer is to supply
the real procedures, which means identifying module 9 first.

The disc contains a single XEX, so ordinal 9 is very likely a **system module**
that a retail disc expects the console kernel to provide, not something
recoverable from `Default.xex` alone.

## Options

1. **Identify module ordinal 9** (Ghidra, or the XEX module/section table) and
   implement its exports with `REX_EXPORT`. Most correct, most work.
2. **SDK patch** so the dispatcher resolves minted host trampolines, letting
   the current addresses dispatch instead of trapping. Needs approval per
   AGENTS.md rule 6, and is written into `patches/`.
3. **Accept the NULL stub** and trace which procedure the game wanted from the
   `0x0` call site, then work backwards. Cheapest, slowest.

Option 2 is the only one that fixes the class of problem rather than this one
instance, but it touches the SDK.

## Diagnostics still in the tree

`src/diagnostics.cpp` overrides `sub_82220330` to return NULL and log. It is
what produces the clean NULL rather than a trap. Remove it when module 9 is
handled properly.

## Environment notes

- Build with `-Drexglue_DIR=<repo>/tools/linux-amd64/lib/cmake/rexglue`.
  Without it CMake picks `~/.local/share/smd-rexglue-sdk` via
  `~/.cmake/packages/rexglue`, which is release-only (no `librexruntimed.so`).
- Native libs must sit next to the binary (RUNPATH is `$ORIGIN`). With the GPU
  plugin enabled, CMake stages all three itself.
- SDK runtime logs go to `sd360/out/build/linux-amd64-debug/logs/sd360_NNN.log`,
  not stdout. Check there first when a run fails silently.