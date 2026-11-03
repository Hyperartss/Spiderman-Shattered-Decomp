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

## SOLVED (partly): boot works, then a silent guest crash deadlocks it

The loading screen renders, so drawing and presentation are fine. The black
screen is a **deadlock caused by a guest thread dying without killing the
process.**

Recovered with `-DREXGLUE_RECOMP_DEBUG_INFO=full`, which restores stack
unwinding (with the default `line-tables-only` the backtrace stops at frame 5).
Note: a fresh configure of that preset does **not** stage `librexruntimed.so`
and `libTracyClientd.so` next to the binary, so they must be copied from the SDK
or the binary will not start.

### The chain

1. Many threads — `Main XThread` and several `XThread`s — block in
   `NtWaitForSingleObjectEx` with **no timeout** and `is_alertable=false`:

   ```cpp
   NtWaitForSingleObjectEx_entry(
       object_handle = 4160749624,   // 0xF80EEBC8
       wait_mode = 1, alertable = 0,
       timeout_ptr = {host_ptr_ = 0x0, guest_addr_ = 0})   // infinite
   ```

   Full path: `XObject::Wait` → `rex::thread::Wait` →
   `PosixConditionBase::Wait` → futex.

2. The thread that would signal that object — `XThread11FFD6C0` — has
   **crashed with SIGSEGV**:

   ```
   #0  __imp__sub_82207840   @ sd360_recomp.17.cpp:21559
       REX_STORE_U32(ea, ctx.r10.u32);      // stwu r10,4(r11), a stack push
   ```

   A second crash appears in `sub_82202720`.

3. A guest-thread SIGSEGV does not abort the process. The thread simply stops,
   nothing ever signals the object, and every waiter blocks forever. Audio
   survives because it runs on separate threads (`XMA Decoder`, `Audio Worker`).

So the black screen is **not** a rendering or VSync problem. It is a guest
pointer fault killing the thread that the rest of the game is synchronised on.

### Not the cause

- **Async file IO.** `Kernel Dispatch` sits in
  `dispatch_cond_.wait` (`kernel_state.cpp:655`), which means
  `dispatch_queue_` is **empty** — no completion is outstanding. The SDK also
  `REX_FATAL`s if a deferred completion throws, and there are no FATALs.
- **Presentation.** Disabling `vulkan_async_skip_incomplete_frames` removed the
  skip warnings and the run went from crashing to surviving indefinitely, but
  the screen stayed black because nothing was drawing into the frames.

### Next

`sub_82207840` and `sub_82202720` need to be understood — either a bad function
boundary (so codegen split them wrongly) or a wrong value returned by some
kernel import. Both are answerable in Ghidra, which already has `GameLogic.dll`
and `Default.xex` imported with full analysis.

## Current blocker: bogus GPU descriptor pointer

Traced to a single instruction with full debug info (guest registers are now
readable, so the PPC context is visible at the fault):

```
ctx.r1  = 0x705FE620   real stack pointer
ctx.r11 = 0xA25501FC   the buffer being written
ctx.r10 = 2609 (0xA31) the value stored
```

`sub_82207840` (recomp.17.cpp:21559, `stwu r10,4(r11)`) is building **Xenos GPU
command-buffer descriptors** — the constants `0xA31`, `0x3000000`, `0xC0043C00`
are all Xenos register values. The write target is not a stack slot; it is the
descriptor buffer, and `r11` reaches it via:

```
21381  r11 = r27 + r30
21464  r9  = r11
21473  r11 = rot(r10,32) & 0xFFFFFFFF     \
21481  r11 = r11 + 4095                    | page-align arithmetic on GPU
21485  r11 = rot(r11) & 0xFFFFF000          | state loaded from r31+15116
21493  r11 = r11 - r10                    /  and r31+15120
21539  r11 = r9                            <- becomes the store target
```

`0xA25501FC` is in **no known region**: the guest arena is at `0x100000000`, the
XEX image at `0x82000000`-`0x82520000`, the guest stack at `0x705FE620`.

There are **zero** GPU warnings or errors in the run, so the Vulkan/Xenos layer
never rejected anything — the guest simply computed a descriptor address that
does not correspond to mapped memory.

### Note on Ghidra's decompile of this function

`Ghidra reports sub_82207840 calling FUN_822be754()` and using its return
value. **That is a decompiler artifact.** In codegen, `bl 0x822be754` becomes
`__savegprlr_23` — 0x822BE754 is the standard PowerPC prologue stub, not a game
function, and it is absent from the generated code entirely. Ghidra models the
stub as a call that returns `r3`, which poisons everything downstream in the
listing. The generated C++ is authoritative for this function, not the Ghidra
listing.

### Why it deadlocks the whole game

The thread holding this descriptor dies of SIGSEGV. A guest-thread SIGSEGV does
not abort the process, so the event other threads block on
(`NtWaitForSingleObjectEx` on handle `0xF80EEBC8`, infinite and non-alertable)
is never signalled, and the game hangs with an empty frame.

## Root cause: the guest writes to a guard/read-only page

ReXGlue guest addresses are **offsets from `base`**, where `base = 0x100000000`.
The guest arena lives in `/dev/shm/xenia_memory_*` and is not uniformly
writable — it contains guard and read-only regions:

```
1a22e0000-1a22f0000 ---s     guard
1a22f0000-1a2350000 rw-s
1a2350000-1a2351000 r--s     <-- the faulting page
1a2351000-1a23b0000 rw-s
1a23b0000-1a23b1000 r--s
```

The faulting `this` pointer, guest offset `0xA235027C` (host `0x1A235027C`),
lands squarely on an **`r--s` page**. The guest is *writing* to it. That is the
SIGSEGV — not a wild pointer, but a wrong pointer that falls into a guard or
read-only region.

### Where the bad pointer comes from

Traced five frames back from the faulting store:

