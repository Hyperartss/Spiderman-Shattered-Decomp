// Renders the game A character model from the installed PC game data.
// Model names and pak filenames are deliberately kept out of this source.
//
// Usage:
//   cargo run --release -- "<path>/Data/BaseUltimate.pak"
//
// Reads the pak read-only and writes only logs/. No game data is copied out.
// Bevy 0.15 API (see the compile notes in docs/DESIGN.md).
//
// ---------------------------------------------------------------------------
// WHAT IS PROVEN (STATUS.md records how each number was obtained)
// ---------------------------------------------------------------------------
//
// Container: a .pak is one Root chunk (id 0x01). Chunk header is 12 bytes,
// little-endian: u32 id, u16 version, u16 hasChildren, u32 length. Children
// tile the parent payload exactly; 203/203 paks walk with zero errors.
//
// The character mesh is a GenSub_VertexList (0x1388) paired with a
// GenSub_IndexList (0x1389) under one parent, here
// Gen_PostLoadData (0x26) > Geo_GeomLOD (0x325).
//
// The vertex chunk is NOT one uniform array: it is a concatenation of blocks
// with different record strides. All five blocks re-verified today at 100.0%
// unit normals and 100% finite positions (tools/split_vertex_blocks.py):
//
//   block 1    0 ..    832   stride 32   26 verts    100% / 100%
//   block 2    832 ..  4352   stride 32  110 verts    100% / 100%
//   block 3  4352 .. 41412   stride 68  545 verts    100% / 100%
//   block 4 41440 ..340368   stride 68 4396 verts    100% / 100%
//   block 5 340384..380028  stride 68  583 verts    100% / 100%
//
// Record: f32[3] position at +0, f32[3] normal at +12. Bytes +24.. are unused
// here (UV / other data, not decoded).
//
// The index chunk splits on 0xAAAA into 4 submeshes needing exactly
// 26 / 110 / 545 / 4396 vertices, matching blocks 1-4. Two independently
// derived numbers agreeing is the main evidence this layout is right.
//
// Block 5 (583 verts, the head) has NO index data referencing it. Provenance
// unknown, so skipped - that is why the face is missing.
// NOT TESTED against the game's own render; we cannot run the game.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// (start, end, stride, vertex_count) over the vertex buffer.
const BLOCKS: [(usize, usize, usize, usize); 4] = [
    (0, 832, 32, 26),
    (832, 4352, 32, 110),
    (4352, 41412, 68, 545),
    (41440, 340368, 68, 4396),
];

/// VertexList length of the base character mesh.
const CHARACTER_VERTEX_LEN: u64 = 380032;

struct Chunk {
    payload: u64,
    len: u64,
    typ: u32,
    parent: u64,
}

/// Walk the chunk tree. `hasChildren` is honoured, so only containers recurse.
fn walk(f: &mut File, start: u64, clen: u64, depth: u32, out: &mut Vec<Chunk>) {
    if depth >= 12 {
        return;
    }
    let payload_end = start + 12 + clen;
    let mut o = start + 12;
    let mut hdr = [0u8; 12];
    while o + 12 <= payload_end {
        if f.seek(SeekFrom::Start(o)).is_err() || f.read_exact(&mut hdr).is_err() {
            break;
        }
        let t = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let has_children = u16::from_le_bytes([hdr[6], hdr[7]]) != 0;
        let l = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as u64;
        let next = o + 12 + l;
        if next > payload_end || next <= o {
            break;
        }
        out.push(Chunk {
            payload: o + 12,
            len: l,
            typ: t,
            parent: start,
        });
        if has_children {
            walk(f, o, l, depth + 1, out);
        }
        o = next;
    }
}

