//! Text metrics shared by pagination and raster drawing.
//!
//! Original English charmap glyphs occupy 8px cells; other glyphs retain
//! Fusion Pixel's metrics (Chinese 10px). Language does not change a glyph's
//! width. Authored newlines are hard breaks, as in the ROM's text controls.

use crate::alloc_prelude::*;

pub const DIALOGUE_LINE_WIDTH_PX: usize = 18 * 8;

/// The English charmap in constants/charmap.asm. FontGraphics is copied to
/// vFont (tile $80), while TextBoxGraphics is copied to tile $60.
pub fn tile_for_char(ch: char) -> Option<u8> {
    Some(match ch {
        'A'..='Z' => 0x80 + (ch as u8 - b'A'),
        'a'..='z' => 0xA0 + (ch as u8 - b'a'),
        '0'..='9' => 0xF6 + (ch as u8 - b'0'),
        ' ' => 0x7F,
        '(' => 0x9A,
        ')' => 0x9B,
        ':' => 0x9C,
        ';' => 0x9D,
        '[' => 0x9E,
        ']' => 0x9F,
        'é' => 0xBA,
        '\'' => 0xE0,
        '-' => 0xE3,
        '?' => 0xE6,
        '!' => 0xE7,
        '.' => 0xE8,
        '▷' => 0xEC,
        '▶' | '>' => 0xED,
        '▼' => 0xEE,
        '♂' => 0xEF,
        '¥' | '$' => 0xF0,
        '×' => 0xF1,
        '/' => 0xF3,
        ',' => 0xF4,
        '♀' => 0xF5,
        '‘' => 0x70,
        '’' => 0x71,
        '“' => 0x72,
        '”' => 0x73,
        '·' | '№' => 0x74,
        '…' => 0x75,
        '┌' => 0x79,
        '─' => 0x7A,
        '┐' => 0x7B,
        '│' => 0x7C,
        '└' => 0x7D,
        '┘' => 0x7E,
        _ => return None,
    })
}

pub fn char_advance(ch: char) -> u32 {
    if tile_for_char(ch).is_some() || (ch.is_ascii() && !ch.is_ascii_control()) {
        8
    } else {
        dotzuki_renderer::embedded_font::char_advance(ch)
    }
}

pub fn measure_text(text: &str) -> u32 {
    text.chars().map(char_advance).sum()
}
/// Wrap every authored row without joining it to the next row or dropping
/// overflow. Callers paginate the result in pairs before typewriter reveal.
/// The punctuation/word wrapping helpers are adapted from dotzuki-ui v0.8.2
/// widgets/dialog.rs, using the game's actual glyph metrics.
pub fn wrap_hard_lines(text: &str, max_width_px: usize) -> Vec<String> {
    if max_width_px == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for row in text.split('\n') {
        let wrapped = if row.chars().any(is_cjk_char) {
            wrap_cjk_paragraph(row, max_width_px)
        } else {
            wrap_latin_paragraph(row, max_width_px)
        };
        if wrapped.is_empty() {
            out.push(String::new());
        } else {
            out.extend(wrapped);
        }
    }
    out
}

/// True for CJK ideographs, kana, hangul, full-width forms and CJK
/// punctuation — characters that wrap with full-width (10px) metrics.
fn is_cjk_char(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x9FFF   // CJK radicals, punctuation, kana, unified ideographs
        | 0xAC00..=0xD7AF // Hangul syllables
        | 0xF900..=0xFAFF // CJK compatibility ideographs
        | 0xFE30..=0xFE4F // CJK compatibility forms
        | 0xFF00..=0xFFEF // full-width forms
    )
}

/// A single character, an unbreakable ASCII word/number run, or an
/// unbreakable run of closing punctuation (……, ！！) .
#[derive(Debug, Clone)]
enum WrapUnit {
    Ch(char),
    Word(String),
    CloseRun(String),
}

fn unit_width(u: &WrapUnit) -> usize {
    match u {
        WrapUnit::Ch(c) => char_advance(*c) as usize,
        WrapUnit::Word(w) | WrapUnit::CloseRun(w) => measure_text(w) as usize,
    }
}

fn unit_append(out: &mut String, u: &WrapUnit) {
    match u {
        WrapUnit::Ch(c) => out.push(*c),
        WrapUnit::Word(w) | WrapUnit::CloseRun(w) => out.push_str(w),
    }
}

