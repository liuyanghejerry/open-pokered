use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // Generate CJK glyph data from Fusion Pixel BDF fonts
    generate_cjk_glyphs(&manifest_dir, &out_dir);
}

fn generate_cjk_glyphs(manifest_dir: &str, out_dir: &str) {
    let fonts_dir = Path::new(manifest_dir).join("fonts");
    let latin_path = fonts_dir.join("fusion-pixel-10px-monospaced-latin.bdf");
    let zh_path = fonts_dir.join("fusion-pixel-10px-monospaced-zh_hans.bdf");

    // Only re-run when the build script or the source fonts change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", latin_path.display());
    println!("cargo:rerun-if-changed={}", zh_path.display());

    let latin = parse_bdf(&latin_path);
    let zh = parse_bdf(&zh_path);

    // ASCII (0x20..=0x7E) comes from the Latin face (5px half-width). Every other
    // glyph the zh_hans face provides is baked, giving full CJK coverage (~24k chars).
    let mut all: Vec<(char, GlyphData)> = Vec::new();
    let mut seen: std::collections::HashSet<char> = std::collections::HashSet::new();
    for ch in ' '..='~' {
        if let Some(g) = latin.get(&ch) {
            all.push((ch, g.clone()));
            seen.insert(ch);
        }
    }
    for (ch, g) in &zh {
        if seen.insert(*ch) {
            all.push((*ch, g.clone()));
        }
    }
    all.sort_by_key(|(ch, _)| *ch);

    // Serialize to a compact binary blob, parsed at runtime by embedded_font.rs.
    // A ~24k-entry Rust array literal would balloon compile time, so we emit bytes
    // plus a codepoint-sorted index for binary search.
    //
    // Layout (all little-endian):
    //   u32 count
    //   count × { u32 codepoint, u32 data_offset }   (sorted ascending by codepoint)
    //   per glyph @ data_offset:
    //     u8 width, u8 nrows, i16 x_off, i16 y_off, u8 advance, (u16 × nrows) rows
    let count = all.len() as u32;
    let data_start = 4 + all.len() * 8;
    let mut index = Vec::with_capacity(all.len() * 8);
    let mut data = Vec::new();
    for (ch, g) in &all {
        let off = (data_start + data.len()) as u32;
        index.extend_from_slice(&(*ch as u32).to_le_bytes());
        index.extend_from_slice(&off.to_le_bytes());

        let nrows = g.rows.len().min(255) as u8;
        data.push(g.w.min(255) as u8);
        data.push(nrows);
        data.extend_from_slice(&(g.xo as i16).to_le_bytes());
        data.extend_from_slice(&(g.yo as i16).to_le_bytes());
        data.push(g.adv.min(255) as u8);
        for &r in g.rows.iter().take(nrows as usize) {
            data.extend_from_slice(&r.to_le_bytes());
        }
    }

    let mut blob = Vec::with_capacity(4 + index.len() + data.len());
    blob.extend_from_slice(&count.to_le_bytes());
    blob.extend_from_slice(&index);
    blob.extend_from_slice(&data);

    let dest = Path::new(out_dir).join("glyphs.bin");
    fs::write(&dest, &blob).unwrap();

    println!(
        "cargo:warning=CJK glyphs baked: {} chars ({} KiB blob)",
        all.len(),
        blob.len() / 1024
    );
}

#[derive(Debug, Clone)]
struct GlyphData {
    w: u32,
    xo: i32,
    yo: i32,
    adv: u32,
    rows: Vec<u16>,
}

fn parse_bdf(path: &Path) -> std::collections::HashMap<char, GlyphData> {
    let mut map = std::collections::HashMap::new();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            println!("cargo:warning=Failed to read BDF {:?}: {}", path, e);
            return map;
        }
    };

    let blocks: Vec<&str> = content.split("STARTCHAR ").collect();
    for block in &blocks[1..] {
        let mut encoding: Option<u32> = None;
        let mut dwidth = 0u32;
        let mut bbx_w = 0u32;
        let mut bbx_xo = 0i32;
        let mut bbx_yo = 0i32;
        let mut in_bitmap = false;
        let mut bitmap_lines: Vec<String> = Vec::new();

        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("ENCODING ") {
                encoding = line[9..].trim().parse().ok();
            } else if line.starts_with("DWIDTH ") {
                let parts: Vec<&str> = line[7..].split_whitespace().collect();
                dwidth = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            } else if line.starts_with("BBX ") {
                let parts: Vec<&str> = line[4..].split_whitespace().collect();
                if parts.len() >= 4 {
                    bbx_w = parts[0].parse().unwrap_or(0);
                    bbx_xo = parts[2].parse().unwrap_or(0);
                    bbx_yo = parts[3].parse().unwrap_or(0);
                }
            } else if line == "BITMAP" {
                in_bitmap = true;
            } else if line == "ENDCHAR" {
                break;
            } else if in_bitmap {
                bitmap_lines.push(line.to_string());
            }
        }

        if let Some(cp) = encoding {
            if let Some(ch) = char::from_u32(cp) {
                let mut rows = Vec::new();
                for hl in &bitmap_lines {
                    let val = u32::from_str_radix(hl, 16).unwrap_or(0);
                    let total_bits = hl.len() * 4;
                    let normalized = if total_bits > bbx_w as usize {
                        val >> (total_bits - bbx_w as usize)
                    } else {
                        val
                    };
                    rows.push(normalized as u16);
                }
                map.insert(
                    ch,
                    GlyphData {
                        w: bbx_w,
                        xo: bbx_xo,
                        yo: bbx_yo,
                        adv: dwidth,
                        rows,
                    },
                );
            }
        }
    }
    map
}
