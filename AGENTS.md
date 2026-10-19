# AGENTS.md — Rules for AI agents working on this repo

## Goal

Get the Xbox 360 build of *Spider-Man: Shattered Dimensions* statically recompiled with the ReXGlue SDK into a native, running program. Work in small verified steps.

ReXGlue is a **static recompiler** (PowerPC → C++ ahead of time), not a decompiler. It is experimental and not push-button: expect to hand-specify function boundaries, supply missing kernel imports, and iterate on generated code. Do not promise or imply otherwise.

## Hard rules

1. **Game files are read-only.** Never modify, move, rename or delete the ISO, the extracted game folder, or `default.xex`. Work on copies or write results elsewhere.
2. **Never commit game material.** No ISO, XEX, extracted assets, textures, audio, or generated codegen output (it is derived from the game binary). Check `git status` before every commit. If something copyrighted is staged, unstage it and tell the user.
3. **One step at a time.** Do one thing, check the result, report, then continue. No large speculative rewrites.
4. **Log, don't dump.** Send build output, codegen output and runtime output to files in `logs/` and read only the relevant parts (`grep`, `tail`, targeted ranges). Do not paste or read whole multi-thousand-line outputs.
5. **Read the docs before touching the config.** Before editing the TOML, read the wiki page for it: https://github.com/rexglue/rexglue-sdk/wiki/rexglue-CLI-Configuration-File . Do not invent TOML keys, CLI flags or API names. If you can't find it in the docs or the SDK source, say so and ask.
6. **Don't edit the SDK unless told to.** The SDK lives in `tools/rexglue-sdk` (or wherever the user installed it). If a fix needs an SDK change, write it as a patch in `patches/`, explain why, and wait for approval.
7. **Don't touch system packages or global config** without asking (apt installs, PATH edits, git config).
8. **Be honest about status.** Distinguish "compiles", "codegen passes", "boots", and "playable". Never claim something works without having run it.

## Environment

- OS: Linux (Mint). Windows notes are in the README.
- Toolchain: CMake 3.25+, Ninja, **Clang 20+**, GTK3 dev headers, Vulkan-capable GPU.
- Run `./check-env.sh` first. Do not proceed to codegen until it passes, or the user knowingly skips a check.
- Paths (the user fills these in; ask if missing):
  - `GAME_DIR` — extracted ISO contents (read-only)
  - `XEX_PATH` — `$GAME_DIR/default.xex`
  - `SDK_DIR` — ReXGlue SDK install
  - `ASSETS_DIR` — `./assets` (git-ignored)

## Workflow

### Phase 0 — Environment
- Run `./check-env.sh`; report what's missing and how to install it. Install nothing without approval.
- Confirm the SDK is installed and `rexglue --help` works.

### Phase 1 — Baseline
- The user confirms the ISO boots in Xenia Canary. Ask for its log if codegen or runtime behaviour looks wrong, to compare against.

### Phase 2 — Scaffold
- `rexglue init --app_name sd360 --app_root ./sd360`
- Set `file_path` in `sd360/sd360_config.toml` to `XEX_PATH` (after reading the config reference).
- Confirm that `generated/rexglue.cmake` is the only thing under `generated/` that is tracked in git.

### Phase 3 — Codegen
- Run the `sd360_codegen` CMake target; redirect output to `logs/codegen-<timestamp>.log`.
- Summarise the validation errors by category (unresolved calls, bad function boundaries, jump tables, missing kernel imports). Save a summary to `docs/codegen-status.md`.
- Use `--force` only to get output to work through incrementally, and say when you did.

### Phase 4 — Iterate
Loop, one category at a time:
1. Pick the smallest fixable item from the error summary.
2. Read the relevant wiki page (Function Overrides, Mid-ASM Hooks, Kernel State & Objects, Codegen Pipeline Overview).
3. Make one change (TOML boundary/jump-table entry, kernel import stub, override, or hook).
4. Re-run codegen and/or the build; log to `logs/`.
5. Record the change and result in `docs/`. If it made things worse, revert it.

Prefer fixing function boundaries and jump tables in the TOML over writing overrides. Prefer overrides over mid-ASM hooks. Stubs for kernel imports should log when called so they can be implemented properly later.

### Phase 5 — Build and run
- Build with the debug preset first. Launch with `ASSETS_DIR` as the first argument.
- Capture the full runtime log to `logs/`. On a crash, report the failing function/address and the nearest log lines, not the whole log.
- Known failure modes in other ports: file IO/async handling crashes after the main menu (reblue), stale SDK `.so`/DLL mismatches causing undefined-symbol errors (project-eleven). Check those first.

## Understanding the code

- Use Ghidra with an Xbox 360 XEX loader for naming and analysis. Keep a function map in `docs/functions.md` (address → name → notes → confidence).
- If the user has the PC build of the game, it can help identify what a 360 function does, but never copy PC code into the recompilation; treat it as a reference only.

## Commit style

- Small commits, imperative messages (`Add jump table for sub_82301234`).
- Only configs, hooks, patches, scripts and docs. Run `git status` and review the diff first.

## When stuck

Stop and ask. State what you tried, what the log says (relevant lines only), and the two or three options you see. Don't guess at TOML keys, flags or SDK APIs.

## Reference

- https://github.com/rexglue/rexglue-sdk/wiki
- Working example ports to read first: The Outfit (`DohmBoy64Bit/TheOutFit`), project-eleven (`ThePlayer14/project-eleven`), reblue (`rexglue/reblue`)
