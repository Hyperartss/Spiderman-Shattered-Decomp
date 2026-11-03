# Shader decompilation notes

The game ships Xbox 360 **Xenos microcode**, not source or DXBC. ReXGlue
translates that microcode to SPIR-V at runtime. The translator also contains a
ucode disassembler, so the guest shaders can be read without Ghidra:

```
./dump-shaders.sh [out-dir] [extra sd360 flags]
```

`Shader::AnalyzeUcode` (`src/graphics/pipeline/shader/translator.cpp:317`) writes
two files per shader when it is first analysed:

| File | Contents |
| --- | --- |
| `shader_<HASH>.ucode.bin.vert/.frag` | raw ucode, 3 dwords per instruction |
| `shader_<HASH>.ucode.vert/.frag` | readable disassembly |

`<HASH>` is the same hash the runtime log prints as
`Creating graphics pipeline state with VS <HASH>, PS <HASH>`, so a log line can
be mapped to a dump file directly.

A shader is dumped **when analysed**, not every frame. With a warm pipeline
cache nothing is dumped; `dump-shaders.sh` therefore forces a private
`--cache_root` and `--store_shaders=false`.

## What the disassembly looks like

```
/*    0.0 */       exec
/*    2   */          tfetch2D r0.xyz_, r0.xy, tf0
/*    0.1 */       alloc colors
/*    1.0 */       exec
/*    3   */          max r1._, r1, r1
              +       log r1._y__, r_abs[1].x
...
/*    7   */          mul r1._yzw, r1.yyzw, c56.xxyz
              +       exp r1.x___, r1.x
```

- `tfN` is a **texture fetch slot**, resolved through the fetch constant bound
  for that draw.
- `cNNN` are ALU float constants; the values are in the constant dump.
- `log` + `mulsc c54.x` + `exp` is `pow(x, c54.x)` — the game's gamma/tone-map.

## Limitation that matters

**`num_format` and `exp_adjust` are not in the shader.** They live in the
texture fetch constant (`xe_gpu_texture_fetch_t` word 3), bound per draw. So
decompiling the shader alone cannot explain a wrongly-scaled texture read; you
must pair the shader with the fetch constants for the same draw. The earlier
`fetchdump` instrumentation did exactly that, for example:

```
t=39.71 COPY dest=0678E000 info=000B4D01 fmt=26 exp_bias=11
t=39.72 tf0 base=0678E000 fmt=26 exp_adjust=-11   <- 5.11 fixed point read
t=39.72 COPY dest=06AB8000 info=01000100 fmt=2  exp_bias=0
t=39.72 tf0 base=06AB8000 fmt=2  exp_adjust=0     <- plain 8-bit target
```

The `fmt=26` / `exp_bias=11` / `exp_adjust=-11` chain is the one patches `0001`
and `0002` fixed. The `fmt=2` 8-bit resolve at `exp_bias=0` is a normal
normalized target and does not need the integer path.

## First dump (2026-11-02)

`logs/claude/shdump/`: 287 files, **71 fragment shaders** covering the main menu
and the first level. Menu-phase shaders are identifiable by their dump time
(they are analysed in one batch when the menu loads).

Notable menu-phase fragment shaders:

| Hash | Lines | tfetch | Shape |
| --- | --- | --- | --- |
| `3B08ACD15E33A5F7` | 6 | 1 | pure passthrough blit (`tfetch` + `max`) |
| `277A5088B86DFC45` | 8 | 1 | fetch, `sge` per channel against `c110`, multiply — sign/negative mask |
| `2B9DAC1EA005A58C` | 18 | 1 | fetch + `log`/`exp` pow + lerp with `c14/c54/c55/c56` |
| `3DEE22C16799CF5C` | 22 | 1 | same family, extra channel scale by `c10` |
| `84E0569DB3803343` | 15 | 1 | lighter variant, alpha out of `c56.w` |
| `A1A2EC7832776300` | 21 | 1 | same family, extra `mul` by `c10.wxyz` |

All of these are structurally sane: a single fetch, no branches, pow/lerp
post-process. Nothing in the menu shader set looks malformed, so a black frame
is far more likely to be the *input* being black (resolve/fetch constant) than
the ALU being translated wrongly.

## Next diagnostic

Dump the shaders AND capture the fetch constant for the draw that lands on a
black fade frame, then compare its `exp_adjust`/`num_format` against what the
translator assumes. That is one instrumented run, not a full SDK change.
