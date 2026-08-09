use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_embedded_target = target_arch == "wasm32" || target_os == "android" || target_os == "ios";

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    // gfx/ lives at gfx (sibling of crates/); this crate is at
    // crates/pokered-renderer, so two levels up.
    let gfx_dir = Path::new(&manifest_dir).join("../../gfx");
    let gfx_dir = gfx_dir.canonicalize().unwrap_or_else(|_| gfx_dir);

    if gfx_dir.exists() {
        println!("cargo:rerun-if-changed={}", gfx_dir.display());
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("embedded_assets.rs");
    let mut file = BufWriter::new(File::create(&dest_path).unwrap());

    if is_embedded_target && gfx_dir.exists() {
        generate_embedded_assets(&mut file, &gfx_dir);
    } else {
        generate_empty_stub(&mut file);
    }

    file.flush().unwrap();

    // ── Generate CJK glyph data from Fusion Pixel BDF fonts ──
    generate_cjk_glyphs(&manifest_dir, &out_dir);
}

fn generate_embedded_assets<W: Write>(mut writer: W, gfx_dir: &Path) {
    writeln!(writer, "use std::collections::HashMap;").unwrap();
    writeln!(writer, "use std::sync::OnceLock;").unwrap();
    writeln!(writer).unwrap();

    let mut assets: Vec<String> = Vec::new();
    collect_embed_files(gfx_dir, gfx_dir, &mut assets);
    assets.sort();

    if assets.is_empty() {
        writeln!(
            writer,
            "pub fn get_embedded_asset(_relative_path: &str) -> Option<&'static [u8]> {{"
        )
        .unwrap();
        writeln!(writer, "    None").unwrap();
        writeln!(writer, "}}").unwrap();
        writeln!(writer).unwrap();
        writeln!(
            writer,
            "pub fn list_embedded_assets() -> &'static [&'static str] {{"
        )
        .unwrap();
        writeln!(writer, "    &[]").unwrap();
        writeln!(writer, "}}").unwrap();
        return;
    }

    for (i, rel_path) in assets.iter().enumerate() {
        writeln!(
            writer,
            "static ASSET_{}: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../gfx/{}\"));",
            i,
            rel_path.replace('\\', "/")
        )
        .unwrap();
    }

    writeln!(writer).unwrap();

    writeln!(
        writer,
        "pub fn get_embedded_asset(relative_path: &str) -> Option<&'static [u8]> {{"
    )
    .unwrap();
    writeln!(
        writer,
        "    let normalized = relative_path.replace('\\\\', \"/\").to_lowercase();"
    )
    .unwrap();
    writeln!(writer).unwrap();
    writeln!(
        writer,
        "    static ASSETS: OnceLock<HashMap<&'static str, &'static [u8]>> = OnceLock::new();"
    )
    .unwrap();
    writeln!(writer, "    let map = ASSETS.get_or_init(|| {{").unwrap();
    writeln!(writer, "        let mut m = HashMap::new();").unwrap();

    for (i, rel_path) in assets.iter().enumerate() {
        writeln!(
            writer,
            "        m.insert(\"{}\", ASSET_{} as &'static [u8]);",
            rel_path.to_lowercase(),
            i
        )
        .unwrap();
    }

    writeln!(writer, "        m").unwrap();
    writeln!(writer, "    }});").unwrap();
    writeln!(writer).unwrap();
    writeln!(writer, "    map.get(normalized.as_str()).copied()").unwrap();
    writeln!(writer, "}}").unwrap();

    writeln!(writer).unwrap();
    writeln!(
        writer,
        "pub fn list_embedded_assets() -> &'static [&'static str] {{"
    )
    .unwrap();
    writeln!(writer, "    static PATHS: &[&str] = &[").unwrap();

    for rel_path in &assets {
        writeln!(writer, "        \"{}\",", rel_path).unwrap();
    }

    writeln!(writer, "    ];").unwrap();
    writeln!(writer, "    PATHS").unwrap();
    writeln!(writer, "}}").unwrap();
}

fn generate_empty_stub<W: Write>(mut writer: W) {
    writeln!(
        writer,
        "pub fn get_embedded_asset(_relative_path: &str) -> Option<&'static [u8]> {{"
    )
    .unwrap();
    writeln!(writer, "    None").unwrap();
    writeln!(writer, "}}").unwrap();
    writeln!(writer).unwrap();
    writeln!(
        writer,
        "pub fn list_embedded_assets() -> &'static [&'static str] {{"
    )
    .unwrap();
    writeln!(writer, "    &[]").unwrap();
    writeln!(writer, "}}").unwrap();
}