fn rd(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

struct Decoded {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    log: String,
    valid: bool,
}

fn decode(pak: &str) -> Decoded {
    let mut log = String::new();
    log.push_str(&format!("pak={pak}\n"));

    let mut f = File::open(pak).expect("cannot open pak (read-only open, check path)");
    let mut hdr = [0u8; 12];
    f.read_exact(&mut hdr).unwrap();
    let root_len = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as u64;
    log.push_str(&format!("root length {root_len}\n"));

    let mut chunks: Vec<Chunk> = Vec::new();
    walk(&mut f, 0, root_len, 0, &mut chunks);
    log.push_str(&format!("chunks walked: {}\n", chunks.len()));

    let v = chunks
        .iter()
        .find(|c| c.typ == 0x1388 && c.len == CHARACTER_VERTEX_LEN)
        .unwrap_or_else(|| {
            panic!("no VertexList of len {CHARACTER_VERTEX_LEN} - pass the character pak")
        });
    let i = chunks
        .iter()
        .find(|c| c.typ == 0x1389 && c.parent == v.parent)
        .expect("no sibling IndexList");
    log.push_str(&format!(
        "vertex chunk payload={} len={}\nindex  chunk payload={} len={}\n",
        v.payload, v.len, i.payload, i.len
    ));

    let mut vbuf = vec![0u8; v.len as usize];
    f.seek(SeekFrom::Start(v.payload)).unwrap();
    f.read_exact(&mut vbuf).unwrap();
    let mut ibuf = vec![0u8; i.len as usize];
    f.seek(SeekFrom::Start(i.payload)).unwrap();
    f.read_exact(&mut ibuf).unwrap();

    // ---- vertices, block by block, verifying as we go ----
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    log.push_str("\nblocks (unit normal = finite and |n| ~ 1):\n");
    for (bi, &(start, end, stride, expect)) in BLOCKS.iter().enumerate() {
        let n = (end - start) / stride;
        let (mut finite, mut unit) = (0usize, 0usize);
        for k in 0..n {
            let o = start + k * stride;
            let p = [rd(&vbuf, o), rd(&vbuf, o + 4), rd(&vbuf, o + 8)];
            let mut nm = [rd(&vbuf, o + 12), rd(&vbuf, o + 16), rd(&vbuf, o + 20)];
            if p.iter().any(|c| !c.is_finite()) || nm.iter().any(|c| !c.is_finite()) {
                // keep the slot so indices stay aligned, but make it safe
                positions.push([0.0, 0.0, 0.0]);
                normals.push([0.0, 1.0, 0.0]);
                continue;
            }
            finite += 1;
            let len = (nm[0] * nm[0] + nm[1] * nm[1] + nm[2] * nm[2]).sqrt();
            if (len - 1.0).abs() < 0.05 {
                unit += 1;
            }
            if len > 1e-6 {
                nm = [nm[0] / len, nm[1] / len, nm[2] / len];
            } else {
                nm = [0.0, 1.0, 0.0];
            }
            positions.push(p);
            normals.push(nm);
        }
        log.push_str(&format!(
            "  block {}: bytes {}..{} stride {}  verts {} (expected {}) \
             finite {finite}  unit {unit}  [{:.1}% unit]\n",
            bi + 1,
            start,
            end,
            stride,
            n,
            expect,
            100.0 * unit as f64 / expect.max(1) as f64
        ));
    }
    log.push_str(&format!("total verts decoded: {}\n", positions.len()));

    // ---- indices: split on 0xAAAA, walk as a triangle strip ----
    let raw: Vec<u16> = ibuf
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let mut segs: Vec<Vec<u16>> = Vec::new();
    let mut cur: Vec<u16> = Vec::new();
    for x in &raw {
        if *x == 0xaaaa {
            segs.push(std::mem::take(&mut cur));
        } else {
            cur.push(*x);
        }
    }
    segs.push(cur);
    segs.retain(|s| !s.is_empty());

    let needs: Vec<usize> = segs
        .iter()
        .map(|s| (*s.iter().max().unwrap() as usize) + 1)
        .collect();
    let block_needs: Vec<usize> = BLOCKS.iter().map(|b| b.3).collect();
    let valid = needs == block_needs;
    log.push_str(&format!("\nsubmeshes: {}\n", segs.len()));
    log.push_str(&format!("verts per submesh (from indices): {needs:?}\n"));
    log.push_str(&format!("verts per block  (from layout): {block_needs:?}\n"));
    log.push_str(&format!(
        "agreement: {}\n",
        if valid {
            "EXACT MATCH"
        } else {
            "MISMATCH - treat geometry as unproven"
        }
    ));

    // Each submesh consumes exactly one block, so the base offset is cumulative.
    let mut bases: Vec<usize> = Vec::with_capacity(needs.len());
    let mut acc = 0usize;
    for n in &needs {
        bases.push(acc);
        acc += n;
    }

    let mut indices: Vec<u32> = Vec::new();
    let (mut windows, mut degenerate) = (0usize, 0usize);
    for (si, seg) in segs.iter().enumerate() {
        if bases[si] + *needs.get(si).unwrap_or(&0) > positions.len() {
            break;
        }
        let base = bases[si] as u32;
        if seg.len() < 3 {
            continue;
        }
        for j in 0..seg.len() - 2 {
            windows += 1;
            let (a, b, c) = (seg[j] as u32, seg[j + 1] as u32, seg[j + 2] as u32);
            // a strip flips winding every other triangle
            let (a, b) = if j % 2 == 1 { (b, a) } else { (a, b) };
            if a == b || b == c || a == c {
                degenerate += 1;
                continue;
            }
            if base + c as u32 >= positions.len() as u32 {
                continue;
            }
            indices.push(base + a);
            indices.push(base + b);
            indices.push(base + c);
        }
    }
    log.push_str(&format!(
        "strip windows={windows} degenerate={degenerate} triangles={}\n",
        indices.len() / 3
    ));

    // bbox over only the vertices the indices actually touch
    let mut used = vec![false; positions.len()];
    for &ix in &indices {
        used[ix as usize] = true;
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    let (mut nused, mut nunused) = (0usize, 0usize);
    for (k, p) in positions.iter().enumerate() {
        if !used[k] {
            nunused += 1;
            continue;
        }
        nused += 1;
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    log.push_str(&format!("verts referenced={nused} unreferenced={nunused}\n"));
    log.push_str(&format!(
        "bbox min=({:.3},{:.3},{:.3}) max=({:.3},{:.3},{:.3})\n",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    ));
    log.push_str(&format!(
        "size=({:.3},{:.3},{:.3})\n",
        hi[0] - lo[0],
        hi[1] - lo[1],
        hi[2] - lo[2]
    ));
    log.push_str("block 5 (583 head verts) NOT decoded: nothing indexes it\n");

    Decoded {
        positions,
        normals,
        indices,
        log,
        valid,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: game-port <path-to-character-pak>");
        std::process::exit(2);
    }
    let d = decode(&args[1]);

    if std::fs::create_dir_all("logs").is_ok() {
        if let Ok(mut out) = File::create("logs/model_view.log") {
            out.write_all(d.log.as_bytes()).ok();
        }
    }
    println!("{}", d.log);
    if !d.valid {
        println!("\n*** submesh/block counts disagree: geometry is unproven ***");
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, d.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, d.normals.clone());
    mesh.insert_indices(Indices::U32(d.indices.clone()));

    // Centre horizontally, feet on y=0, scale to ~2 units tall.
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &d.positions {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let cx = (lo[0] + hi[0]) * 0.5;
    let y0 = lo[1];
    let cz = (lo[2] + hi[2]) * 0.5;
    let h = (hi[1] - lo[1]).max(0.001);
    println!("model height {h:.3} game units; scaled to 2 units for the scene");

    // Handles are resolved here rather than inside a system. Inserting into
    // Assets<T> needs ResMut<Assets<T>>, and Bevy 0.15 refuses a system that
    // also holds a read borrow of the same resource, so the asset is added
    // before the app runs instead.
    let mut app = App::new();
    // Plugins must be added before assets can be inserted: App::new() on its own
    // is a bare World with no Assets<T> resources.
    app.add_plugins(DefaultPlugins);
    let body_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(mesh);
    let body_mat = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.12, 0.15),
            perceptual_roughness: 0.6,
            // double-sided: winding parity is unproven, so both faces draw
            cull_mode: None,
            ..default()
        });
    let floor_mat = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.22, 0.26),
            perceptual_roughness: 0.9,
            ..default()
        });

    app.insert_resource(ClearColor(Color::srgb(0.10, 0.10, 0.13)))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 500.0,
            ..default()
        })
        .insert_resource(Handles {
            body_mesh,
            body_mat,
            floor_mat,
        })
        .insert_resource(Scale {
            s: 2.0 / h,
            cx,
            y0,
            cz,
        })
        .insert_resource(Orbit {
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.2,
            dist: 5.0,
            centre: Vec3::new(0.0, 1.0, 0.0),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, orbit)
        .run();
}

