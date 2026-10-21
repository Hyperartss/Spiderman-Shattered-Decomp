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