/// Materializes the current line (trimming leading/trailing spaces).
fn units_to_string(units: &[WrapUnit]) -> String {
    let mut s = String::new();
    for u in units {
        unit_append(&mut s, u);
    }
    s.trim().to_string()
}

/// Split one authored row into wrap units. Consecutive closing punctuation
/// stays together where the box width allows it.
fn cjk_units(paragraph: &str) -> Vec<WrapUnit> {
    let mut units: Vec<WrapUnit> = Vec::new();
    let mut word = String::new();
    let mut chars = paragraph.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ' ' {
            if !word.is_empty() {
                units.push(WrapUnit::Word(core::mem::take(&mut word)));
            }
            units.push(WrapUnit::Ch(' '));
        } else if ch.is_ascii_alphanumeric() {
            word.push(ch);
        } else {
            if !word.is_empty() {
                units.push(WrapUnit::Word(core::mem::take(&mut word)));
            }
            if is_closing_punct(ch) {
                match units.last_mut() {
                    Some(WrapUnit::CloseRun(run)) => run.push(ch),
                    _ => units.push(WrapUnit::CloseRun(ch.to_string())),
                }
            } else {
                units.push(WrapUnit::Ch(ch));
            }
        }
    }
    if !word.is_empty() {
        units.push(WrapUnit::Word(word));
    }
    units
}

/// Closing punctuation that must not open a line.
fn is_closing_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '…'
            | '—'
            | '～'
            | '」'
            | '』'
            | '）'
            | '】'
            | '〉'
            | '》'
            | '”'
            | '’'
    )
}

/// Opening brackets that must not end a line.
fn is_opening_punct(c: char) -> bool {
    matches!(c, '「' | '『' | '（' | '【' | '〈' | '《' | '“' | '‘')
}

/// Greedy CJK fill for one authored row.
fn wrap_cjk_paragraph(paragraph: &str, max_width_px: usize) -> Vec<String> {
    let units = cjk_units(paragraph);
    let mut lines: Vec<String> = Vec::new();
    let mut line: Vec<WrapUnit> = Vec::new();
    let mut line_px: usize = 0;

    let mut i = 0;
    while i < units.len() {
        let unit = &units[i];
        let w = unit_width(unit);

        // Never open a line with a space.
        if line.is_empty() && matches!(unit, WrapUnit::Ch(' ')) {
            i += 1;
            continue;
        }

        if line.is_empty() && w > max_width_px {
            let mut text = String::new();
            unit_append(&mut text, unit);
            let mut pieces = split_by_pixels(&text, max_width_px).into_iter().peekable();
            while let Some(piece) = pieces.next() {
                if pieces.peek().is_some() {
                    lines.push(piece);
                } else {
                    line_px = measure_text(&piece) as usize;
                    line.push(WrapUnit::Word(piece));
                }
            }
            i += 1;
            continue;
        }

        if line.is_empty() || line_px + w <= max_width_px {
            line.push(unit.clone());
            line_px += w;
            i += 1;
            continue;
        }

        // ── overflow: 禁则处理 (kinsoku shori) ──
        // Closing punctuation must not open a line: pull the last unit of
        // the current line down so the run follows it on the next line
        // (追い込み), preserving character order. A word/run wider than the
        // box is split by pixels so it cannot overwrite the border.
        if let WrapUnit::CloseRun(_) = unit {
            // Drop trailing spaces — they would be trimmed at display anyway.
            while matches!(line.last(), Some(WrapUnit::Ch(' '))) {
                let u = line.pop().unwrap();
                line_px = line_px.saturating_sub(unit_width(&u));
            }
            if line.len() >= 2 && unit_width(line.last().unwrap()) + w <= max_width_px {
                let pulled = line.pop().unwrap();
                lines.push(units_to_string(&line));
                line.clear();
                line.push(pulled);
                line.push(unit.clone());
                line_px = line.iter().map(unit_width).sum();
            } else {
                line.push(unit.clone());
                let combined = units_to_string(&line);
                let mut pieces = split_by_pixels(&combined, max_width_px)
                    .into_iter()
                    .peekable();
                line.clear();
                line_px = 0;
                while let Some(piece) = pieces.next() {
                    if pieces.peek().is_some() {
                        lines.push(piece);
                    } else {
                        line_px = measure_text(&piece) as usize;
                        line.push(WrapUnit::Word(piece));
                    }
                }
            }
            i += 1;
            continue;
        }
        // Opening brackets must not end a line: roll the bracket onto the
        // next line, where it binds to the overflowing unit that follows.
        let mut carried: Option<WrapUnit> = None;
        if let Some(last) = line.last() {
            if matches!(last, WrapUnit::Ch(c) if is_opening_punct(*c)) {
                if let Some(u) = line.pop() {
                    carried = Some(u);
                }
            }
        }
        lines.push(units_to_string(&line));
        line.clear();
        line_px = 0;
        if let Some(u) = carried {
            let uw = unit_width(&u);
            line.push(u);
            line_px += uw;
        }
        // The overflowing unit starts the fresh line (words hard-split).
        match unit {
            WrapUnit::Word(s) => {
                for piece in split_by_pixels(s, max_width_px) {
                    if !line.is_empty() {
                        lines.push(units_to_string(&line));
                        line.clear();
                        line_px = 0;
                    }
                    line.push(WrapUnit::Word(piece));
                    line_px += unit_width(line.last().unwrap());
                }
                i += 1;
            }
            WrapUnit::Ch(' ') => {
                // A space overflowed — drop it rather than open the line with it.
                i += 1;
            }
            WrapUnit::Ch(_) => {
                line.push(unit.clone());
                line_px += w;
                i += 1;
            }
            WrapUnit::CloseRun(_) => unreachable!("closing runs are handled above"),
        }
    }
    if !line.is_empty() {
        lines.push(units_to_string(&line));
    }
    lines
}

