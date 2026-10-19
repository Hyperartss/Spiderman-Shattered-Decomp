# Runtime status

First successful **build and launch** of the 360 recompilation.

## Status ladder

| Milestone | State |
| --- | --- |
| Compiles | **YES** — 0 warnings, 0 errors |
| Codegen passes | **YES** — 9,564 functions, 0 validation errors |
| Boots | **PARTIAL** — runtime initialises, spawns threads, traps on first unresolved indirect call |
| Playable | No |

## Build

| Field | Value |
| --- | --- |
| Preset | `linux-amd64-debug` |
| Compiler | clang 20.1.2 |
| Steps | 52/52 succeeded |
| Binary | `sd360`, 43.8 MB ELF x86-64, with debug info |
| Warnings | **0** |

Built from `tools/linux-amd64` (SDK 0.10.0). See "SDK trap" below.

## Launch

```
./sd360/out/build/linux-amd64-debug/sd360 --game_data_root ./run
```

Two argument corrections versus README/AGENTS.md:

- The data root is **`--game_data_root <path>`**, not a positional argument.
  A bare positional argument yields `--game_data_root was not provided`.
- The runtime looks for **`<game_data_root>/default.xex` in lowercase**, but the
  extracted file is `Default.xex`. On a case-sensitive filesystem this fails.

Worked around with `run/`, a directory of symlinks into `assets/` that exposes
`default.xex` plus `Data`, `video`, `AvatarAssetPack`, `nxeart` and
`$SystemUpdate`. Game files stay read-only, per AGENTS.md rule 1.

Runtime libs must sit next to the binary (RUNPATH is `$ORIGIN`):
`librexruntimed.so` and `libTracyClientd.so` are copied from the SDK.

## What happens at boot

The runtime gets substantially alive before failing:

- SDL initialises on the x11 driver
- `Kernel Dispatch` thread starts
- `XMA Decoder` thread starts (audio path is live)
- `Audio Worker` thread starts
- At least 7 `XThread` workers start (XThreadB3..XThreadC7, XThreadD1, D2)
- Recompiled guest code executes into `xstart`

So this is **not** a load-time failure. Real guest code runs, the thread
scheduler works, and audio decoding is initialised.

## The fault

`SIGABRT` from `rex::runtime::InvalidFunctionTrap`
(`src/system/function_dispatcher.cpp:38`), reached from a `bctrl` indirect call:

| Frame | Symbol | Site |
| --- | --- | --- |
| 5 | `rex::runtime::InvalidFunctionTrap` | function_dispatcher.cpp:38 |
| 6 | `sub_8218DD48` | `sd360_recomp.4.cpp:14742` |
| 7 | `sub_8218DF20` | `sd360_recomp.31.cpp:15018` |
| 8 | `sub_82200AA8` | `sd360_recomp.23.cpp:20893` |
| 9 | `sub_821FFBA8` | `sd360_recomp.4.cpp:20712` |
| 10 | `sub_8218DB18` | `sd360_recomp.40.cpp:14265` |
| 11 | `xstart` | `sd360_recomp.23.cpp:22944` |

### Faulting address

Read from `PPCContext::last_indirect_target` (offset 332, confirmed by
compiling `offsetof` against the SDK headers — note this needs C++23 for
`std::byteswap`):

| Register | Value | Valid? |
| --- | --- | --- |
| `lr` | `0x8218DF14` | yes — matches `ctx.lr = 0x8218DF14` in generated code |
| call target (`ctr`) | **`0x880DCE78`** | **no** |

`0x880DCE78` is outside the XEX image (`0x82222E98` + ~3.6 MB) and outside the
discovered function range (`0x82080000`–`0x82300280`).

### Reading

This is **not** a missing kernel import. Kernel imports sit at low addresses in
the image; this target is garbage. The guest dereferenced a function-pointer
table (vtable-like; the code loads it from offsets `30836`–`30900` of an
object just before the `bctrl`) and got an unpopulated value.

Most likely one of:

1. A vtable belonging to a class whose implementation was never discovered, so
   the table was never filled and holds uninitialised data.
2. An indirect call through a table that is populated at runtime by a module or
   system initialiser that never ran.
3. A bad function boundary, so codegen mis-split a function and the vtable
   built next to it is wrong.

Category 3 is the cheapest to test and is where AGENTS.md says to start
(prefer fixing boundaries in the TOML over writing overrides).

## SDK trap (worth remembering)

CMake silently selected `~/.local/share/smd-rexglue-sdk` instead of the in-repo
SDK, because:

- `~/.cmake/packages/rexglue` is CMake's **user package registry**, which
  outranks normal search paths.
- `generated/rexglue.cmake` calls `find_package(rexglue 0.10.0)` — a *minimum*,
  not `EXACT` — so `0.10.0.0-dev.unknown` satisfies it.

That SDK is **release-only**: 2 `.so` files, no `librexruntimed.so`. A Debug
build cannot link against it.

Fix applied: configure with
`-Drexglue_DIR="<repo>/tools/linux-amd64/lib/cmake/rexglue"`.

Permanent fix, not yet applied: remove `~/.cmake/packages/rexglue`.

## Next step (Phase 4)

Before changing anything, read the wiki pages AGENTS.md lists — Function
Overrides, Mid-ASM Hooks, Kernel State & Objects, Codegen Pipeline Overview —
then take the smallest fixable item. Do not guess TOML keys or SDK APIs.

Repro:
```
./sd360/out/build/linux-amd64-debug/sd360 --game_data_root ./run
```
Backtraces: `logs/gdb-backtrace.log`, `logs/gdb-target2.log`.