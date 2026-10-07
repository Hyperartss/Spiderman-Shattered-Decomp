# Game A — file and folder survey (Spider-Man: Shattered Dimensions, PC, Build 711768)

Source: install at `/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spider-Man Shattered Dimensions`, read-only.
Engine: Beenox Goliath Engine (not UE3). Custom "GS" package format. ~14 GB total.

## Top level

| Path | What it is |
| --- | --- |
| `Data/` | ~12 GB, 219 files. All game content (maps, models, textures, scripts, audio streams) in custom `.pak` archives plus audio/shader streams. |
| `video/` | ~1.8 GB, 175 `.usm` files. Full-motion video / in-engine cutscene movies. |
| `Docs/` | ~172 KB, 4 files. Minimum system requirements, EULA, customer support page, readable (README). |
| `Game.exe` | Main game executable (UE/GOLITH engine runtime, imports `rld.dll`, `Data\GameLogic.dll`, D3DX9, CRI middleware). SecuROM-protected. |
| `Launcher.exe` / `Launcher_MC.exe` | Startup/front-end launchers. |
| `rld.dll` | SecuROM copy-protection loader (DRM). Not anti-cheat. |
| `Launcher_0.ico`, `SMSD.ico` | Icons. |

## Data/ by extension

| Ext | Count | Total size (approx) | Probable contents |
| --- | --- | --- | --- |
| `.pak` | 203 | ~7.7 GB | Custom "GS version 5.50.4066" archives (`buildman - Oct 19 2010` header). Compressed/encrypted containers of level/world data: maps, meshes/models, textures, animation, materials, gameplay scripts and config. One per level or content group. |
| `.dat` (`Streams*.dat`) | 4 | ~4.0 GB | CRI ADX audio/movie streaming containers (music, dialogue, SFX). Encrypted/packed chunks. |
| `.dll` | 2 (`GameLogic.dll`) | — | Game-specific logic/scripts compiled into a native DLL. |
| `.bnx` | 9 | ~40 KB | Localization text bundles (`igct*.bnx`, CRI CFT format, UTF-16 "Beenox PC-specific…" header). One per language + base. |
| `Shaders/3.0/` | 2 | ~234 MB | Compiled DX9 shaders: `Shaders.dat` (bytecode blob) + `Shaders.idx` (index/lookup). |

`Data/` has no loose maps/models/textures — everything is inside the `.pak` archives and `Streams*.dat`.

## .pak naming conventions (observed from listing)

- `Base*.pak` — shared content per Spider-Man variant (BaseAmazing, BaseNoir, BaseUltimate, Base2099, BaseHero, BaseEnemies, BaseGameplay).
- `<Villain>Main.pak` + `<Villain>01x….pak`, `<Villain>01xTransition….pak` — per-villain story arcs and individual levels (Electro, Carnage, Kraven, Goblin, Sand, Mysterio, Deadpool, Vulture, Hammer, Scorpion, Lady, Jugg, Osborn).
- `FrontScreen*.pak`, `Gallery.pak`, `WebOfSkills.pak`, `Tutorial*.pak`, `Legal.pak`, `Common.pak`, `Main.pak` — menus, extras, tutorial chapters, and shared/global content.
- Each level `.pak` is plausibly one map/scene: level geometry, collision, placed models, and lighting, plus its textures.

## video/

- 175 `.usm` files (Bink video). Subfolders: `Combo/` (57 files), `Credits/`, `DeadPool/` (92), `LevelIntros/` (12), `LogoIntros/` (3), `StoryVideo/` (7). Playable standalone; used for opening, cinematics, and transitions. The `.usm` header contains a path string like `video/…`, which confirms content type.

## Notes / caveats

- All sizes and counts are from `find`/`du`/`ls` at notes-creation time; "probable contents" for `.pak`/`.dat` is inferred from headers, folder names, and strings in `Game.exe`, not from a working parser.
- Nothing in here has been decompiled or extracted. Contents are compressed/encrypted; a reader is still TBD.
- `Streams*.dat` and `Shaders/3.0/Shaders.dat` dominate install size after textures.

## How I checked / how you can re-check

These were derived read-only with:
- `find . -type f | sed 's/.*\.//' | sort | uniq -c | sort -rn` (extension counts)
- `du -sh Data Docs video` and `du -ch Data/*.pak | tail -1` (sizes)
- `ls video/*` and `ls Data/Shaders/3.0` (subfolder layout)
- `xxd`/`strings` on file headers and `Game.exe` (format identification: GS/buildman header, CRI/Bink markers, Goliath source paths)

To verify a single number yourself, re-run the matching command in the install folder, e.g.:
```
cd "/home/hyper/Desktop/New Drive Files/WIP/SPider-MAn/Spider-Man Shattered Dimensions"
du -sh Data video Docs
ls Data/*.pak | wc -l
```
