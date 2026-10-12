# STATUS

Read-only on Game A throughout. Nothing committed. Evidence in `logs/` only.

## Level structure — SOLVED (2026-10-08)

The level's `Env_Render` region is fully mapped. This is new information and it
supersedes the guesswork in the older notes.

Path: `Gen_EnvironmentLibrary (0x30) > GenSub_Resource (0x138D) > Env_Render (0x0BBB)`
— the same `0x30 > 0x138D > 0xBBB` path the older notes had guessed, now named.

Contents of `Env_Render` in the candidate first-level pak (248 chunks total):

| Chunk | Count | Role |
|---|---|---|
| `Octree_InfoHeader` / `Octree_Info` | 1 each | octree header; `Octree_Info` opens with float bounding boxes |
| `Octree_NodeList` | 1 | 225,600 bytes of spatial node data |
| `RenderOctree_Prims` | 1 | 64,000 bytes — the drawable primitives |
| `RenderOctree_PrimIdx` | 1 | 42,048 bytes, max value 999 |
| `RenderOctree_Obj` | 1 | 3,192 bytes, packed float bboxes |
| `RenderOctree_Instances` / `InstanceIdx` | 1 each | placed objects |
| `RenderOctree_MaterialList` | 1 | 628-byte header + 78 materials |
| `GenSub_VertexList` / `GenSub_IndexList` | 1 each | the actual mesh data |

So the level geometry is **not** a single blob: the octree (`NodeList` + `Prims`)
decides what is drawn where, and `VertexList`/`IndexList` hold the vertex pool.
Connecting them is the open question.

## Update — 2026-10-08 (Xbox 360 cross-check)

- The 360 `.pkz` container format is fully decoded there, and its chunk tree
  uses the same layout as Game A. Game A parsed with **0 problems** and root
  length == file size - 12 on the character, enemy, candidate-level, main,
  and one antagonist-level pak. Rough totals per file.
- Field order confirmed: `u32 id, u16 version, u16 has_children, u32 len`
  (little-endian on Game A). See docs/FORMAT.md correction. This resolves why
  the old notes called them just `a`/`b`.
- Full resource-name index written: `logs/resource_names_all.log`, built by
  `tools/decode_resource_names.py`. Results: every .pak parses; one
  character-variant pak gives 1,870 decoded resource names; the biggest paks
  are the enemy / antagonist-arc / character-variant group (pak filenames
  withheld per ground rules — see `logs/resource_names_all.log`);
  level environments are identified by an `Env_Render` (0xBBB) body, e.g.
  every antagonist `*Main*` level pak.
- No game data was extracted and no files outside this repo were modified.

## Summary

Container format solved. Character model decodes but its head block is
unreferenced by the index data. Level geometry does **not** currently decode
into coherent geometry — two attempts have failed, so per the agreed rule this
step is stopped and recorded.

## Proven (numbers support these)

### Container

- 203 paks, ~8,199,655,960 B total. Every pak is one type-`0x1` root chunk
  (203/203).
- 12-byte chunk header: `u32 type, u16 a, u16 b, u32 len`. Children tile the
  parent payload exactly.
- `u32 @0x08 == filesize - 12` in all 203.

### Character model

- `0x1389` (u16 indices) separates submeshes with `0xAAAA`; indices inside a
  submesh are local to it.
- `0x1388` lays blocks out contiguously as **68-byte records**: `f32[3]`
  position at +0, `f32[3]` normal at +12. Evidence: 583/583 and 4396/4396
  unit-length normals in the head and body blocks; every other stride tested
  gives ~70% finite.
- Five blocks, each boundary landing exactly on a `0xAA` pad run:

  | block | bytes | verts | unit normals | location |
  |---|---|---|---|---|
  | 1 | 0..832 | 32-stride, 26 | 26/26 | head height |
  | 2 | 832..4352 | 32-stride, 110 | 110/110 | head height |
  | 3 | 4352..41412 | 68-stride, 545 | 545/545 | limb, x≈0.6 |
  | 4 | 41440..340368 | 68-stride, 4396 | 4396/4396 | full body |
  | 5 | 340384..380028 | 68-stride, 583 | 583/583 | **head only** |

- Index submeshes need exactly 26 / 110 / 545 / 4396 verts, matching blocks
  1–4. **Block 5 has no index data referencing it** — this is why the face is
  holed. Where its indices live is unknown.
- Each submesh body is a **triangle strip with degenerate bridges**. Proof:
  11944 manifold edges vs 601 for a list decode; open boundary edges fall
  7894 → 2037; all 5077 verts referenced (list decode reaches only 4739).
- Winding parity unproven (~50% agreement either way); material must be
  double-sided.

### Transform table (level pak)

- `0x1ff` contains affine matrices: 58 in 58 chunks, orthonormal bases, last
  row `(0,0,0,1)`. Translations cluster (tx p50=0.000, ty p50=19.734,
  tz p50=17.160); 27 of 58 within 50 units of the origin.
- **Disproved** as a per-submesh transform for the big geometry chunk: it
  covers only 58 of 427 submeshes, and the placed submeshes land nowhere near
  the translation values (`logs/xf_mapping_test.log`). It belongs to a
  different object.

## Level vertex layout — STOPPED, unresolved (2026-10-08)

`Env_Render` structure is now mapped (see above), but the **vertex record layout
is still not solved**. Per the "fail twice" rule this step is stopped and written
up rather than guessed at again.

### What is proven

- `0x1388` (vertex) and `0x1389` (index) for the level: 8,100,832 and 837,088 bytes.
- **The 68-byte stride in this file is disproved for the level.** 68 does not
  divide 8,100,832 (remainder 60). 32 divides exactly: 253,151 records.
