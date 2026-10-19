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

### What was ruled out for module 9

The XEX header was parsed directly to see whether ordinal 9 refers to a module
inside this XEX. The header mixes endianness; reading fields big-endian
reproduces every value the runtime independently reported, which validates the
anchors:

| Offset (BE) | Value | Meaning (confirmed) |
| --- | --- | --- |
| 0x00 | `XEX2` | magic |
| 0x2C | `0x82222E98` | kernel module base — matches runtime |
| 0x30 | `0x00010201` | module section flag |
| 0x34 | `0x82000000` | image base — matches runtime |
| 0x94 | `0x00520000` | image size (0x82000000→0x82520000) |
| 0x9F8 | `0x00080000` | code section offset (+0x80000) |

What could **not** be determined:

- There is no `MZ` or `PE\0\0` signature anywhere in the first 0x2000 bytes, so
  this XEX does not embed a standard PE header and section count cannot be read
  the usual way.
- A 24-byte section-header layout anchored at 0x2C produces self-consistent
  values for four entries and then garbage, so that layout is wrong. It was not
  used.
- `sub_82220370` is a bare `XexLoadImage` wrapper that logs nothing, so the game
  never names the module it wants.

Conclusion: ordinal 9 semantics need the xexkernel definition of `XexLoadImage`
or a real XEX parser. Guessing the section layout would be inventing structure,
which AGENTS.md rule 5 forbids. Ghidra with an XEX loader — already listed as a
prerequisite in the README for Phase 4 naming — is the practical route.

## Ghidra status

Ghidra is installed and working headless, but the XEX loader does not register.

Setup that succeeded:
- `openjdk-21-jdk` installed. The pre-existing OpenJDK was a **JRE** (`java` but
  no `javac`), which is why Ghidra reported "unsupported java version" — a
  misleading message, since `application.java.min=21` and 21 is fine.
- Ghidra 12.1.4 copied from Old stuff into `tools/ghidra_12.1.4_PUBLIC`. The
  original was **not** modified (mtime unchanged).
- `zeroKilo/XEXLoaderWV` 12.1.3 unpacked into `Ghidra/Extensions/XEXLoaderWV`,
  alongside the built-in `Eclipse`/`Ghidra`/`IDAPro` extension directories.

What still fails:
- Import reports `No load spec found for import file: Default.xex`.
- Naming the loader explicitly gives
  `InvalidInputException: Invalid loader name specified: xexloaderwv.XEXLoaderWVLoader`,
  so the extension is not being registered at all.
- `analyzeHeadless` logs no error or version warning about the extension; the
  class searcher simply does not pick it up.
- No 12.1.4 build of XEXLoaderWV exists (newest published is 12.1.3), so a
  version-gated build would mean installing Ghidra 12.1.3 instead.

Note: Ghidra would not answer the module-9 question anyway. That is
`XexLoadImage` ordinal semantics, not game code. Ghidra's value here is naming
functions and reading the boot path in Phase 4 — real, but a side quest that
does not unblock the current blocker.

## Ghidra status: WORKING

Ghidra 12.1.4 plus the `zeroKilo/XEXLoaderWV` XEX loader now import and
analyse the XEX. Analysis succeeded (7,552 functions; PNG "invalid data"
warnings are benign embedded-icon misdetections).

**Why the loader failed before:** extensions must be installed in the *user*
extension directory, not the Ghidra install directory:

```
~/.config/ghidra/ghidra_12.1.4_PUBLIC/Extensions/XEXLoaderWV
```

Installing into `ghidra_12.1.4_PUBLIC/Extensions/` (alongside the built-in
`Eclipse`/`Ghidra`/`IDAPro` dirs) silently does nothing — Ghidra does not scan
that path for user extensions. `GhidraMCP` was already installed in the user
directory, which is what gave the correct pattern.

No 12.1.4 build of XEXLoaderWV exists (12.1.3 is newest) but it works against
12.1.4 anyway, so the earlier version concern was a red herring.

Requires a **JDK**; the machine had only a JRE, which Ghidra misreports as
"unsupported java version" even though `application.java.min=21`.

### Section layout

