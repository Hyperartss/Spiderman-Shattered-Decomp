# STATUS

Read-only on Game A throughout. Nothing committed. Evidence in `logs/` only.

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

## Failed attempts (level geometry)

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
- Which pak is genuinely "the first level". `Tutorial010SMA` chosen from naming
  convention, which is evidence, not proof.

## Process notes

Two visual problems were reported fixed before the shipped binary reflected
the probe numbers: the triangle-strip walk advanced by 2 instead of 1 (5253
triangles instead of 9054), and the character's head block was never loaded.
Probe numbers were real but produced by Python while the Rust binary differed.
All numbers must now come from the shipped binary's own log.

Do not re-run a Python probe and report its numbers as the result of the
render. That is what caused both of the above.