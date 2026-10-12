# DESIGN — Rust PC port: first level, fully polished

Goal (one sentence): **a native Rust/Bevy app that loads the first level
from my own installs (PC and 360), renders it correctly, and lets me walk
around in it with a placeholder character — fully polished.**

"Port" here means a reimplementation, the IW4L pattern: the repo contains
only our code; the app reads the player's own game data at runtime; nothing
from the original is copied or shipped. Reference projects: `vladtrc/iw4L`
(Rust/Bevy runtime for an owned install), guides 3 & 12 from
`trevaintdead/ai-game-modding-guides`, and `bethington/ghidra-mcp` + the
local Ghidra 12.1.4 for the native code.

Plan only. Nothing runs until you reply OK. After OK I do exactly one step,
report numbers **from the shipped binary's log**, and wait again.

## What changed vs the old plan

- **Scope**: the user decided on a full rewrite/port; v1 = engine + first
  level, fully polished ("fully polished" defined at the bottom).
- **Data source**: PC install **and** the 360 ISO, both read-only. This
  supersedes the earlier "never touch Game B" default (your decision).
- **Repo is already public**: hygiene (step 0) must happen before any new
  code lands, else new commits keep leaking your paths/email into a public
  repo.
- **Tooling**: Ghidra 12.1.4 already sits next to the project; wire it in
  via Ghidra MCP so I can decompile `Game.exe` / `GameLogic.dll` read-only
  when the data formats or gameplay logic need ground truth.

## How the rewrite is built (IW4L pattern)

1. Read formats from the player's own install (PC `.pak` / 360 extraction).
2. Engine: Bevy 0.15 already chosen; rapier for physics; wgpu rendering.
3. Rules: movement first, then maps, then interactions — **the 4-step
   geometry → collision → camera → movement core stays the skeleton.**
4. Compare against the real game; keep a "what works / what's missing" list
   in README (IW4L does this — narrow claims survive contact with users).

## Steps (one at a time; each reports numbers and waits for OK)

### Step 0 — Repo hygiene (do this before any new code)

Concrete, small, reversible:

1. **Stop tracking `config.toml`** (machine-specific path). Add
   `config.example.toml` with a placeholder path; the binary reads a local,
   gitignored `config.toml` (already ignored by the whitelist).
2. **Remove the absolute install paths from tracked files** (`agent.md` lines
   with install paths, `docs/FILES.md`, `docs/FORMAT.md`) → replace with
   `<ABSOLUTE PATH TO YOUR INSTALL>` placeholders.
3. **Game-name / asset-manifest decision** (one decision, default
   recommended): keep docs neutral going forward — "Game A", the hex chunk
   paths, no pak filenames in tracked files (that is the rule that already
   exists in this project). README may keep a plain one-line "what this is".
   If you prefer the open naming style (IW4L names its game), say so and I
   apply it everywhere consistently.
4. **Commit identity**: set a repo-local git identity for future commits
   (GitHub noreply-style address) so new commits don't carry your personal
   email. Existing 4 public commits still contain the old identity; fixing
   those is a force-push history rewrite — **separate decision, not in this
   step** unless you say otherwise.
5. **Re-scan and report**: run the greps (paths, emails, secrets, asset
   files) before and after; report the diff explicitly.

Success: clean greps, `config.toml` untracked, no personal paths in tracked
files. Log: the before/after grep outputs (evidence only, no game data).

### Step 1 — Tooling: Ghidra MCP

- Clone `bethington/ghidra-mcp` **outside the repo** (e.g. a `tools/`
  sibling of the project), build/deploy against the already-installed
  Ghidra 12.1.4, register the MCP server with OpenCode, verify
  `check_connection` and `mcp/health`.
- Ghidra projects/databases stay out of the repo (gitignored). Only notes in
  `logs/` and `docs/` are shareable.
- `morluto/rea` is deferred — it is heavier (Node, extra providers) and not
  needed for step 2+3.

Success: I can call Ghidra MCP tools against your own binaries, read-only.

### Step 2 — Geometry: make the first level render correctly

