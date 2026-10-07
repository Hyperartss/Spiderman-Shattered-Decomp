// Model viewer — renders the character model in a Bevy window.
//
// Usage: spider-shattered <path-to-character-pak>
//
// The pak path is an argument so no game filename is baked into tracked source.
//
// Proven layout this rebuilds (see STATUS.md):
//   vertex chunk 0x1388, index chunk 0x1389, paired under 1 > 0x26 > 0x325
//   vertex blocks, each boundary landing on a 0xAA pad run:
//     block 1  0      .. 832    stride 32   26 verts
//     block 2  832    .. 4352   stride 32  110 verts
//     block 3  4352   .. 41412  stride 68  545 verts
//     block 4  41440  .. 340368 stride 68 4396 verts
//     block 5  340384 .. 380028 stride 68  583 verts  <- head, NO index data
//   position f32[3] at +0, normal f32[3] at +12.
//   index: u16, 0xAAAA separates submeshes, indices local per submesh.
//   body is a triangle strip with degenerate bridges: walk ONE index per
//   triangle, skip windows where two indices match.
//
// Block 5 is omitted because nothing indexes it — that is the holed face.
// Read-only on Game A. Writes only logs/.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// (start, end, stride) over the vertex buffer.
const BLOCKS: [(usize, usize, usize); 4] = [
    (0, 832, 32),
    (832, 4352, 32),
    (4352, 41412, 68),
    (41440, 340368, 68),
];

/// Vert count per block; the index submeshes need exactly these.
const BLOCK_VERTS: [usize; 4] = [26, 110, 545, 4396];

struct Chunk {
    off: u64,
    len: u64,
    typ: u32,
    parent: u64,
}