/// Latin word wrap for one authored row.
fn wrap_latin_paragraph(paragraph: &str, max_width_px: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_px: usize = 0;

    for token in paragraph.split_whitespace() {
        let token_px = measure_text(token) as usize;
        let space_px = if current.is_empty() {
            0
        } else {
            char_advance(' ') as usize
        };
        if !current.is_empty() && current_px + space_px + token_px > max_width_px {
            lines.push(core::mem::take(&mut current));
            current_px = 0;
        }
        if current.is_empty() {
            if token_px > max_width_px {
                // A single word wider than the line is hard-split by pixels.
                for piece in split_by_pixels(token, max_width_px) {
                    if !current.is_empty() {
                        lines.push(core::mem::take(&mut current));
                    }
                    current = piece;
                    current_px = measure_text(&current) as usize;
                }
            } else {
                current.push_str(token);
                current_px = token_px;
            }
        } else {
            current.push(' ');
            current.push_str(token);
            current_px += space_px + token_px;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Splits a word into chunks, each at most `max_width_px` pixels wide.
fn split_by_pixels(s: &str, max_width_px: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut chunk = String::new();
    let mut chunk_px: usize = 0;
    for ch in s.chars() {
        let w = char_advance(ch) as usize;
        if chunk_px + w > max_width_px && !chunk.is_empty() {
            out.push(core::mem::take(&mut chunk));
            chunk_px = 0;
        }
        chunk.push(ch);
        chunk_px += w;
    }
    if !chunk.is_empty() {
        out.push(chunk);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_rows_and_full_eighteen_cell_width_survive_wrapping() {
        assert_eq!(
            wrap_hard_lines("Hello!\nWelcome!", 144),
            vec!["Hello!", "Welcome!"]
        );
        assert_eq!(
            wrap_hard_lines("1234567890123456789", 144),
            vec!["123456789012345678", "9"]
        );
        assert_eq!(wrap_hard_lines("AB\n\nCD", 144), vec!["AB", "", "CD"]);
    }

    #[test]
    fn mixed_words_use_eight_and_ten_pixel_advances_without_border_overflow() {
        for text in [
            "第123456789012345678901234567890级，皮卡丘！",
            "123456789012345678901234567890等级",
            "一二三四五六七八九十ABCD！测试",
            "一ABCDEFGHIJKLMNOPQRSTUVWXYZ！",
        ] {
            let lines = wrap_hard_lines(text, 144);
            assert_eq!(lines.concat(), text, "no character may disappear: {text}");
            assert!(
                lines.iter().all(|line| measure_text(line) <= 144),
                "overflow: {lines:?}"
            );
        }
        assert_eq!(measure_text("一二三四五六七八九十ABCD"), 132);
    }

    #[test]
    fn chinese_punctuation_and_ascii_number_runs_remain_together() {
        assert_eq!(
            wrap_hard_lines("你好你好，世界", 40),
            vec!["你好你", "好，世界"]
        );
        assert_eq!(wrap_hard_lines("等级10级", 30), vec!["等级", "10级"]);
        assert_eq!(wrap_hard_lines("他说「你好", 30), vec!["他说", "「你好"]);
    }
}