| Section | Range | Size | Flags |
| --- | --- | --- | --- |
| `.rdata` | `82000400`-`8206502b` | `0x64c2c` | r |
| `.pdata` | `82065200`-`82072ecf` | `0xdcd0` | r |
| `.text` | `82080000`-`822fb843` | `0x27b844` | r-x |
| `PSFD00` | `822fba00`-`82300aeb` | `0x50ec` | r-x |
| `.data` | `82310000`-`8249ac97` | `0x18ac98` | rw |
| `.XBMOVIE` | `8249ae00`-`8249ae0b` | `0xc` | rw |
| `.idata` | `824a0000`-`824a042f` | `0x430` | rw |
| `.XBLD` | `824b0000`-`824b00ef` | `0xf0` | r |
| `.reloc` | `824b0200`-`824cd343` | `0x1d144` | r |

Language: `PowerPC:BE:64:A2ALT-32addr`.

### Boot call chain (from Ghidra)

```
0x82222E98  kernel entry (matches the XEX kernel base)
  -> 0x8218DB18
    -> 0x821FFBA8
      -> 0x82200AA8
        -> 0x8218DF20
          -> 0x8218DD48
            +-> 0x82220370   XexLoadImage wrapper
            +-> 0x82220330   XexGetProcedureAddress wrapper
```

### Trampoline addresses are outside the image

Ghidra's map ends at `0x824cd343`. All three host-function trampolines are
**unmapped**:

| Address | Source | In image? |
| --- | --- | --- |
| `0x880dce78` | `XexGetProcedureAddress(handle, 2)` | **no** |
| `0x880d0040` | `XexGetProcedureAddress(handle, 1)` | **no** |
| `0x88678bb8` | second unresolved execute | **no** |

This confirms independently of the runtime that these are host trampolines, not
guest code.

### Scripts

`ghidra-scripts/ProbeBootPath.java` dumps program info, memory blocks, and
caller/callee data for the boot-path addresses. Run headless with:

```
analyzeHeadless ghidra sd360 -process Default.xex -noanalysis \
  -scriptPath ghidra-scripts -postScript ProbeBootPath.java
```

## Module ordinal 9 — root cause, established

Ghidra's decompile of `Function_8218DD48` (the boot-path function that traps):

```c
uVar1  = Function_82220370();              // XexLoadImage
*param_2 = uVar1;                          // handle -> out-param
if (uVar1 == 0) return;

uVar2  = Function_82220330(uVar1, 1);      // GetProc(handle, ordinal 1)
*param_3 = uVar2;                          // stored with NO null check

pcVar3 = Function_82220330(*param_2, 2);  // GetProc(handle, ordinal 2)
if (pcVar3 != 0) {
    // 28 consecutive globals 0x82467848..0x824678a4 are pushed
    pcVar3(...);                           // ordinal 2 IS null-checked
}
```

So the two procedures are treated asymmetrically:

- **Ordinal 1** is written to an out-param and used later **without a null
  check**. Returning NULL for it is what produces
  `[FATAL] Call to invalid or unregistered function at guest address 0x00000000`.
- **Ordinal 2** is null-checked immediately and called with a 28-word argument
  block loaded from `0x82467848`.

This also explains why a NULL stub cannot work: only one of the two call sites
guards against NULL.

### Why this cannot be fixed from the project

The SDK exposes **no public API to make a host function callable through a
guest indirect call**:

| API | Location | Purpose |
| --- | --- | --- |
| `ResolveIndirectFunction(uint32_t)` | `ppc/func.h:33` | lookup only |
| `InitializeFunctionTable(...)` | `system/function_dispatcher.h:83` | whole-module setup |
| `DestroyFunctionTable(...)` | same | teardown |

There is no `RegisterHostFunction` / `AddTrampoline` equivalent. Meanwhile
`PPCFuncRegistrar` (`ppc/function.h:54`) — what `REX_EXPORT` uses — only appends
`(name, host_function_pointer)` to a vector:

```cpp
struct PPCFuncRegistrar {
  PPCFuncRegistrar(const char* name, PPCFunc* func) {
    GetPPCFuncRegistry().emplace_back(name, func);
  }
};
```

It stores **host pointers keyed by name**. The dispatcher resolves **guest
addresses**. Nothing bridges the two, which is precisely why
`XexGetProcedureAddress` hands back `0x880DCE78`/`0x880D0040` — guest addresses
the dispatcher has never been told about.

Overriding the import itself is also closed off: `__imp__XexGetProcedureAddress`
is a strong (`T`) symbol in `librexruntimed.so`, so the linker binds the
generated call straight to the SDK.

### Conclusion

This is an SDK integration gap, not a project bug. Nothing in `sd360` can make
ordinal 9's procedures dispatchable. It needs one of:

1. **Dispatcher patch** — register minted host trampolines so
   `ResolveIndirectFunction` can find them. Fixes the whole class. Needs
   approval per AGENTS.md rule 6.
