# DESIGN — Walkable first-level geometry (Steps 1–4)

Goal (one sentence): load the first level's geometry and let the user walk
around in it with a plain placeholder character.

Plan only. Nothing runs until you reply OK. After OK I do exactly one step,
report numbers from the binary's own log, and wait again.

## Course correction

I have been doing format archaeology and burning your time on it. Stopping
that. The geometry decode is already proven good enough to walk on:

- Level geometry: `0x1388` / `0x1389` sibling pair under `1 > 0x30 > 0x138d >
  0xbbb`, from `Tutorial010SMA.pak`.
- Vertex record: 32 bytes, `f32[3]` position at +0, `f32[3]` normal at +12.
  419 of 427 submeshes have finite bboxes; 121 reach a 1.000 unit-normal
  score.
- Index: u16, `0xAAAA` separates submeshes, indices local per submesh, each
  body a triangle strip with degenerate bridges.
- The `0x1ff` transform table is real but belongs to a different object —
  tested and disproved as a submesh transform (369 of 427 submeshes get no
  transform, and placed submeshes land nowhere near the translations).

**Honest tradeoff:** the geometry is a full city-scale level (~14,000 unit
span) and I do not have its logical grouping or spawn point. So Step 1
renders a *playable subset* — the dense cluster a character can stand in —
not a faithful reconstruction of the level. That is a deliberate choice to get
you a walkable world now instead of more reverse engineering. Fidelity can be
improved later if you want it; the engine loop works either way.

## Step 1 — Geometry

- Rank all 427 submeshes by centroid distance from the dense centre of the
  level. Keep a cluster sized for a human-scale space. The cutoff is a single
  named constant in source, logged so the choice is visible and changeable.
- Decode kept submeshes: strip walk, advance one index per triangle, skip
  degenerate bridges, alternating winding, double-sided material.
- Camera outside the subset bounding sphere, one directional light.
- Log to `logs/geometry.log`: submeshes kept/dropped, vertices, triangles,
  bbox, subset radius, load time.

Success: a recognizable structure at human scale — floor plus walls or props.
You confirm from the window.

## Step 2 — Collision

- **Default: derive collision from Step 1's triangles.** No second format to
  reverse. A dedicated collision chunk would only replace this if *proven*.
- Physics: `bevy_rapier3d` matched to Bevy 0.15 — collision events built in,
  one new dependency, version pin a **guess** until cargo resolves it.
- Log every contact (collider, world position) to `logs/collision.log`.
- Success: a capsule dropped from above the bbox lands on geometry and the log
  shows a contact at a plausible position.

## Step 3 — Camera

- Third-person follow camera behind the placeholder capsule.
- Log camera position and look target at 1 Hz to `logs/player.log`.
- Success: mouse-look orbits without clipping through the capsule; log shows
  changing positions.

## Step 4 — Movement

- Capsule + gray material, gravity, walk/run, jump. Controls: WASD + mouse +
  Space.
- Log at 1 Hz plus on any contact: position, velocity, grounded flag →
  `logs/player.log`; contacts → `logs/collision.log`.
- Success: 60 s walk with no falling through the floor and a continuous
  position trace in the log.

## Defaults (I set these so one OK is enough — say the word to change any)

| decision | default |
|---|---|
| level pak | `Tutorial010SMA.pak` (naming convention, evidence not proof) |
| geometry | filtered playable subset, radius cutoff logged |
| physics | `bevy_rapier3d` |
| camera | third-person follow |
| window | 1280×720 windowed |
| character model | leave the holed head; level is the goal |

## Ground rules (unchanged)

- Game A read-only. Never write to the install; extract only into
  `extracted/`; never touch Game B.
- No game data, payload bytes or game names in tracked files; evidence to
  `logs/` only. Chunk types as hex.
- Guesses marked as guesses. Claims only from numbers that prove them.
- A step that fails twice → update `STATUS.md`, stop, report.
- No commits unless asked.
- **Every reported number comes from the shipped binary's log, not a probe.**
  Two earlier reports were wrong because the Python probe and the Rust binary
  diverged; see `STATUS.md`.

## What I will NOT do

- No writes to Game A, no extraction outside `extracted/`, no Game B.
- No commits or staging.
- No claims about chunk semantics without a supporting number.
- No step executed before your OK.

Reply OK to start Step 1.