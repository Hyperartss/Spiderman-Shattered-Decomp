# Spider-Man: Shattered Dimensions (Xbox 360) — ReXGlue Recompilation

Experimental static recompilation of the **Xbox 360** build of *Spider-Man: Shattered Dimensions* using the [ReXGlue SDK](https://github.com/rexglue/rexglue-sdk), for educational and research purposes.

> **What this is:** ReXGlue statically recompiles the game's PowerPC code into C++ ahead of time, which is then compiled natively. It is **not** a decompiler. The generated C++ has no original names or structure. Readable source comes from a separate effort (Ghidra).
>
> **What this is not:** a turnkey solution. ReXGlue is experimental and pre-1.0. Expect to manually specify function boundaries, provide missing kernel imports, and iterate on codegen output. If you just want to play the game, use [Xenia](https://xenia.jp).

## No game files in this repo

This repo contains **only** configs, hooks, patches, scripts and docs. It never contains the ISO, `default.xex`, extracted game data, or the generated C++ (which is derived from the game binary). You must supply your own legally obtained copy. See `.gitignore`.

## Prerequisites

| Need | Notes |
| --- | --- |
| Your own game dump | Xbox 360 ISO of Shattered Dimensions, extracted to a folder (e.g. with `extract-xiso`) |
| CMake 3.25+ | |
| Ninja | |
| Clang 20+ | The SDK enforces Clang. On Linux Mint/Ubuntu the default repos likely ship an older version; use apt.llvm.org |
| GTK3 dev headers | `libgtk-3-dev` on Debian/Ubuntu/Mint |
| Vulkan-capable GPU + drivers | The runtime reuses Xenia's graphics backend as a temporary renderer |
| ReXGlue SDK | Latest release/nightly from [GitHub Releases](https://github.com/rexglue/rexglue-sdk/releases), or build from source |
| Xenia Canary | Baseline: confirm the ISO boots there before blaming your own work |
| Ghidra + an Xbox 360 XEX loader | For naming functions and understanding the code. Verify the loader works with your Ghidra version |

Windows needs Visual Studio 2022 with the Desktop C++ workload, including the Clang compiler (20.x+) and MSBuild support for LLVM.

Run `./check-env.sh` to see what's missing.

## Setup

```bash
# 1. Install the SDK (or download a release and add its bin/ to PATH)
git clone --recursive https://github.com/rexglue/rexglue-sdk.git tools/rexglue-sdk
cd tools/rexglue-sdk
cmake --preset linux-amd64
cmake --build out/build/linux-amd64 --target install
cd ../..

# 2. Extract your ISO somewhere outside this repo (or into the ignored assets/ folder)
#    and locate default.xex

# 3. Scaffold the project
rexglue init --app_name sd360 --app_root ./sd360

# 4. Edit sd360/sd360_config.toml so file_path points at your default.xex
#    (see the rexglue CLI Configuration File wiki page for the full schema)

# 5. Run codegen, then build
cmake --build --preset linux-amd64-debug --target sd360_codegen
cmake --build --preset linux-amd64-debug
```

Unresolved functions at codegen are expected. The pipeline reports them as validation errors; `--force` generates output anyway so you can work through them incrementally.

## Workflow

1. **Baseline** — boot the ISO in Xenia Canary, keep its logs.
2. **Codegen** — run it, collect the validation errors into `logs/`.
3. **Iterate** — fix errors in the TOML (function boundaries, jump tables), stub missing kernel imports, add function overrides and mid-ASM hooks.
4. **Build and run** — launch with the assets folder as the first argument; compare against the Xenia baseline.
5. **Name things** — use Ghidra to understand what you're patching; keep notes in `docs/`.

## Repo layout

```
README.md          this file
AGENTS.md          rules and workflow for AI coding agents
check-env.sh       checks the toolchain
sd360/             ReXGlue project (created by `rexglue init`)
  sd360_config.toml
  src/             app entry point, overrides, hooks
  generated/       codegen output (git-ignored, except rexglue.cmake)
docs/              notes: function maps, kernel import status, findings
patches/           SDK patches, if any are ever needed
logs/              git-ignored
assets/            git-ignored; your extracted game files
```

## References

- [ReXGlue SDK wiki](https://github.com/rexglue/rexglue-sdk/wiki)
- [Getting Started](https://github.com/rexglue/rexglue-sdk/wiki/Getting-Started)
- [Codegen Pipeline Overview](https://github.com/rexglue/rexglue-sdk/wiki/Codegen-Pipeline-Overview)
- [rexglue CLI Configuration File](https://github.com/rexglue/rexglue-sdk/wiki/rexglue-CLI-Configuration-File)
- [Function Overrides](https://github.com/rexglue/rexglue-sdk/wiki/Function-Overrides) and [Mid-ASM Hooks](https://github.com/rexglue/rexglue-sdk/wiki/Mid-ASM-Hooks)
- [Kernel State & Objects](https://github.com/rexglue/rexglue-sdk/wiki/Kernel-State-%26-Objects)
- Example ports: [The Outfit](https://github.com/DohmBoy64Bit/TheOutFit), [project-eleven](https://github.com/ThePlayer14/project-eleven), [reblue](https://github.com/rexglue/reblue)

## Disclaimer

Not affiliated with or endorsed by Activision, Beenox, Marvel, or Microsoft. ReXGlue is not affiliated with or endorsed by Microsoft or Xbox.