fn walk(f: &mut File, start: u64, clen: u64, depth: u32, out: &mut Vec<Chunk>) {
    if depth >= 8 {
        return;
    }
    let payload_end = start + 12 + clen;
    let mut o = start + 12;
    let mut hdr = [0u8; 12];
    while o + 12 <= payload_end {
        if f.seek(SeekFrom::Start(o)).is_err() {
            break;
        }
        if f.read_exact(&mut hdr).is_err() {
            break;
        }
        let t = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let l = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as u64;
        let next = o + 12 + l;
        if l == 0 && t == 0 {
            break;
        }
        if next > payload_end {
            break;
        }
        out.push(Chunk {
            off: o + 12,
            len: l,
            typ: t,
            parent: start,
        });
        walk(f, o, l, depth + 1, out);
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
    submeshes: Vec<usize>,
    windows: usize,
    degenerate: usize,
    log: String,
}

fn decode(pak: &str) -> Decoded {
    let mut log = String::new();
    log.push_str(&format!("pak={}\n", pak));

    let mut f = File::open(pak).expect("cannot open pak (read-only open, check path)");
    let mut hdr = [0u8; 12];
    f.read_exact(&mut hdr).unwrap();
    let root_len = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as u64;

    let mut chunks: Vec<Chunk> = Vec::new();
    walk(&mut f, 0, root_len, 0, &mut chunks);
    log.push_str(&format!("chunks walked: {}\n", chunks.len()));

    // vertex chunk = the 0x1388; index chunk = the 0x1389 under the same parent
    let v = chunks
        .iter()
        .find(|c| c.typ == 0x1388 && c.len == 380032)
        .expect("no 0x1388 of len 380032 — is this the character pak?");
    let i = chunks
        .iter()
        .find(|c| c.typ == 0x1389 && c.parent == v.parent)
        .expect("no sibling 0x1389");
    log.push_str(&format!("vertex chunk off={} len={}\n", v.off, v.len));
    log.push_str(&format!("index  chunk off={} len={}\n", i.off, i.len));

    let mut vbuf = vec![0u8; v.len as usize];
    f.seek(SeekFrom::Start(v.off)).unwrap();
    f.read_exact(&mut vbuf).unwrap();
    let mut ibuf = vec![0u8; i.len as usize];
    f.seek(SeekFrom::Start(i.off)).unwrap();
    f.read_exact(&mut ibuf).unwrap();

    // ---- vertices, block by block ----
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    log.push_str("blocks (unit normals = finite and |n| ~ 1):\n");
    for (bi, &(start, end, stride)) in BLOCKS.iter().enumerate() {
        let n = (end - start) / stride;
        let (mut finite, mut unit) = (0usize, 0usize);
        for k in 0..n {
            let o = start + k * stride;
            let p = [rd(&vbuf, o), rd(&vbuf, o + 4), rd(&vbuf, o + 8)];
            let mut nm = [rd(&vbuf, o + 12), rd(&vbuf, o + 16), rd(&vbuf, o + 20)];
            if p.iter().any(|c| !c.is_finite()) || nm.iter().any(|c| !c.is_finite()) {
                // keep the vertex slot so indices stay aligned, but make it safe
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
            "  block {}: bytes {}..{} stride {}  verts {}  finite {}  unit {}\n",
            bi + 1,
            start,
            end,
            stride,
            n,
            finite,
            unit
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

    let mut submeshes: Vec<usize> = Vec::new();
    for s in &segs {
        submeshes.push((*s.iter().max().unwrap() as usize) + 1);
    }
    log.push_str(&format!("submeshes: {}\n", segs.len()));
    log.push_str(&format!("verts per submesh: {:?}\n", submeshes));
    log.push_str(&format!(
        "expected from block layout: {:?}\n",
        BLOCK_VERTS
    ));

    // base vertex offset for each submesh (cumulative)
    let mut bases: Vec<usize> = Vec::with_capacity(submeshes.len());
    let mut acc = 0usize;
    for s in &submeshes {
        bases.push(acc);
        acc += s;
    }

    let mut indices: Vec<u32> = Vec::new();
    let (mut windows, mut degenerate) = (0usize, 0usize);
    for (si, seg) in segs.iter().enumerate() {
        let base = bases[si] as u32;
        if seg.len() < 3 {
            continue;
        }
        for j in 0..seg.len() - 2 {
            windows += 1;
            let (a, b, c) = (seg[j] as u32, seg[j + 1] as u32, seg[j + 2] as u32);
            // alternating winding: strips flip every other triangle
            let (a, b) = if j % 2 == 1 { (b, a) } else { (a, b) };
            if a == b || b == c || a == c {
                degenerate += 1;
                continue;
            }
            indices.push(base + a);
            indices.push(base + b);
            indices.push(base + c);
        }
    }
    log.push_str(&format!(
        "strip windows={} degenerate={} triangles={}\n",
        windows,
        degenerate,
        indices.len() / 3
    ));

    // bbox over the verts the indices actually touch
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
    log.push_str(&format!(
        "verts referenced={} unreferenced={}\n",
        nused, nunused
    ));
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
    log.push_str("block 5 (583 head verts) NOT decoded: no index data references it\n");

    Decoded {
        positions,
        normals,
        indices,
        submeshes,
        windows,
        degenerate,
        log,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: spider-shattered <path-to-character-pak>");
        std::process::exit(2);
    }
    let pak = args[1].clone();

    let d = decode(&pak);

    let mut out = File::create("logs/model_view.log").unwrap();
    out.write_all(d.log.as_bytes()).ok();
    println!(
        "submeshes={:?} strip_windows={} degenerate={} triangles={}",
        d.submeshes,
        d.windows,
        d.degenerate,
        d.indices.len() / 3
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, d.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, d.normals.clone());
    mesh.insert_indices(Indices::U32(d.indices.clone()));

    // centre the model horizontally, feet at y=0
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &d.positions {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let cx = (lo[0] + hi[0]) * 0.5;
    let cz = (lo[2] + hi[2]) * 0.5;
    let y0 = lo[1];
    let h = (hi[1] - lo[1]).max(0.001);
    println!(
        "model height {:.3} units, feet dropped to y=0; camera looks at (0, {:.3}, 0)",
        h,
        h * 0.5
    );

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.10, 0.10, 0.13)))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(PendingModel {
            cx,
            y0,
            cz,
            h,
            mesh: Some(mesh),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Character model".into(),
                resolution: (1280.0f32, 720.0f32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, (setup_camera, spawn_model))
        .run();
}

#[derive(Resource)]
struct PendingModel {
    cx: f32,
    y0: f32,
    cz: f32,
    h: f32,
    mesh: Option<Mesh>,
}

fn setup_camera(model: Res<PendingModel>, mut commands: Commands) {
    let h = model.h;
    let mut cam_tf = Transform::from_xyz(h * 1.1, h * 0.7, h * 1.8);
    cam_tf.look_at(Vec3::new(0.0, h * 0.5, 0.0), Vec3::Y);
    commands.spawn((Camera3d::default(), cam_tf));
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.6, 0.0)),
    ));
}

fn spawn_model(
    mut commands: Commands,
    mut pending: ResMut<PendingModel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mesh) = pending.mesh.take() else {
        return;
    };
    let handle = meshes.add(mesh);
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.15, 0.15),
        cull_mode: None, // winding parity is unproven, so render both sides
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Mesh3d(handle),
        MeshMaterial3d(mat),
        Transform::from_xyz(-pending.cx, -pending.y0, -pending.cz),
    ));
}
