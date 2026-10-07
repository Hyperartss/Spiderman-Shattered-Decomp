// Probe: find the real vertex layout for the LEVEL chunk.
//
// Symptom being explained: the level renders as a pyramid / weird ramp rather
// than a level. That is what misaligned vertex records look like.
//
// The level has been decoded at stride 32. The character model was proven to
// be 68. No stride has been tested exhaustively against normal OFFSET too --
// the normal may not be at +12 in this chunk.
//
// Strategy, in order of strength:
//   1. 0xAA padding runs give exact block boundaries (this is what solved the
//      character model). Report them.
//   2. For each block, score every (stride, pos_offset, nrm_offset) on
//      fraction finite + fraction unit-length normals.
//   3. Report the best global layout and whether any hits 100%.
//
// Read-only on Game A. Writes only logs/.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

fn rd(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: probe <level_vertex_bin>");
        std::process::exit(2);
    }
    let path = &args[1];
    let mut f = File::open(path).expect("open vertex bin");
    let mut d: Vec<u8> = Vec::new();
    f.read_to_end(&mut d).expect("read vertex bin");
    let mut log = String::new();
    log.push_str(&format!("file={}\nlen={}\n\n", Path::new(path).file_name().unwrap().to_string_lossy(), d.len()));

    // ---- 1. padding runs ----
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < d.len() {
        if d[k] == 0xaa {
            let s = k;
            while k < d.len() && d[k] == 0xaa {
                k += 1;
            }
            runs.push((s, k - s));
        } else {
            k += 1;
        }
    }
    let big: Vec<&(usize, usize)> = runs.iter().filter(|r| r.1 >= 8).collect();
    log.push_str(&format!(
        "0xAA runs: {} total, {} of >=8 bytes -> implies {} blocks\n",
        runs.len(),
        big.len(),
        big.len() + 1
    ));
    log.push_str("first 20 padding runs >=8 bytes (offset, len):\n");
    for r in big.iter().take(20) {
        log.push_str(&format!("  {:>9} {:>5}\n", r.0, r.1));
    }

    // ---- 2. exhaustive (stride, pos_off, nrm_off) scan ----
    let mut best: Vec<(f64, usize, usize, usize, usize, usize)> = Vec::new(); // score, stride, po, no, finite, unit
    for stride in [24usize, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72] {
        for po in [0usize, 4, 8, 12, 16] {
            for no in [12usize, 16, 20, 24] {
                if no <= po + 8 {
                    continue;
                }
                if stride < no + 12 {
                    continue;
                }
                let n = d.len() / stride;
                if n < 100 {
                    continue;
                }
                let (mut finite, mut unit) = (0usize, 0usize);
                for r in 0..n {
                    let o = r * stride + po;
                    if o + 24 > d.len() {
                        break;
                    }
                    let p = [rd(&d, o), rd(&d, o + 4), rd(&d, o + 8)];
                    let q = [rd(&d, r * stride + no), rd(&d, r * stride + no + 4), rd(&d, r * stride + no + 8)];
                    if p.iter().chain(q.iter()).any(|c| !c.is_finite()) {
                        continue;
                    }
                    finite += 1;
                    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
                    if (l - 1.0).abs() < 0.05 {
                        unit += 1;
                    }
                }
                let score = unit as f64 / n as f64;
                best.push((score, stride, po, no, finite, unit));
            }
        }
    }
    best.sort_by(|a, b| b.4.cmp(&a.4).then(a.0.partial_cmp(&b.0).unwrap()));
    // report top by finite count (the real signal), then by unit fraction
    log.push_str(&format!("\ntop 20 layouts by FINITE record count (n = {} records):\n", d.len() / 32));
    log.push_str("   unit%    stride  pos_off  nrm_off  finite      unit\n");
    for b in best.iter().take(20) {
        log.push_str(&format!(
            "  {:>6.1}%  {:>6}  {:>7}  {:>7}  {:>7}  {:>7}\n",
            100.0 * b.0,
            b.1,
            b.2,
            b.3,
            b.4,
            b.5
        ));
    }
    // best unit fraction overall
    let mut byunit = best.clone();
    byunit.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().reverse());
    log.push_str("\ntop 10 layouts by UNIT-NORMAL fraction:\n");
    for b in byunit.iter().take(10) {
        log.push_str(&format!(
            "  unit={:>6.1}%  stride={:>3} po={:>2} no={:>2}  finite={} unit={}\n",
            100.0 * b.0, b.1, b.2, b.3, b.4, b.5
        ));
    }

    // ---- 3. the specific check: is the level consistent with 68? ----
    log.push_str("\nspecific check, whole-file at stride 68 (model's proven stride):\n");
    let n68 = d.len() / 68;
    let (mut f68, mut u68) = (0usize, 0usize);
    for r in 0..n68 {
        let o = r * 68;
        let p = [rd(&d, o), rd(&d, o + 4), rd(&d, o + 8)];
        let q = [rd(&d, o + 12), rd(&d, o + 16), rd(&d, o + 20)];
        if p.iter().chain(q.iter()).any(|c| !c.is_finite()) {
            continue;
        }
        f68 += 1;
        let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
        if (l - 1.0).abs() < 0.05 {
            u68 += 1;
        }
    }
    log.push_str(&format!("  stride68: records={} finite={} unit={}\n", n68, f68, u68));
    log.push_str(&format!("  380032/68 = {:.2}, 8100832/68 = {}\n", 380032.0 / 68.0, d.len() / 68));

    let mut out = File::create("logs/level_layout_scan.log").unwrap();
    out.write_all(log.as_bytes()).ok();
    print!("{}", log);
}