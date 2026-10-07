# DESIGN — Project Setup (Step 0)

Goal of this step: a working Rust + Bevy project skeleton that is ready to
run, with the Game A path configurable, and the repo clean under the
whitelist `.gitignore`. No game data is copied in. No commits.

## What will be true after this step

1. Rust toolchain present via rustup (stable), plus `rust-src` is not needed
   yet; Bevy will build on stable.
2. Bevy's Linux system dependencies installed (X11, ALSA, udev, xkbcommon,
   wayland, OpenSSL dev headers) so `cargo run` can compile Bevy on Linux Mint.
3. Repo root is a Cargo project (`Cargo.toml`, `src/main.rs`) using Bevy.
4. `.gitignore` is a whitelist: it ignores everything and un-ignores only
   source/config/docs we author. `extracted/` and `ghidra/` exist as folders
   and are explicitly ignored.
5. `config.toml` at repo root holds the Game A install path (not hardcoded
   in source). `src/main.rs` parses it and prints the path at startup as the
   smoke test.
6. Nothing is committed.

## Exact commands (to be run on your OK, in order)

All run from the project root:
`/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spiderman Shattered Decomp`

### 1) Rust via rustup
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version && cargo --version
```
Expected: `rustc 1.xx.x (stable)` and a matching `cargo` version.

### 2) Bevy Linux dependencies (Linux Mint is Ubuntu-based)
```sh
sudo apt update
sudo apt install -y g++ pkg-config libx11-dev libasound2-dev libudev-dev \
  libxkbcommon-dev libwayland-dev libxkbcommon-x11-0 libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libssl-dev libxcursor-dev libxi-dev libxrandr-dev
```
Expected: apt finishes with "0 upgraded, ... newly installed" or "already the
newest version". If Wayland dev packages aren't found by name, that's fine —
X11 path is what Bevy uses; we'll drop that line.

### 3) Cargo project
```sh
cargo init --name spider-shattered
```
Expected: creates `Cargo.toml` and `src/main.rs` (won't overwrite existing
files since root is our project dir).

Add to `Cargo.toml`:
```toml
[dependencies]
bevy = "0.15"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```
(Version pinned to a real release at build time; if crates.io resolution
fails, we downgrade `bevy` one minor and retry, and log it in MODLOG.md.)

### 4) Whitelist `.gitignore`
Replace the current `.gitignore/` directory with a file:
```sh
rmdir .gitignore
```
Then `.gitignore` contains:
```gitignore
# ignore everything
*

# source we author
!.gitignore
!.gitattributes
!README.md
!agent.md
!MODLOG.md
!STATUS.md
!docs/
!docs/**
!Cargo.toml
!Cargo.lock
!src/
!src/**
!config.toml
!config.example.toml

# explicitly never track these two folders
/extracted/
/ghidra/
```
Also run:
```sh
mkdir -p extracted ghidra
```
`.gitattributes` stays as-is. Expected: `git status` shows only the source
files above as untracked; `extracted/` and `ghidra/` never appear.

### 5) Config for the Game A path
`config.toml`:
```toml
game_a_path = "/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spider-Man Shattered Dimensions"
```
And `config.example.toml` with a placeholder path. No code reads Game A yet;
`src/main.rs` only loads the config, prints the path, and opens an empty
Bevy window/prints engine version as the smoke test. No game files are read
or copied.

### 6) Smoke test
```sh
cargo run
```
Expected: first build takes several minutes (Bevy is large); final lines show
our printed config path and Bevy starting without a panic. A window may open
briefly on your session — tell me if you'd rather have headless log-only
output and I'll adjust `main.rs` to skip creating the window.

## What I will NOT do

- No `git commit`, no staging.
- No writing to Game A or Game B, no copying game files (extracted/ stays empty).
- No code beyond the skeleton. The format reader is a later step.

## How to check

After I implement, verify with:
```sh
cd "/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spiderman Shattered Decomp"
cargo --version
ls extracted ghidra
git status --short        # should list only source files, never game data
cat config.toml
cargo run                 # prints the Game A path, then Bevy starts
```

Reply "OK" and I'll execute steps 1–6. If you want changes (e.g., no window
popup, different Bevy version, different config filename), say so first.