#[derive(Resource)]
struct Handles {
    body_mesh: Handle<Mesh>,
    body_mat: Handle<StandardMaterial>,
    floor_mat: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct Scale {
    s: f32,
    cx: f32,
    y0: f32,
    cz: f32,
}

#[derive(Resource)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    dist: f32,
    centre: Vec3,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    handles: Res<Handles>,
    scale: Res<Scale>,
) {
    commands.spawn((
        Mesh3d(handles.body_mesh.clone()),
        MeshMaterial3d(handles.body_mat.clone()),
        Transform::from_xyz(-scale.cx * scale.s, -scale.y0 * scale.s, -scale.cz * scale.s)
            .with_scale(Vec3::splat(scale.s)),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(14.0, 14.0))),
        MeshMaterial3d(handles.floor_mat.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.spawn(DirectionalLight {
        illuminance: 6000.0,
        ..default()
    });
    commands.spawn(Camera3dBundle::default());
}

fn orbit(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut o: ResMut<Orbit>,
    mut cam: Query<&mut Transform, With<Camera>>,
) {
    let dt = time.delta_secs();
    let spin = keys.pressed(KeyCode::KeyA);
    if spin {
        o.yaw += dt * 0.7;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        o.yaw += dt * 1.2;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        o.yaw -= dt * 1.2;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        o.pitch = (o.pitch + dt * 0.8).clamp(-1.4, 1.4);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        o.pitch = (o.pitch - dt * 0.8).clamp(-1.4, 1.4);
    }
    if keys.pressed(KeyCode::KeyQ) {
        o.dist = (o.dist + dt * 3.0).clamp(1.2, 30.0);
    }
    if keys.pressed(KeyCode::KeyE) {
        o.dist = (o.dist - dt * 3.0).clamp(1.2, 30.0);
    }
    for mut t in &mut cam {
        let (sy, cy) = o.yaw.sin_cos();
        let (sp, cp) = o.pitch.sin_cos();
        t.translation = o.centre + Vec3::new(cy * cp, sp, sy * cp) * o.dist;
        t.look_at(o.centre, Vec3::Y);
    }
}