- **32 bytes is right for models**, proven two independent ways in
  the candidate first-level pak: four chunks where `len / (max_index+1)` is exactly 32.00
  and 100% of records have a finite position and a unit normal at +12
  (chunks of 2080/65, 1952/61, 1280/40, 1280/40 bytes).
- Index stream: 418,544 `u16`, `0xAAAA` appears 426 times → 415 submeshes.
  Each submesh is 0-based (min 0 in all first 15 checked).

### Why 32 alone does not explain the level

- Packing submeshes back-to-back at 32-byte stride gives only **6.1%** unit
  normals overall (406 of 414 submeshes score below 90%). Individually the first
  few submeshes look perfect at the naive offset (submesh 1 = 100% at 10,240),
  so the record layout is right but **the submesh→offset mapping is wrong**.
- The index count cannot account for the buffer: sum of (max_index+1) is 159,521
  records but the chunk holds 253,151 — a ratio of 1.587. So either there is
  other data in the chunk, or each submesh has a base offset we have not found.
- A brute-force search for a clean 48-record run found offset 9216 scoring 100%
  for *every* submesh, which means the search was finding one long clean region,
  not a per-submesh start. That test was invalid and should not be trusted.

### Prime suspects for next time

1. `GenSub_VertexList` for a level may be a **pool with base offsets**, not
   concatenated per-submesh blocks. The index being 0-based per submesh is
   consistent with a global pool plus a base that is stored elsewhere —
   possibly in `RenderOctree_Prims` (6,4000 bytes, divides cleanly by 20/32/40)
   or `Octree_NodeList` (225,600 bytes, opens with float bounding boxes).
2. The remaining 2,995,392 bytes (93,591 records at 32) may be a second pool,
   LOD data, or vertex colours/UVs in a wider record.
3. `RenderOctree_Obj` (3,192 bytes) had `0xFF7FFFFF` sentinels — looks like
   packed float bounding boxes, possibly the key to the spatial mapping.

### Not to be repeated

- Do not test stride by unit-normal fraction alone. 32, 64, 96 and 128 share byte
  offsets and all score ~92% for the same records. Always require the stride to
  divide the length AND cross-check against `len / (max_index+1)`.
- Do not assume "indices local per submesh" means "submeshes packed
  consecutively". That assumption is what failed here.

### Earlier failed attempts (level geometry)

Both failed. Stopped here per the "fail twice" rule.

### Attempt 1 — subset by centroid distance

- 273 of 427 submeshes kept, but bbox still 5,493 units across.
- Cause: filtering on centroid position ignores submesh **radius**. Radii
  range 0.568 to ~1996, so pieces centred near the middle still extend
  thousands of units.

### Attempt 2 — subset by radius ≤ 50

- 98 submeshes, bbox 155 units, human-scale. User saw "a mess of geometry".
- `dropped_far=0` — the 260-unit centroid filter did no work at all; every
  rejection was size-based. So the subset was never a coherent space, just a
  size cut.

### Attempt 3 — full level, no filter

- 419 submeshes, 155,549 verts, 152,503 triangles, bbox 14,114 units.
- User saw "triangular mess".
- Degenerate ratio is the warning sign: **253,021 degenerate triangles dropped
  out of ~405,524 strip windows = 62%**. The character model, which renders
  as a coherent figure, drops 5,272 of 14,332 = 37%. A 62% degenerate rate
  means most index windows are not real triangles, so the strip layout being
  applied to the level is probably wrong.

## Prime suspect for the next attempt

**The level was decoded at 32-byte stride, but the character model was proven
to be 68 bytes.** That inconsistency is mine: I proved 68 for the model and
then used 32 for the level without retesting. The level tail supports doubt:

- The big level vertex chunk is 8,100,832 B. The 427 submeshes need 159,585
  verts, which is 5,106,720 B at stride 32. **2,994,112 B (93,566 verts at
  stride 32) is unreferenced** by this index chunk and is not `0xAA` padding
  (only 0.3%).
- That tail is **not** plain 32-byte vertex data: 76.3% finite and only 32.2%
  unit-length normals, bbox spanning −549,785 to 4,344. Values that extreme
  are a sign of misalignment, not geometry.

So the next test is to re-probe the level at 68-byte stride (and the other
candidates 28/40/56/72) and see whether the unit-normal fraction jumps and the
unreferenced tail shrinks or becomes coherent. If 68 wins the same way it won
for the model, the level decode and the model decode should share one layout.

## Not tested / open

- The character's block 5 indices.
- Chunk `0x32e` (26724 B, beside the character model) is undecoded; its head
  looks like offset pairs then floats. Possibly bone hierarchy or skin weights,
  and the only candidate found for real animation data.
- 583 unconsumed bytes past the character vertex blocks contain values in
  [0,1] with a `1.0` sentinel and `nan` — consistent with weight data, not
  positions.
- Level collision: a rapier trimesh built from the decoded triangles reported
  **zero** contact events while a dropped capsule fell from y=103 through the
  whole level to y=-225. Either the geometry is too sparse to catch it or the
  contact event wiring is wrong. Unresolved.
- Which pak is genuinely "the first level". The tutorial-010 candidate was
  chosen from naming convention, which is evidence, not proof. (The pak
  filename is withheld from tracked files per the ground rules; it is in
  `docs/` history and `logs/` if needed.)

## Process notes

Two visual problems were reported fixed before the shipped binary reflected
the probe numbers: the triangle-strip walk advanced by 2 instead of 1 (5253
triangles instead of 9054), and the character's head block was never loaded.
Probe numbers were real but produced by Python while the Rust binary differed.
All numbers must now come from the shipped binary's own log.

Do not re-run a Python probe and report its numbers as the result of the
render. That is what caused both of the above.