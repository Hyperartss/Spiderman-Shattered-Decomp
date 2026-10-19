# Codegen status

## Run summary

| Field | Value |
| --- | --- |
| Date | 2026-10-18 20:47:44 UTC |
| SDK | ReXGlue 0.10.0 |
| Manifest | `sd360/sd360_manifest.toml` |
| Entrypoint | `assets/Default.xex` |
| Duration | 2.7 s |
| Log | `logs/codegen-20261018-204744.log` |

## XEX metadata (read by the tool, not guessed)

| Field | Value |
| --- | --- |
| Title ID | `41560881` |
| Media ID | `06AAF6AB` |
| Version | `0.0.0.1` |
| Filetime | 2010-07-14 18:43:04 UTC |
| Size | 3,743,744 bytes |
| MD5 | `183953ad5ba194429ef2c77b4c326960` |
| Kernel module base | `0x82222E98` |

## Result: validation errors by category

| Category | Count |
| --- | --- |
| Unresolved calls | 0 |
| Bad function boundaries | 0 |
| Jump tables | 0 |
| Missing kernel imports | 0 |
| **Total** | **0** |

Codegen exited 0 and wrote 95 files. No `--force` was used — this is a clean
pass, not forced output.

Scanned for stub markers in the output (`REX_UNRESOLVED`, `REX_THUNK`,
`REX_STUB`, `REX_UNKNOWN`, `REX_TODO`, `REX_INVALID`): none found. The only
match was the constant `REX_THUNK_RESERVE_SIZE`, which is a definition, not a
pending item.

## Generated output

| Field | Value |
| --- | --- |
| Functions discovered | 9,564 |
| `DEFINE_REX_FUNC` count | 9,564 (matches discovery exactly) |
| Recomp `.cpp` files | 44 |
| Header partitions | 16 |
| Total size | 46 MB |
| Address range | `0x82080000` – `0x82300280` |

Discovery and codegen agree exactly at 9,564, which is a good consistency
signal: no function was discovered and then dropped.

## Honest status

- **Compiles** — not yet tested
- **Codegen passes** — YES, verified above
- **Boots** — not yet tested
- **Playable** — not yet tested

Nothing has been built or run. The generated C++ has never seen a compiler.

## Blocked on

`clang 20+` and `ninja` are still not installed, so `sd360_codegen` via CMake
and the native build cannot run yet. Run `./setup-toolchain.sh` first.

Codegen itself was unaffected: the `rexglue` binary has no clang/llvm/ninja
runtime dependency (verified with `ldd`), so this pass was legitimate.

## Notes for the next iteration

1. `rexglue init` wrote `../assets/default.xex`, lowercasing the real filename
   `Default.xex`. On a case-sensitive filesystem this does not resolve. It was
   corrected to `../assets/Default.xex` in `sd360_manifest.toml`. Re-check this
   if the manifest is ever regenerated.
2. The README/AGENTS.md document `rexglue init --app_name X --app_root Y`.
   SDK 0.10.0 actually takes `--project-name` and `--xex-path`. The docs are
   stale; trust `rexglue init --help`.
3. Codegen was clean on first run, which is unusual for this tool. If the build
   fails to link, suspect missing kernel imports that codegen deferred rather
   than expecting a wall of codegen errors.