Current blocker, honestly: the level vertex chunk is **653 blocks**, not one
array; every single-stride decode (24–72, offsets 0–16/12–24) tops out at
**36.5% unit normals** vs the model's 100%. That is why it renders as a
pyramid/ramp. Approach:

1. Per-block decode: use the 0xAA pad runs (652 of them) as block
   boundaries; for each block, score stride × offsets on finite + unit-normal
   fraction; choose per block. Expect mixed strides (blocks 1–2 of the model
   are 32, blocks 3–5 are 68 — same pak, different strides — so this is the
   real structure).
2. Match decoded blocks to the 427 index submeshes; render the full set.
3. Verify: the window reads as a level — a floor you could stand on, plus
   walls/props — not a pyramid. Numbers from the binary's log
   (`logs/geometry.log`): blocks, per-block strides, submesh↔block mapping,
   triangle count, degenerates, bbox.
4. If the decode fails two more times: update `STATUS.md`, stop, report
   (you may say continue — that's your call, not mine).

Success: a recognizable level, confirmed by you in the window.

### Step 3 — Collision

- Rapier trimesh from the decoded triangles; dynamic capsule, small radius,
  CCD enabled; log every contact to `logs/collision.log` with world
  position; add a "is there a floor at the drop point?" diagnostic.
- Earlier drop test produced zero contacts (capsule fell through to
  y=-225). Fix = CCD + `ContactForceEvent` + the diagnostic, not guessing.

Success: a capsule dropped from above lands; log shows a contact at a
plausible position.

### Step 4 — Camera

- Third-person follow camera, mouse-look orbit without clipping through the
  capsule; camera pos/target logged at 1 Hz to `logs/player.log`.

Success: orbiting changes the logged position each second.

### Step 5 — Movement (v1 polish)

- WASD + mouse + Space; gravity, walk/run/jump, grounded flag; 60 s stability
  test with continuous position trace to `logs/player.log`, contacts to
  `logs/collision.log`. Character = gray placeholder capsule (the holed-head
  model decode is parked; it is art, not engine).

Success: 60 s of walking/jumping with no floor falls and a position trace
you can plot.

### Step 6 (after v1) — 360 ISO content

- Read-only extract from the 360 image into `extracted/`; compare formats
  and content against PC; adopt higher-quality/missing assets.
- This is the first step that needs the 360 data path confirmed safe.

### Step 7+ (after) — the rest of "entire game", in layers

More levels walking → combat/AI → web-swinging movement → audio (CRI ADX)
→ menus/UI → story flow. Each is its own planned step.

## Defaults (say the word to change any)

| decision | default |
|---|---|
| first-level pak | the pak chosen earlier, selection reason logged (evidence, not proof) |
| engine | Bevy 0.15.3 (already pinned) |
| physics | `bevy_rapier3d` 0.29 (already pinned) |
| window | 1280×720 windowed |
| camera | third-person follow |
| player | placeholder capsule |
| DRM | never touched; we read data files only |
| Ghidra | local 12.1.4, read-only, DBs never in repo |
| 360 | read-only; extraction into `extracted/` only |

## Ground rules (unchanged, with the one update)

- Game A read-only. **Game B (360) now in scope but also read-only.** Never
  write to either install; extract only into `extracted/`.
- No game data, payload bytes or game names in tracked files; evidence to
  `logs/` only; chunk types as hex.
- Guesses marked as guesses; claims only from numbers in the shipped
  binary's log, never from a Python probe.
- A step that fails twice → update `STATUS.md`, stop, report.
- No commits unless asked.
- Never put credentials in the repo or in any file I can read.

## What I will NOT do

- No writes to either install; no extraction outside `extracted/`.
- No commits or staging without being asked.
- No claims about chunk semantics without a supporting number.
- No step executed before your OK.

## "Fully polished" — the v1 bar (success measured here)

- The first level renders as a level (not a pyramid/mess), from real decode.
- The capsule stands on the floor and stays on it; no falls through.
- Third-person camera orbits cleanly.
- WASD + mouse + Space feel stable for 60 s+.
- `logs/geometry.log`, `logs/collision.log`, `logs/player.log` back every
  claim with numbers.
- README has a "what works / what's missing" list in IW4L style.

Reply OK to start Step 0 (repo hygiene).