```
sub_820F4E58
  r30 = *(r1+80)                       <- out-parameter filled by the call below
  sub_8220C700(r3=r30)  -> sub_82207D78(r3) -> sub_82207840(r3) -> FAULT
sub_82208A68            <- writes that out-parameter; zeroes it first
  sub_82081190(r3=24448, r4=9344)       <- allocates 24,448 bytes
```

`sub_82207840` then does page-align arithmetic on that value
(`rlwinm` / `+4095` / `& 0xFFFFF000` / subtract) loaded from `r31+15116` and
`r31+15120`, and the result is the descriptor address it stores to.

The GPU layer reports **zero** warnings or errors, so nothing was rejected on the
Vulkan side — the guest simply computed an address that is not a valid,
writable arena offset.

### Two crash sites, both the same shape

`sub_82207840` (recomp.17.cpp:21559) and `sub_82202720` (recomp.29.cpp:21482)
both die writing a GPU descriptor. Both receive a `this` pointer around
`0xA2xxxxxx`. Because both are Xenos command-buffer builders doing the same
kind of address arithmetic, a single upstream cause is likely.

### Candidate causes, in the order AGENTS.md prefers

1. **Bad function boundary** in one of these functions, so codegen split it
   wrongly and the arithmetic produced a wrong value. Cheapest to test, and the
   class of bug already seen 13 times in this project with the XEX hotpatch
   entries.
2. **A kernel import returning a wrong value**, seeding bad GPU state.
3. Genuine emulator gap in the Xenos ring-buffer emulation.

## Where the project stands

| Milestone | State |
| --- | --- |
| Compiles | YES — 0 warnings |
| Codegen passes | YES — 9,564 (XEX) + 21,049 (GameLogic.dll) functions |
| Boots | YES — renders the loading screen |
| Audio | YES — Beenox intro and credits play |
| Menu | Blocked by the above crash |

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
## Measured boot state (2026-10-19)

Verified at runtime, not inferred:

- XexLoadImage for both Default.xex and GameLogic.dll completes in ~1s
  (01:04:10 -> 01:04:11; both module function tables initialised, 21049
  guest functions registered).
- The guest calls XexLoadImage exactly once. `recomp.4.cpp:14517` is a single
  call with a null check and straight-line continuation.
- Audio plays (XMA decoder, pipewire), so file I/O and the audio path work.
- The GPU picks the RX 5700 XT, creates pipelines and MSAA colour/depth targets,
  and presents ~2660 frames at 1120x632 (~35ms apart).
- The framebuffer the GPU resolves into (guest `0x04445000`, 2867200 bytes) is
  100% zero for the whole run. The guest issues no draw work.

## Methodological corrections

Three earlier conclusions were wrong and have been discarded. Recording them
because the failure modes repeat:

1. **Breakpoint-per-block gave a fake slow decrypt.** Setting a breakpoint on
   `rijndaelDecrypt` traps 418,304 times over a 6.7MB XEX (one trap per 16-byte
   block), which made the load look like it needed minutes. Uninstrumented it is
   ~1s. Never measure throughput with a breakpoint on the hot function.

2. **Thread numbers are not stable.** The guest recycles thread slots, so
   "Thread 13" was a different thread in different runs. Any conclusion keyed to
   a bare thread index is unreliable; match on thread name at sample time.

3. **grepping the runtime log after a gdb run proves nothing.** gdb stops before
   delivering SIGSEGV to the program's handler, so the runtime's own fault
   messages never appear. To get them, `handle SIGSEGV nostop noprint pass`.

Also: the guest arena is a single ~4.5GB file in /dev/shm and is leaked on
abnormal exit. Stale arenas filled /dev/shm to 9.3G of 16G and silently degraded
later runs. Clear with `rm -f /dev/shm/xenia_memory_*` when a run ends badly.

Forcing PROT_WRITE over the arena is NOT a workaround. The read-only pages are
how `EnableAccessCallbacks` traps guest access to GPU-managed memory; forcing
them writable disables the GPU cache-invalidation trap and renders black.

## SOLVED: black screen at the Beenox logo (leaked XAudio2 engine lock)

### Symptom

FrontScreen loads, the Marvel and Activision logo movies play, then the screen
goes black (audio may keep going) exactly where the Beenox logo should start.

### What was really happening (measured, not inferred)

Open-file sampling of `/proc/<pid>/fd` once a second:

```
t=3   Data/FrontScreen.pkz
t=5   video/LogoIntros/MarvelIntro.usm
t=19  video/LogoIntros/ActivisionIntro.usm     <- stays open forever
      video/LogoIntros/BeenoxIntro.usm         <- never opened
```

So the Activision movie never finishes tearing down. Live gdb (all guest
threads mapped to their KTHREAD pointers) at ~36 s and ~48 s:

```
CSW waiter=XThread1FFD6C0            (XAudio2 processing thread, sub_82266878)
CS lock_count=2 recursion=2 owner=30029018   (= Main XThread)
```

The main thread owns XAudio2's engine critical section twice and never
releases it; the audio thread blocks in `RtlEnterCriticalSection` forever, so
the movie's voice never stops and the movie never reports completion.

A breakpoint trace of the engine lock's Enter (`sub_82235FD0`) / Leave
(`sub_82235FE0`) on the main thread shows the two unmatched Enters at the end
of the Activision movie (~30 s), both through `sub_82245AB0` (engine Lock()),
reached from `sub_82247E78` and `sub_82236250` (voice teardown). Their matching
Unlock() is the very next function, `0x82245AC8`.

### Root cause: `{ size = 0x8 }` in the manifest

`0x82245AC8` was in `[entrypoint.functions]` as `{ size = 0x8 }`. A CONFIG
`size` is authoritative (it outranks PDATA, `phase_register.cpp`), so codegen
emitted only the first two instructions:

```cpp
DEFINE_REX_FUNC(sub_82245AC8) {
	// lwz r11,72(r3)
	// addi r3,r11,64
}        // <- the vtable+20 tail call to RtlLeaveCriticalSection is gone
```

