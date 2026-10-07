# Game A `.pak` format, in plain language

This is what we have confirmed from the numbers. Everything in a `.pak` is a
tree of "chunks". The file is a single tree whose leaves are the actual data
(textures, geometry, strings). We have not decompressed or extracted anything
into the repo.

## 1. The basic building block: a chunk

A chunk is:

```
u32  type   (4 bytes, little-endian)
u16  a      (2 bytes, little-endian)
u16  b      (2 bytes, little-endian)
u32  len    (4 bytes, little-endian)
[len] payload bytes
```

So every chunk has a 12-byte header, then `len` payload bytes. Total chunk
size is `12 + len`.

Rule that held for our tree walk: a chunk's payload either is itself a
sequence of child chunks that tile the parent exactly (every byte covered, no
leftover), or it is raw data (a leaf). For all 203 `.pak` files we tested,
this rule let us walk the whole tree with no break: **every file parses
cleanly** as one tree under a type-1 root.

## 2. Top level of a `.pak`

- The whole file is one chunk of type `0x1`.
- Its `len` equals `file size - 12` (this held in all 203 files).
- Its payload starts at file offset 0x0c.

## 3. First chunks inside the type-1 root (always present)

In every pak we looked at, the payload of the type-1 root starts with a fixed
sequence of small chunks. The first is:

- `type 0x11`, `a=3`, `b=0`, `len=264` — an "info" chunk, present in all
  files, always the same length (264). Its contents are not decoded.
- After that come the small fixed-size metadata chunks:
  - `0x1a` (len 24), `0x19` (len 20), `0x25` (len 16), `0x10` (len 4–8,
    present in 202/203), and `0x37` (len 0).

These look like a fixed metadata header (counts, ids, platform tags). We did
not decode them.

## 4. A `0x26` chunk is one named resource

`0x26` is a container used as a resource record. Its direct children are
always the same shape:

- one `0x138e` record, always len 88. The first 4 bytes are an id-like u32,
  then a couple of small fields, then a zero-padded ASCII name of at most
  64 bytes starting at payload offset 24. (Names are treated as data; they
  are not written into tracked files.)
- one body chunk, whose type tells you what kind of resource it is:
  - `0x195` → texture-like payload (see §5)
  - `0x401` → another body type
  - `0x325` → a sub-chunk that itself holds a `0x1388`/`0x1389` pair
  - `0x70b` → another body type
  - `0x4b1`, `0x4b6`, `0x4b9`, `0x4bc`, `0x4bb`, `0x838`, `0x4b5` → other
    container variants

Every `0x26` we parsed (39442 of them) parsed cleanly.

## 5. `0x195` body = a DDS image with a 28-byte wrapper

A `0x195` chunk's payload layout:

```
[0..28)   28-byte prefix
          u32 @0  = payload length minus 24 (held in 21842/21853 cases)
          u32 @4, @8      mostly 0
          u32 @12 usually 1
          u32 @16 usually 0x3f800000 (~1.0f)
          u32 @20  mostly 1 or 0
          u32 @24  mostly 0
[28..     "DDS " magic + DDS header + image data
```

- The DDS magic always sits at payload offset 28 (checked on all 21853
  instances).
- fourcc is DXT1 (8544), DXT5 (12551), or an undecodable raw value (758).
- We verified the exact-size rule on the DXT1/DXT5 rows:
  `chunk_len - 28 == 4 ("DDS ") + 124 (DDS_HEADER) + data_size`, where
  `data_size` is the sum over mips of `w*h*blocksize` (8 for DXT1, 16 for
  DXT3/DXT5). This matched **21095/21095**.
- The 758 with unparsable fourcc (fourcc=0, flags `0x41`/`0x20000`,
  bitcount 32 or 8, masks for RGBA8 / 8-bit gray) are **not yet understood**
  and are excluded from the exact-size claim. This is a "not tested" area.

## 6. Chunk-type table

| type | what we think it is | status | evidence |
|---|---|---|---|
| `0x1` | root of the whole pak | proven | every pak is one type-1 chunk ending at file size |
| `0x11` | fixed info block, len 264 | proven (shape) | tiles as child in 0/203 (leaf data), always len 264 |
| `0x26` | resource container | proven | always a strict container; holds `0x138e` + one body |
| `0x138e` | name record | proven (shape) | len always 88, name at offset 24 |
| `0x195` | DDS-carrying body | proven (shape) | 100% exact-size match on DXT1/DXT5 rows |
| `0x1388` | vertex-like leaf, len%32==0 | guess | 2979/2979 len%16==0 and len%32==0; best unit-normal ≈0.30 |
| `0x1389` | index-like leaf, near 0x1388 | guess | always sibling of `0x1388`; u16 max-fit only ~35%, u32 never fits |
| `0x12d` | small leaf | unknown | found under `1 > 0x30 > 0x138d > 0xbbc` |
| `0x401` | secondary body type | unknown | appears under `0x26` instead of `0x195` |
| `0x70b` | body type, small | unknown | appears rarely under `0x26` |
| `0x4b1`,`0x4b6`,`0x4b9`,`0x4bc`,`0x4bb`,`0x838`,`0x4b5` | misc containers/leaves | unknown | appear under `0x26` as alternative small bodies |
| `0x18`, `0x1b`–`0x1f`, `0x21`, `0x24`, `0x27`, `0x28`, `0x2a`, `0x2e`, `0x32`–`0x35` | containers of per-pak fixed blocks | proven (shape) | always tile cleanly in the strict test |
| `0x2e` | leaf with big payloads (~2.7 MB max) | unknown | low entropy?, few samples |
| `0x2a`, `0x24`, `0x13`, `0x16` | leaf/small containers | unknown | fixed sizes in many |
| `0x3x` (`0x326`/`0x327` etc.) | inside `0x325` | unknown | small containers, sizes 20/52 |

## 7. Not tested (we did not run a test for these)

- The 758 unparsable-fourcc `0x195` payloads: format not decoded.
- Names: extracted but not matched against actual resources.
- Whether chunk `a`/`b` fields carry type info (guess: `a`≈payload kind, `b`≈0).
- The meaning of `0x11`'s 264 bytes; of the fixed small chunks `0x1a/0x19/0x25/0x10/0x37`.
- Any content outside a `0x26` + body structure; `Streams*.dat`, `.usm`, `.bnx`, `Shaders.dat`.
- Mip-count rounding for the 758 unparsable DDS rows (only floor=227/758, ceil=0/758 tested).

## How to check

```sh
cd "/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spiderman Shattered Decomp"

# DDS magic offset = 28 on every instance, csv of which 758 overlap the 11:
grep -A3 "=== overlap ===" logs/chunks.log

# uncompressed DDS size match (floor-mip sizing)
grep -A3 "=== uncompressed" logs/chunks.log

# the fixed 88-byte name records and names: see logs/names.log
head logs/names.log

# extraction mapping (name -> source pak, offsets, sizes): see logs/extract.log
cat logs/extract.log

# note: older runs (top_ok=203/203, size match 21095/21095) were overwritten;
# re-run the corresponding probes if you need those lines back.
```