fn collect_embed_files(base_dir: &Path, current_dir: &Path, assets: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_embed_files(base_dir, &path, assets);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e, "png" | "tilemap"))
                .unwrap_or(false)
            {
                if let Ok(rel_path) = path.strip_prefix(base_dir) {
                    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
                    assets.push(rel_path_str);
                }
            }
        }
    }
}

// ── CJK Glyph Generation ──────────────────────────────────────────

fn generate_cjk_glyphs(manifest_dir: &str, out_dir: &str) {
    let fonts_dir = Path::new(manifest_dir).join("fonts");
    let latin_path = fonts_dir.join("fusion-pixel-10px-monospaced-latin.bdf");
    let zh_path = fonts_dir.join("fusion-pixel-10px-monospaced-zh_hans.bdf");

    let latin = parse_bdf(&latin_path);
    let zh = parse_bdf(&zh_path);

    let ascii: Vec<char> = (' '..='~').collect();
    let cjk: Vec<char> = "你好！欢迎来到宝可梦的世界！我是大木博士，大家都叫我宝可梦博士。▼，。\
        选择语言中文这个世界生活着一种叫做的神奇生物对一些人来说是宠物另会用它们来对战而我则以研究为职业\
        首先请问你叫什么名字这是我的孙子从婴儿时就竞争对手了呃他着来传说就要开始充满想与冒险正等出发吧\
        妙蛙种子草花小火恐龙喷杰尼龟卡咪水箭绿毛虫铁甲蛹巴蝶独角壳针蜂波比鸟大拉达烈雀嘴阿柏蛇怪\
        皮丘雷穿山鼠王尼多兰娜后朗力诺皮可西六尾九胖丁超音蝠走路臭霸王派斯特毛球末入蛾地三喵老\
        可鸭哥猴火爆狗风速蚊香蝌蚪君泳士凯勇基胡腕豪力喇叭芽口呆食玛瑙母毒刺拳隆隆岩马烈焰呆壳\
        兽磁合葱嘟嘟利海狮白泥贝鬼斯通耿蛇催眠貘引梦钳蟹巨霹雳电球顽弹蛋椰树卡拉嘎腿郎快拳舌\
        瓦双铁甲犀暴吉利蔓藤袋墨龙鱼星宝石魔墙偶飞螳迷唇姐击鸭嘴罗肯泰暴鲤拉斯百变布伊多边菊\
        刺盔镰刀翼急冻闪电焰迷你哈超梦幻炎冰虫幽灵钢念斗飞地格斗毒岩鸟一般拍击空劈连环巴掌拳\
        居斩起摇绑紧摔藤鞭踩踏踢撞泰山压顶闹番舍冲尾针乱弹瞪咬住叫吼唱音爆定身法溶解液喷射雾\
        枪炮冲浪束冻暴幻彩极坏啄翻滚倒倍奉还球取级寄生种刀毒麻菇瓣吐丝怒旋涡电十万伏特磁落震\
        裂挖洞剧念强催眠术姿高速闪愤瞬移夜魔影仿刺耳声子分身再生硬变烟幕奇缩壳圆屏障光墙黑反\
        聚忍挥指鹦学舌舔浊污攻棒炎瀑夹星弹炮缠忆折弯匙生膝瞪食梦瓦投命吸吻击波昏孢棱角化纹理\
        三门牙替挣扎神鸟"
        .chars()
        .collect();

    let mut all: Vec<(char, GlyphData)> = Vec::new();
    for ch in &ascii {
        if let Some(g) = latin.get(ch) {
            all.push((*ch, g.clone()));
        }
    }
    for ch in &cjk {
        if let Some(g) = zh.get(ch) {
            // Avoid duplicates (some punctuation may be in both)
            if !all.iter().any(|(c, _)| c == ch) {
                all.push((*ch, g.clone()));
            }
        }
    }
    all.sort_by_key(|(ch, _)| *ch);

    let dest = Path::new(out_dir).join("cjk_glyphs.rs");
    let mut f = BufWriter::new(File::create(&dest).unwrap());

    writeln!(f, "&[").unwrap();
    for (ch, g) in &all {
        let rows_str = g
            .rows
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            f,
            "    ({}u32, {}u32, {}u32, {}i32, {}i32, {}u32, &[{}u16]),",
            *ch as u32, g.w, g.h, g.xo, g.yo, g.adv, rows_str
        )
        .unwrap();
    }
    writeln!(f, "]").unwrap();
    f.flush().unwrap();

    println!(
        "cargo:warning=CJK glyphs generated: {} chars ({} ASCII + {} CJK)",
        all.len(),
        ascii.len(),
        cjk.len()
    );
}

#[derive(Debug, Clone)]
struct GlyphData {
    w: u32,
    h: u32,
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
        let mut bbx_h = 0u32;
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
                    bbx_h = parts[1].parse().unwrap_or(0);
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
                        h: bbx_h,
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