Unlock() became a no-op. The other `size = 0x8` entries have the same defect
wherever the function is longer than 8 bytes (`0x822FCB68`, `0x820CA870`, and
likely several in GameLogic.dll); only `0x822B9B50` and `0x8223A390` were
genuinely short thunks.

**Correction to earlier notes:** these addresses are not "XEX hotpatch
entries". They are ordinary functions reached only through vtables / indirect
calls, so discovery never saw a direct call to them.

### Fix

Every entry is now `= {}`: the entry point is registered, and discovery finds
the natural end (`phase_discover.cpp`: "If no size specified (size=0), let
discovery find natural boundaries via region"). `fix-missing-functions.sh` now
appends `{}` instead of `{ size = 0x8 }`.

### Other earlier conclusions that were wrong

- **"A guest-thread SIGSEGV deadlocks every waiter" / "the guest writes to a
  read-only page".** Those SIGSEGVs are ReXGlue's normal write-watch traps
  (`EnableAccessCallbacks` protects GPU-shared pages read-only and handles the
  fault). A gdb run with `AccessViolationCallbackThunk` instrumented shows
  hundreds of them handled per second. gdb stops on them by default, which
  made them look like crashes. Always run with
  `handle SIGSEGV nostop noprint pass`.
- **"The guest issues no draw work."** The render thread
  (`sub_8220C700 -> sub_82207E78`) is live, and the ring-buffer read pointer
  advances between samples (`*rptr` 0x683 -> 0x7E7 -> 0x94B).

### Debug tooling (in `logs/claude/`, git-ignored)

- `sample.sh <first> <interval> <n> <out>`: runs sd360 under gdb, interrupts
  n times, and runs `xthreads` (guest thread -> KTHREAD -> guest call chain)
  and `cswaiters` (every thread blocked in RtlEnterCriticalSection, plus the
  CS owner) from `ring.py`.
- `sample_lock.sh`: the same, plus `lockt.py`, which logs every main-thread
  Enter/Leave of the XAudio2 engine lock to `lock_trace.txt`.
- `mon.sh <secs> <tag>`: plain run that samples the game files open every
  second (the cheapest way to see how far boot got).

## Measured boot state (2026-10-20, after the manifest fix)

| Stage | Debug build | RelWithDebInfo build |
| --- | --- | --- |
| Loading screen | renders | renders |
| Marvel / Activision / Beenox movies | play (audio), **video black** | same |
| Front-end load (`Streams.dat`) | **abort**: SDK debug assert `texture_cache.cpp:876 !(parameters.mip_linear)` | OK |
| Title screen ("PRESS START") | not reached | **renders**, 3D background washed out / near white |

- `0x881EFC00` (GameLogic.dll) was the next missing entry point; added as `{}`
  under `[modules.functions]`. `fix-missing-functions.sh` used to append new
  entries at EOF, which landed them in `[entrypoint.functions]` (wrong binary,
  silently ignored); it now inserts under the correct header.
- RelWithDebInfo: `cmake --preset linux-amd64-relwithdebinfo -Drexglue_DIR=...`
  then build; copy `librexruntimerd.so` and `libTracyClientrd.so` from
  `tools/linux-amd64/lib` next to the binary (not staged automatically).
  Run with `PRESET=linux-amd64-relwithdebinfo ./run.sh`.
- `render_target_path_vulkan=fsi` hits a debug assert in
  `RenderTargetCache::last_update_accumulated_render_targets()`; untested in
  RelWithDebInfo.

### Movie video is black: what is known

Per frame the game clears the RT, then draws one RectangleList with
VS `1377FFBBFD60DADA` / PS `10F50C571CB15333` (YUV->RGB with a pow/gamma step
via `log`/`exp`), then resolves to the front buffer at `0x04445000`. The Y/U/V
planes (`k_8`, 1120x632 + 2x 560x316) are uploaded every frame, the fetch
constants and PS constants c251-c255 are sane (1.164 / 1.596 / 2.017 / 2.4),
alpha ref 0.0039, the viewport covers 1120x632, and no `IssueDraw failed` is
logged -- yet the resolved image is exactly the clear colour. The menu being
washed out white points at the same area: shader translation (log/exp/cnd*)
or render-target format / gamma handling, not game code.

`SD360_CAPTURE_DIR=<dir>` (optionally `SD360_CAPTURE_EVERY=<s>`) dumps the
guest's presented frame to PPM, so this can be checked without screenshots.


## SOLVED: black movies and black/white title-screen 3D (2026-10-20)

State after these fixes: boot -> Marvel / Activision / Beenox logos -> intro
movie -> title screen with the 3D Spider-Man scenes rendering. Remaining:
occasional black flickers and some briefly broken shaders (see "Open").

### Movies black: render target path

With the default host-render-target path every `.usm` movie draws black (audio
plays). `REX_RENDER_TARGET_PATH_VULKAN=fsi` (fragment shader interlock, EDRAM
emulated in the pixel shader) plays them. `run.sh` now sets it by default. The
fsi path trips debug asserts, so the RelWithDebInfo build is the default too.

### Title-screen 3D: two SDK bugs in texture fetch (patches/0001, 0002)

Measured with `logs/claude/fetchdump.py` (breakpoints on
`VulkanCommandProcessor::IssueDraw/IssueCopy`, dumping texture fetch constants
and resolve registers). The game's HDR chain:

```
draw 7e3 / k_16_16_16_16 render target
COPY  dest fmt 26 (k_16_16_16_16)  RB_COPY_DEST_INFO=0x000B4D01
      -> copy_dest_number = 2 (unsigned integer), copy_dest_exp_bias = +11
FETCH tf fmt 26  w3=0x0106AD11
      -> num_format = 1 (integer), exp_adjust = -11
```

i.e. it stores the scene as 5.11 fixed-point integers and reads it back with
`int * 2^-11`.

1. **0001** — the SPIR-V translator read the result exponent bias from fetch
   word 4 bits 13:18 (that is inside the LOD bias field). The field is word 3
   bits 13:18 (`xe_gpu_texture_fetch_t::exp_adjust`), which the DXBC
   translator already uses. Before: the -11 was never applied -> white.
2. **0002** — integer `num_format` on 16-bit fixed-point textures was not
   implemented (the SDK's own comment in `vulkan/texture_cache.h` says to add a
   multiplier "if any game is seen using num_format 1 for fixed-point
   formats" — this one does). The host view is UNORM, so a read returned
   `int/65535`, and the resolve shader packed `saturate(v*2^11)*65535`
   (saturating at v = 1/2048) instead of the integer `v*2^11`. After 0001
   alone: everything 65535x too dark -> black.
   Fix: resolve to an unsigned-integer 16-bit destination uses
   `exp_bias - 16` (`util/draw.cpp`), and the texture fetch multiplies by
   65535 (32767 for signed components) when `num_format` is integer and the
   format is k_16 / k_16_16 / k_16_16_16_16 (`spirv_translator_fetch.cpp`).
   Net error is 65535/65536.

Both patches live in `patches/` and are applied to `tools/rexglue-sdk`; only
the GPU plugin needs rebuilding:

```
cd tools/rexglue-sdk
git apply ../../patches/0001-*.patch ../../patches/0002-*.patch
cmake --preset linux-amd64 -DCMAKE_C_COMPILER=clang-20 -DCMAKE_CXX_COMPILER=clang++-20
cmake --build out/build/linux-amd64 --config RelWithDebInfo --target rexgpu-xenos
cp out/linux-amd64/RelWithDebInfo/librexgpu-xenosrd.so \
   ../../sd360/out/build/linux-amd64-relwithdebinfo/
```

(The original prebuilt plugin/runtime are kept in
`sd360/out/build/linux-amd64-relwithdebinfo/prebuilt-sdk/`.)

Not covered by 0002: signed-integer resolve destinations, and 8-bit / 32-bit
fixed formats with integer num_format. Not seen in this game so far.

### Open

- Occasional black flicker and briefly wrong shaders. First suspect:
  `async_shader_compilation` (default true) combined with
  `REX_VULKAN_ASYNC_SKIP_INCOMPLETE_FRAMES=false`, which presents frames drawn
  while pipelines are still compiling.

## 2026-11-02: level loading works, PC controls built in

### Missing functions that blocked level loading

Two more indirectly-called entry points, both found by the runtime's own
`[FATAL] Call to invalid or unregistered function`:

| Address | Binary | Fix |
| --- | --- | --- |
| `0x8832B660` | GameLogic.dll | `[modules.functions] 0x8832B660 = {}` |
| `0x82182648` | Default.xex | `[entrypoint.functions] 0x82182648 = {}` |

`0x8832B660` is a tail-call thunk to `0x881F0AA0`; `0x82182648` reads a pointer
and returns `-1` when it is null. Neither has a direct caller in the generated
code, which is why discovery missed them.

After both entries: GameLogic registers **21,053** functions (was 21,052).
Verified by driving the game with real key events: boot -> title -> save select
-> main menu -> Level Select -> first level reaches the in-level **AUTO SAVE**
prompt, with **no FATAL** in the log (`logs/claude/level/`, runtime log
`sd360_057.log`). Before these fixes the same path died on `0x8832B660` and
then `0x82182648`.

**Build gotcha:** `sd360_GameLogic` is not part of the default `all` target.
Changing a `[modules.functions]` entry regenerates the DLL sources but a plain
`cmake --build --preset linux-amd64-relwithdebinfo` leaves the old
`libsd360_GameLogic.so` in place (it was 12 days stale here). Build
`--target sd360_GameLogic` explicitly, or the runtime loads the stale module.

### PC controls are now built in

- New `sd360/src/pc_controls.h` holds the layout; `Sd360App::OnPostSetup`
  applies it with `rex::cvar::SetFlagByName`, but only for flags whose
  `GetFlagSource()` is still `Source::kDefault`. Config file, `REX_*`
  environment variables and `--flag` overrides all still win.
- Layout follows the 2010 PC port: WASD + mouse look, LMB fast attack, RMB web
  zip (tap) / web swing (hold), Space jump, Enter select, Esc pause. Bindings
  the PC documentation does not list are marked `[guess]` in the header.
- `controls.cfg` was deleted and `run.sh` no longer parses it. Verified by
  navigating the menus with only Enter and Down (no config file present).

### Dimension fade — still open (measured)

With `SD360_CAPTURE_MS=500` the menu fade shows repeated black windows where the
3D scene drops to a constant clear colour while the UI keeps drawing:
`71.8`, `74.3-74.8`, `87.5-88.5`, `91.0-91.5 s` in `logs/claude/final/`.
This is the same outage that made level select look black before selecting a
level, so the fade and the "levels don't load" symptom share a render-target
cause rather than a game-logic one.

A/B result: `REX_GPU_ALLOW_INVALID_FETCH_CONSTANTS=true` (the bypass the SDK's
own warning suggests) did **not** remove the black windows
(`logs/claude/fetchfix/`). The runs still emit thousands of *invalid texture
fetch constant* warnings, so the fetch path is not what is dropping the scene.

Still to try, in order: the fsi vs host render-target path per scene (host path
fixes nothing here but is cheap to compare), `direct_host_resolve`, and the
EDRAM resolve formats. Do not conclude it is shader compilation without a run
with `async_shader_compilation=false` to compare.

### Fade investigation, second pass (same day)

Ruled out, with measurements:

1. **Shader compilation.** In a run with a warm pipeline cache the log contains
   **zero** `Creating graphics pipeline state` lines, yet the black windows
   still occur (`sd360_056.log`). The earlier "async shaders" theory is dead.
2. **Render-target path choice.** `REX_RENDER_TARGET_PATH_VULKAN=` (host path)
is    far worse, not better: black stretches of 13-33 s during the same menu
   window (`logs/claude/hostmenu/`). `fsi` is the correct default; the fade
   problem is a specific resolve/fetch step, not the overall path.
3. **Invalid fetch constants.** `REX_GPU_ALLOW_INVALID_FETCH_CONSTANTS=true`
   changes nothing (`logs/claude/fetchfix/`).

Remaining suspect: integer `num_format` coverage. `patches/0002` handles only
**unsigned 16-bit** fixed-point resolve destinations and fetches; its own notes
say signed-integer destinations and 8-bit / 32-bit fixed formats are not
covered. A fade intermediate in one of those formats would come back black or
blown out — which also matches the in-level frame that jumped from luma 38 to
158 right before the run ended.

`REX_DIRECT_HOST_RESOLVE=false` was tested as well and is **worse**: 24 black
windows in the 60-190 s span, some 6.6 s long, against 4 short (0.5-1.5 s)
windows with the current defaults (`logs/claude/dhr/`). So the shipped
configuration (`fsi`, direct host resolve on) remains the best of the three;
the defect is a narrow resolve/fetch gap, not the path or resolve mode.

To pin the format down, the resolve already emits a `REXGPU_TRACE` line naming
the source and destination formats (`util/draw.cpp`, "Resolve: ... -> ..."), but
only at trace level. That is the next diagnostic: a short run at trace level,
grep `Resolve:` around a black window. Do not ship an SDK format patch before
that, and get approval first per AGENTS.md rule 6.

### "Crash on level load" — not reproduced on the current build

`coredumpctl` holds exactly one sd360 abort: **2026-11-02 01:56:22**, SIGABRT,
which is the process death that followed the `0x82182648` FATAL at 01:56:19 —
before that address was registered. Nothing since.

Two apparent "crashes" while testing were not crashes. `ReXApp::OnClosing`
(`rex_app.cpp:498`) logs `Window closing, shutting down...` and calls `_Exit(0)`;
that path is a window-close request, and one run was also killed by a
`pkill -x sd360` from an overlapping test script. Both look like a clean exit in
the log, with no core dump. Lesson for future runs: never let two driven runs
overlap.

What *is* verified on the current build: the level loads and renders geometry
(`logs/claude/crash1/final.png`), reaching the in-level auto-save prompt, with
no FATAL. If a crash still happens in normal use, capture the runtime log and
`coredumpctl list` from that exact run before changing anything.

## 2026-11-02 — Invalid texture fetch constants: black menu and two-pane garbage

Symptom: the main-menu background renders black, and the Amazing/Ultimate
dimension switch draws two horizontally repeated, magenta/green copies of the
scene in a band at the top of the window with black below; the same corruption
shows up in the level.

Diagnosis:

- The guest output captured through `SD360_CAPTURE_MS` is always clean, so the
  fault is in draws the emulator refuses to issue, not in the presenter (the
  1120x632 -> 1920x1080 blit renders the title correctly).
- `sd360_069.log` contains **748** warnings during a menu run:
  `Texture fetch constant (...) has "invalid" type!`
  Four distinct constants, all with `dword_0 & 3 == 0`
  (`FetchConstantType::kInvalidTexture`) while the rest of each constant is a
  well-formed texture (format 6 = `k_8_8_8_8`, non-zero base pages):

      804248FC 0AD2B086 07C0F81F 00A80C14 00000003 00000C00
      88C00000 067B805A 004EE45F 012EAD11 00000003 00000200
      810000FC 0AD1A054 000FE07F 00800D10 00000140 0AD1EA00
      00000000 00000000 00000000 00A80000 00000003 00000000

- `TextureCache::BindingInfoFromFetchConstant`
  (`src/graphics/pipeline/texture/cache.cpp:904`) returns early for
  `kInvalidTexture` unless `gpu_allow_invalid_fetch_constants` is set, so every
  draw using those slots was dropped.

Change (launcher-level workaround): `run.sh` now passes
`--gpu_allow_invalid_fetch_constants=true`. Truly empty constants are still
skipped by the cache (`base_page == 0 && mip_page == 0` -> return), so this does
not make the emulator invent textures for unused slots.

Verified: the same menu run with the flag reports **0** invalid-fetch warnings
(`sd360_071.log`) and the title background cycles Amazing / Ultimate / 2099
behind the AUTOSAVE dialog. Captures: `logs/claude/art/grid_art2.jpg` (fixed)
against `logs/claude/art/grid_art.jpg` (black background), and
`logs/claude/bypass/grid_bypass.jpg`.

Open question: whether the game's constants really carry type 0, or the type
bits are lost somewhere in the constant upload path. If the latter, this flag
masks an emulator bug instead of accommodating the title.

## 2026-11-02 (later) — Correction: the two-pane / squashed-level artifact is in the guest output

The invalid-fetch-constant flag from the previous section is still correct for what it
does (748 warnings -> 0, menu background restored) and should stay. It does **not** fix
the two-pane artifact: the user reported it still on the main menu and in a level, and a
reproduction confirms it. The earlier attribution of the two-pane garbage to those
constants was wrong.

Measured 2026-11-02 14:38-14:53, RelWithDebInfo, `./run.sh` unchanged:

| Run | Render target path | Guest output | Window |
| --- | --- | --- | --- |
| `logs/claude/rep3` (fsi, default) | fsi | level 3D squeezed into ~20 of 632 rows at the top; UI text, spider logo normal; persists 25 s+ | same |
| `logs/claude/hostlvl` (`REX_RENDER_TARGET_PATH_VULKAN=`) | host | same band | same |
| `logs/claude/winshot`, `art` | fsi | title-screen 3D full frame, correct | same |

Notes:

- The presenter is **not** implicated. `SD360_CAPTURE_DIR` dumps the presenter's own
  guest-output image, and that image already contains the band
  (`logs/claude/rep3/lvl_guest.jpg`, `band_full.jpg`). The 1120x632 -> 1920x1080 blit
  renders the title screen correctly.
- Movies (YUV full-screen quad) and all 2D UI render full-frame at the correct positions;
  only the 3D scene pass is wrong, so it is not a global front-buffer problem.
- Both render-target paths show it, so it is not an EDRAM/fsi resolve difference.
- Invalid-fetch warnings are 0 in these runs (`sd360_072/074/076.log`), so the flag is
  active and unrelated.
- Band height is ~20 of 632 rows (~1/32 of the frame), which points at a
  viewport/scissor/resolve *rect* rather than a colour or format problem.

Suspects, not yet tested:

- `VdGetCurrentDisplayInformation` (`src/kernel/xboxkrnl/xboxkrnl_video.cpp:205`) reports
  `display_window_overscan_*` = width/4 and height/4 per side, i.e. a 50%x50% display
  window. A title that lays out its scene viewport from the display window / scaler
  parameters would render into a much smaller rect than the front buffer.
- Per-draw viewport and scissor values in the level. `logs/claude/vpdump.py` was written
  for this but gdb produced no output on the first attempt (it did not attach).
- Whether the band scales with the window/video mode: run with
  `./run.sh --window_width 1280 --window_height 720` and compare band height.

Follow-up, same day: `./run.sh --window_width 1280 --window_height 720` (which also
changes the guest video mode to 1280x720, `xboxkrnl_video.cpp:42`) does **not** help —
`logs/claude/w720/tail.jpg` shows the same band, and the guest frames are 1120x632 with
only the top ~20 rows carrying scene content. So the video mode is not the variable
either. (A first pass counted every row as "content" because a >3/255 luma threshold
catches dithering noise in the black area; the montage is the reliable check.)

Two more facts from this pass:

- The front buffer described earlier is 1120x640 (`0x04445000`, 2867200 bytes) while the
  guest output is 1120x632, so the swap refresh already goes through a scaled/cropped
  copy path.
- Both fsi and host render-target paths, both video modes, and the fetch-constant flag on
  or off all produce the band, so the fault is upstream of all of those: the scene pass'
  viewport / scissor / resolve rect, or the front-buffer-to-guest-output refresh.

Next step (needs an SDK instrumentation patch, not a fix): log, for the first N draws of
one level frame, `viewport_info.xy_offset/xy_extent`, the scissor, the active render
target size and the resolve rect. That names the wrong value directly. `gdb` with
`logs/claude/vpdump.py` produced no output on the first attempt, so the log has to come
from inside the plugin (built the same way as patches/0001-0002).

## 2026-11-02 — Correction 2: the "band" is the LEVEL LOADING SCREEN, and the load stalls

The user supplied a screenshot of the artifact alongside the game's spider logo and a red
progress bar. That identifies the state: it is the **level loading screen**, not gameplay.

Measured with a 1 s guest-frame capture while driving title -> save select -> main menu ->
level select -> first level (`logs/claude/lvl2/`):

- From t ~= 128 s the red progress bar is **27 px wide and does not change for the next
  85 s**. The frame alternates between the loading screen (~13-21 bright rows of 158) and
  occasional full frames (158) - that alternation is the "flicker" being reported.
- So the level **never finishes loading**. This is the same failure the user described
  earlier as "levels had loading problems", not a new rendering regression.
- An earlier count that read the band as "the 3D scene squeezed into 20 rows" was
  measuring this loading screen; the frames at 92-95 s with very few bright rows are the
  Xbox LIVE message boxes dimming the screen (`logs/claude/loadbar/loadseq.jpg`).

Process state during the stall (`logs/claude/stall/sample.txt`):

- No error, no FATAL, no missing-function trap in `sd360_082.log`.
- Open files are only `run/Data/Streams.dat`, the runtime log and the shader caches - no
  file is being read, so it is not an I/O call left in flight.
- Guest threads are almost all in `futex_do_wait` (Main XThread too); one XThread is
  runnable. That is an in-guest wait (critical section / event / thread join), not a host
  file read.

Therefore the next question is *which guest wait* the loader is parked on - sample the
main XThread's guest PC during the stall (gdb) rather than looking at rendering at all.

Superseded conclusions (do not act on them):

- "3D scene squeezed into ~20 rows / wrong viewport" - the frame is the loading screen.
- The two render-target paths, the video mode and the fetch-constant flag are all
  irrelevant to this symptom (tested, all identical).
- The diagnostic draw-dump patch (`patches/0003-draw-state-dump.patch`) is still useful
  for other issues but will not explain the stall.

Note on the first gdb attempt (`logs/claude/stall2/bt.txt`): it is **not** a sample of the
stall. gdb stopped on a SIGSEGV in `sub_82202720` (`REX_STORE_U32(ea, ctx.r11.u32)`) because
the script did not pass SIGSEGV through, and the runtime handles guest page faults with
that signal. The backtrace therefore shows a normal guest store, not the loader's wait.
Any repeat must use `handle SIGSEGV nostop noprint pass` (as `logs/claude/sample_vw.sh`
does) before `run`.

## 2026-11-02 (later) — Correction 3: the strip is in-level 3D, and what is now ruled out

Correction 2 was wrong in its conclusion: the spider logo with the red bar is the **health
HUD**, not a loading bar. Live window captures during the level show the tutorial hint
("Push [stick] towards a surface to stick to it", "Use [stick] to move around"), so the game
*is* in gameplay while the artifact is on screen.

Observed shape of the artifact (logs/claude/still/live_still.png, dims2/grid.jpg):

- The 3D scene is squeezed into roughly 20 of 632 rows at the top, full width; everything
  below is black. 2D/HUD/text draw correctly at their normal size and position.
- It comes and goes: in one run eight frames sampled 8 s apart gave one full-screen magenta
  (uninitialised-texture looking) frame, one *perfectly correct full-screen 3D frame*, and
  six strip frames.
- Some frames show the HUD spider twice, at the left and right edges - matching the user's
  "2 screens at the top left and right".
- It happens both standing still and moving, on the title screen and in levels, and only for
  the Amazing and Ultimate dimensions.

Eliminated by measurement on this build (do not re-test these):

1. **Guest viewport.** `patches/0003-draw-state-dump.patch` logs every distinct per-draw
   state. Across a 195-state session every 3D draw has
   `vport_scale=(560,-316) vport_offset=(560,316)` - a full 1120x632 frame. No draw ever
   asks for a short viewport, so the strip is not the guest's viewport.
2. **Partial resolve extent.** `patches/0004-partial-resolve-dump.patch` logs
   `GetRenderTargetHeight` vs `DrawExtentEstimator::EstimateMaxY` and the resulting
   `height_used`. The values cycle 193 / 632 / 1176 / 2352 within every frame, and the same
   set appears in every 2-second window sampled, whether the screen showed the strip or not -
   so `height_used` does not separate good frames from bad ones.
3. Previously ruled out: presenter/blit (the guest's own capture is already banded), video
   mode 1280x720 vs 1920x1080, the host render-target path vs `fsi`, and the invalid
   fetch-constant flag (that one still fixes the black menu background and stays on).

What is left: the 3D geometry itself for those two dimensions - the vertex shader or the
vertex-shader constants it reads - given that the same build renders a correct full-screen
scene in another frame. The magenta frame suggests a texture or render target being sampled
before it holds valid content, which may be the same root cause.

Next measurement (not yet done): for strip frames, log the vertex shader hash together with
the vs constant registers (0x4000 range) and the render target base, and compare against a
2099/Noir frame that renders correctly.

### Attempt: render-target base comparison (2026-11-02, later still)

`patches/0003-draw-state-dump.patch` now also logs the EDRAM tile base of the colour and
depth render targets (`RB_COLOR_INFO.color_base | bit_11 << 11`, same for `RB_DEPTH_INFO`),
alongside viewport, scissor, pitch, MSAA, format and the vertex/pixel shader hashes.

Observed during boot and menu (202 sx360_094.log, 202 distinct states):

- Every 1120-pitch scene/HUD draw uses `cbase=0 dbase=1120`.
- The 1024x1024 passes use `cbase=0 dbase=0`.
- No state in the sampled window used a colour base other than 0.

**This did not test the hypothesis**, because the ten screenshots taken during the window
(logs/claude/bases/grid.jpg, every 6 s) all show the 3D background rendering *full-frame and
correct* behind the autosave panel - the strip was not on screen at any point in that window.
The run therefore cannot say whether the strip correlates with a different base, and the
"no difference between dimensions" reading would be wrong to take from it.

What is needed instead: frame-correlated sampling - log the current state (or a state index)
once per presented frame from `IssueSwap`, so each screenshot has an exact state line next to
it. Then a strip frame and a good frame from the *same* dimension can be compared directly.

Frame-correlated sampler (added 2026-11-02, late): `IssueSwap` now logs
`SD360FRAME t=<unix> n=<frame> fb=<ptr> <w>x<h>` once per presented frame, so any screenshot
can be tied to the exact frame range it belongs to.

Result of three sampled windows (logs/claude/frames, logs/claude/lvl3):

- Every presented frame in all three windows is `fb=71585792 1120x632` at ~45 fps - one
  front buffer, one size, no variation, including while the game is running.
- Colour base is 0 and depth base 1120 tiles for every 1120-pitch scene draw; the 1024x1024
  passes use depth base 0. No colour base other than 0 was observed.
- **None of the three windows contained the strip.** Title + autosave panel, idle title, and
  the save-select screen all rendered the 3D background full-frame and correct.

So the sampler works but has not yet caught a bad frame. The next run must be driven to the
moment the artifact is actually on screen (the reliable repro so far is in-level, standing
still: logs/claude/still/live_still.png), and only then compared - otherwise every conclusion
drawn from the window is about a working screen.

### Captured artifact frame at full resolution (2026-11-02, latest)

A 190 s continuous recording of the window (logs/claude/rec.mkv, 2 fps, 380 frames) with an
automatic "mostly black" detector found the artifact on screen at ~17 s in. Extracted frame:
**logs/claude/art34.png** (1920x1080, title screen).

What it shows, measured on that frame:

- The SPIDER-MAN logo and the red PRESS bar are at their normal size and position, sharp.
- The 3D background is absent except for a band roughly 20 px tall at the very top edge,
  which contains **three bright fragments at almost exactly 1/4, 1/2 and 3/4 of the width**.
- Row luminance (192x108 proxy): content only in rows 6-7, then the logo at 18-21 and the
  PRESS bar at 88-91; everything else is black.

Three evenly spaced copies in a thin band is the signature of a resolve/copy whose geometry
(pitch, height or tile stride) is wrong - content tiled at a fixed stride - rather than a
squashed viewport or a missing draw. It also explains "2 screens at the top left and right"
seen on other frames, which is the same repetition with a different count.

Everything at draw-issue level is already excluded for this build: viewport (always full
frame), scissor, render-target colour base (always 0) / depth base, MSAA, format, shader
hash, resolve height estimate, swap size (always 1120x632 at ~45 fps) and the presenter.
So the next instrument must sit in the **resolve/copy path** (render_target_cache Resolve),
logging source EDRAM base, pitch in tiles, MSAA, resolve height and destination address for
the frames that show the band - not in IssueDraw.

Resolve-path logger: `patches/0005-resolve-dump.patch` logs (on change) the copy geometry
straight out of `draw_util::ResolveInfo` at the top of `VulkanRenderTargetCache::Resolve` -
depth/colour, EDRAM base, row length, rows, pitch in tiles, width/height in 8-pixel units,
destination base/start/length and the destination info register. It builds and runs
(26,378 change entries in one 100 s session).

However the 100 s recording taken with it (`logs/claude/rec2.mkv`, started at a known wall
clock) contained **zero** artifact frames by the strict detector (bright band in the top ~4%
with a black gap beneath), so it produced no correlation for the bug. The artifact really is
intermittent: the earlier 190 s recording caught it at ~17 s, this one did not catch it at all.

To finish this, the run has to be repeated until the detector fires - the instrumentation is
now all in place (draw state, resolve geometry, per-frame swap, wall-clock timestamps) and the
detector turns a 100-190 s recording into a list of exact artifact frames.

Third attempt to catch it with the resolve logger: relaunched and recorded from the first
second for 200 s (logs/claude/rec3.mkv, 400 frames). The strict detector found **zero**
artifact frames - so the bug was not on screen at all during that run, on any frame.

State of play, honestly: the artifact is real, reproducible in some runs (logs/claude/art34.png)
and absent in others. All draw-issue and swap-level properties measured so far are identical
between its presence and absence, and there is not yet a caught frame *with* the resolve
logger running, so the resolve geometry is still unmeasured at the moment it happens.

The cheapest way to finish: record a long unattended session (10-15 min) in the background
while the game is played normally, then run the detector over it. The instrumentation needed
for the correlation (SD360RES / SD360STATE / SD360FRAME, all with wall-clock timestamps) is
already built into the plugin and enabled with --sd360_draw_dump=true.

### Viewport census across every logged session (decisive negative)

Every distinct draw state ever logged - 3,017 states across eight sessions - was grouped by
its viewport scale:

    2452  560,-316     full frame (1120x632)
     292  512,-512     the 1024x1024 passes (depth pre-pass / shadow-style targets)
     130  140,-79      quarter-size target (280x158)
      79  70,-39.5     eighth-size target (140x79)
      50  280,-158     half-size target (560x316)
      14  0,0          degenerate/no draw

There is **no draw anywhere that asks for a squashed viewport**. A 1/32 vertical squash
would need a scale near -9.9 and never appears. Since the artifact also keeps the HUD at
full size inside the same guest frame, the squash is not a viewport, scissor, resolve or
presenter property - it must come from the vertex/projection data of the scene pass.

Working hypothesis for the fix: the 3D geometry is transformed with a projection whose Y
scale is ~1/32 (632 / 20 rows = 31.6), i.e. a wrong *constant* in that pass - most likely a
wrongly placed vertex-shader float constant (a one-register shift picks up a 1/32 scale from
a neighbouring slot). Confirming it needs one artifact frame captured while the instrumented
build is logging the constants of the scene draws.

Detector lesson: the first "hit" was a LOADING screen (bright wordmark, black below), not the
artifact. Any rule of the form "nothing below the band" also rejects the real artifact, which
keeps its logo and PRESS bar. The collector therefore saves *candidates* for review instead
of deciding.

Also observed: cutscenes can stall - one frame (a green-lit building) stayed pixel-identical
with a constant 250 draws for over four minutes while swaps kept happening. Separate issue
from the strip, recorded here because it blocks driving the game by script.

### 2026-11-03: what the screenshots actually show, and one dead end

The user's frames settle the *shape* of the bug: within one attract/in-level sequence the
same frame is sometimes fully rendered, sometimes black, sometimes a thin band - always with
a correct HUD. That alternation rules out a projection/constant error (a wrong matrix would
be consistently wrong) and points at the copy that brings the 3D pass into the presented
buffer: it sometimes runs, sometimes not, sometimes only partly.

Dead end measured today: `direct_host_resolve` (default true) looked like a match, because
`TryResolveCopyDirectly` can complete for a region it did not fully write. It is not
reachable in this build: run.sh forces REX_RENDER_TARGET_PATH_VULKAN=fsi, so
GetPath() != kHostRenderTargets and the direct path is skipped entirely. Confirmed by
measurement - 29.8 fps default vs 29.9 with --direct_host_resolve=false, i.e. no work
changed. Keep that flag out of the launcher; it buys nothing here.

Consequence: on the FSI path the resolve is a compute dispatch reading edram_buffer_, whose
contents are written by fragment-shader interlock, and run.sh already notes this game's
EDRAM aliasing breaks the host path outright. The next instrument to add is EDRAM range
coverage per resolve (base, row length, rows) cross-referenced with which render target
actually received the draws - if two targets alias the same EDRAM rows and alternate per
frame, the black/partial/good cycle follows directly.

Also: the title attract loop did not reproduce the fault in ~4 minutes of sampling, so the
only reliable reproducer is real gameplay.

### 2026-11-03 (2): the copy is never skipped - measured over a real play session

Added undeduplicated front-buffer resolve logging (SD360FB, one line per copy into
0x04445000) plus the per-frame draw counter. A play session that loaded into a level and
reached 1,602 draws per frame produced:

    frames=6440  front-buffer copies=6440  avg=1.000
    dlen histogram: 2867200 x 6439   (1120x640x4, the full frame)
    frames with ZERO copies: 1  <- the final frame, i.e. the game being closed

So the scene is never skipped and never truncated, and there are no clear-only resolves at
all (0 of 73,058). The front buffer is also a single fixed address across 57,099 swaps, so
there is no double buffering to present the wrong half of. The HUD lives in that same buffer
and always survives, so the presented image is faithful: the 3D region of the framebuffer is
genuinely empty on the bad frames. The remaining suspect is therefore how much of EDRAM a
render target is considered to own, not the copy.

Patch 0004 output points at exactly that. height_used = min(rt_height, EstimateMaxY) flips
constantly for the main 1120-wide target:

    rt=2352 est=8192 used=2352   (whole EDRAM height for 14 tiles)
    rt=2352 est=632  used=632    (the real frame height)

est=8192 is EstimateMaxY's cap, reached when the estimator cannot bound the draws.
EstimateMaxY is called with `interlock_barrier_only ? execute_unclipped_draw_vs_on_cpu_... :
true`, and that cvar already defaults to true, so passing it explicitly changes nothing
(verified: used=2352 still dominates). Worth noting for the next step: on the interlock path
the target is routinely treated as 2,352 rows tall when the frame is 632, which is precisely
how a scene can be written outside the range the resolve reads back.

Also re-confirmed: the shm file (/dev/shm/xenia_memory_*, 0x120001000 bytes) maps guest
addresses identity - the XEX image sits at 0x82000000 with an MZ header - but the front
buffer address 0x04445000 reads as zeros both directly and at the +0x80000000 mirror, so it
cannot be used to inspect the framebuffer from outside the process.