2. **Module 9 identified and implemented** — determine what module ordinal 9 is
   (its ordinal-2 procedure takes a 28-word argument block during graphics
   setup, which suggests a device/graphics interface module) and supply real
   implementations via `REX_EXPORT`.
3. **Return NULL for unresolvable ordinals** — makes the failure clean instead
   of trapping, but does not unblock boot; the game needs these procedures.

Option 1 is the only one that unblocks boot without first reverse-engineering
the module.

## SOLVED: the module was `game:\Data\GameLogic.dll`

The "module ordinal 9" was a misreading. `XexLoadImage`'s first argument is a
**module name string**, not an ordinal:

```cpp
u32 XexLoadImage_entry(mapped_string module_name, u32 module_flags,
                       u32 min_version, mapped_u32 hmodule_ptr);
```

`li r4,9` in the generated code is `module_flags`. Reading the name out of
guest memory (`base + 0x7018FAB0`) gives:

```
game:\Data\GameLogic.dll
```

and `assets/Data/GameLogic.dll` (6,692,864 bytes) was sitting in the extracted
game data the whole time. Codegen only processed `Default.xex`, so the DLL was
never recompiled and its exports pointed at addresses the dispatcher had no
functions for.

### 1. Register the module in the manifest

`rexglue init module` is broken — it rejects `--xex-path` even when supplied, so
the entry was written by hand using the schema taken from the SDK source
(`src/codegen/manifest.cpp:120-147`, and the exact TOML the CLI emits at
`src/rexglue/commands/init_command.cpp:331`):

```toml
[[modules]]
guest_path = "data/gamelogic.dll"
file_path = "../assets/Data/GameLogic.dll"
out_directory_path = "generated/GameLogic"
includes = []
```

`guest_path` must be the canonical form produced by `NormalizeGuestPath`
(`src/system/guest_path.cpp:21`): forward slashes, device prefix stripped,
lowercased.

Codegen then processes both binaries — **195 files written**, and emits
`libsd360_GameLogic.so` plus `dll_targets.cmake`.

### 2. The module .so is dlopen'd by bare name

`shared_lib_name` is `projectName + "_" + targetName` = `sd360_GameLogic`
(`project_recompiler.cpp:500`), and `kernel_state.cpp:762` dlopens that string
directly. This glibc applies **no** `lib`-prefix or `.so` transform to a bare
dlopen name — `LD_DEBUG=files` enumerated *no* candidate paths at all.

So the build now also emits a symlink named exactly `sd360_GameLogic`:

```cmake
add_custom_command(TARGET sd360_GameLogic POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E create_symlink
        $<TARGET_FILE:sd360_GameLogic>
        $<TARGET_FILE_DIR:sd360_GameLogic>/sd360_GameLogic
    VERBATIM)
```

The module is dlopen'd from `librexruntimed.so`, and `DT_RUNPATH` is not
inherited for dlopen, so launching needs the build dir on the loader path:

```bash
LD_LIBRARY_PATH="$PWD/sd360/out/build/linux-amd64-debug" \
  ./sd360/out/build/linux-amd64-debug/sd360 --game_data_root ./run
```

### Result

```
[xex] XexLoadImage(name="game:\Data\GameLogic.dll") -> hmodule=0x300A3000
[xex] XexGetProcedureAddress(hmodule=0x300A3000, ordinal=1) -> 0x82300AEC
[xex] XexGetProcedureAddress(hmodule=0x300A3000, ordinal=2) -> 0x82300AF0
```

| | Before | After |
| --- | --- | --- |
| hmodule | `0x00000000` | `0x300A3000` |
| ordinal 1 | `0x880D0040` | `0x82300AEC` |
| ordinal 2 | `0x880DCE78` | `0x82300AF0` |

The runtime logs:

```
Registering module: data/gamelogic.dll (code_base=880D0000)
Module 'data/gamelogic.dll' registered 21046 functions
```

**`0x880DCE78` is `0x880D0000 + 0xCE78` — inside GameLogic.dll's code.** The
original addresses were correct all along; they were merely unregistered because
codegen had never processed the DLL.

## Remaining

```
[FATAL] Call to invalid or unregistered function at guest address 0x88212EE0
```

`0x88212EE0` is also inside GameLogic.dll's range (`0x880D0000 + 0x142EE0`), so
this is a *function codegen did not discover* in the DLL, not a missing module.
Next step is a function-boundary problem in `GameLogic.dll`.

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