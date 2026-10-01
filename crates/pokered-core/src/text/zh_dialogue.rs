//! Chinese overworld dialogue layout, completed before the typewriter starts.
use crate::alloc_prelude::*;
use crate::overworld::screen::DialoguePage;
use pokered_data::dialogue_layout::{
    char_advance, measure_text, LINE_WIDTH_PX, SECOND_LINE_WIDTH_PX,
};
use pokered_data::zh_dialogue_words::{MAX_WORD_CHARS, WORDS};

pub fn no_line_start(c: char) -> bool {
    "，。！？；：、）】〉》」』”’,.!?;:)]}…—～".contains(c)
}

pub fn no_line_end(c: char) -> bool {
    matches!(
        c,
        '（' | '【' | '〈' | '《' | '「' | '『' | '“' | '‘' | '(' | '[' | '{'
    )
}

fn sentence_end(text: &str) -> bool {
    text.chars()
        .rev()
        .find(|&c| !"）】〉》」』”’)]}".contains(c))
        .is_some_and(|c| "。！？!?".contains(c))
}

fn clause_end(text: &str) -> bool {
    sentence_end(text) || text.ends_with(['，', '；', '：', ',', ';', ':'])
}

fn soft_join(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines {
        let line = line.trim();
        if let (Some(a), Some(b)) = (out.chars().last(), line.chars().next()) {
            // CJK soft breaks are not spaces; Latin words need separation.
            if a.is_ascii_alphanumeric() && b.is_ascii_alphanumeric() {
                out.push(' ');
            }
        }
        out.push_str(line);
    }
    out
}

fn word_bytes(text: &str, protected: &[&str]) -> usize {
    let named = protected
        .iter()
        .filter(|w| !w.is_empty() && text.starts_with(**w))
        .map(|w| w.len())
        .max()
        .unwrap_or(0);
    let mut longest = named;
    for (offset, c) in text.char_indices().take(MAX_WORD_CHARS) {
        let end = offset + c.len_utf8();
        if end > longest && WORDS.binary_search(&&text[..end]).is_ok() {
            longest = end;
        }
    }
    longest
}

fn units(text: &str, protected: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut rest = text;
    let mut opening = String::new();
    while let Some(c) = rest.chars().next() {
        if no_line_start(c) && opening.is_empty() && !result.is_empty() {
            let mut spaces = String::new();
            while result.last().is_some_and(|s| s.trim().is_empty()) {
                spaces.insert_str(0, &result.pop().unwrap());
            }
            if result.is_empty() {
                result.push(c.to_string());
                rest = &rest[c.len_utf8()..];
                continue;
            }
            result.last_mut().unwrap().push_str(&spaces);
            result.last_mut().unwrap().push(c);
            rest = &rest[c.len_utf8()..];
            continue;
        }
        if no_line_end(c) {
            opening.push(c);
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let mut end = word_bytes(rest, protected);
        if end == 0 && (c.is_ascii_alphanumeric() || c == '¥') {
            for (offset, ch) in rest.char_indices() {
                if !(ch.is_ascii_alphanumeric() || "¥'-./%".contains(ch)) {
                    break;
                }
                end = offset + ch.len_utf8();
            }
        }
        if end == 0 {
            end = c.len_utf8();
        }
        opening.push_str(&rest[..end]);
        result.push(core::mem::take(&mut opening));
        rest = &rest[end..];
    }
    if !opening.is_empty() {
        result.push(opening);
    }
    result
}

// Only unavoidably overlong units (e.g. a 40-letter custom nickname) split.
// Roll punctuation with its preceding character and an opener with its next.
fn split_overlong(unit: &str, width: usize) -> (String, String) {
    let mut end = 0;
    let mut used = 0;
    for (offset, c) in unit.char_indices() {
        used += char_advance(c) as usize;
        if used > width {
            break;
        }
        end = offset + c.len_utf8();
    }
    while end > 0
        && (unit[end..].chars().next().is_some_and(no_line_start)
            || unit[..end].chars().last().is_some_and(no_line_end))
    {
        end = unit[..end].char_indices().last().unwrap().0;
    }
    if end == 0 {
        end = unit.chars().next().unwrap().len_utf8();
    }
    (unit[..end].to_string(), unit[end..].to_string())
}

fn take_line(tokens: &mut Vec<String>, width: usize, second: bool) -> String {
    while tokens.first().is_some_and(|s| s.trim().is_empty()) {
        tokens.remove(0);
    }
    let mut count = 0;
    let mut used = 0;
    let mut clause = None;
    let mut sentence = None;
    for token in tokens.iter() {
        let w = measure_text(token) as usize;
        if used + w > width {
            break;
        }
        used += w;
        count += 1;
        if !second
            && sentence_end(token)
            && next_sentence_width(&tokens[count..]) > SECOND_LINE_WIDTH_PX
        {
            sentence = Some(count);
            break;
        }
        if second && sentence_end(token) {
            sentence = Some(count);
        }
        // Prefer a complete clause to a nearly full row; page boundaries
        // have a stronger preference than ordinary line boundaries.
        if clause_end(token) && used >= width * if second { 1 } else { 2 } / 3 {
            clause = Some(count);
        }
    }
    if count == 0 && !tokens.is_empty() {
        if second && measure_text(&tokens[0]) as usize <= LINE_WIDTH_PX {
            // A unit that fits a full row belongs on the next page, rather
            // than being split just to fill the arrow-reserved second row.
            return String::new();
        }
        let (line, remainder) = split_overlong(&tokens[0], width);
        if remainder.is_empty() {
            tokens.remove(0);
        } else {
            tokens[0] = remainder;
        }
        return line;
    }
    if let Some(at) = sentence.filter(|_| !second || count < tokens.len()) {
        count = at;
    } else if count < tokens.len() {
        if let Some(at) = clause {
            count = at;
        } else if second {
            // Avoid a new page containing only a particle such as "的！".
            // Move whole words back from row 2 until the continuation has
            // enough context to read naturally, without changing row 1.
            let mut tail: usize = tokens[count..]
                .iter()
                .map(|s| measure_text(s) as usize)
                .sum();
            if tail <= 40 {
                while count > 0 && tail < 60 {
                    count -= 1;
                    tail += measure_text(&tokens[count]) as usize;
                }
            }
        }
    }
    tokens
        .drain(..count)
        .collect::<Vec<_>>()
        .concat()
        .trim()
        .to_string()
}

fn next_sentence_width(tokens: &[String]) -> usize {
    let mut width = 0;
    for token in tokens {
        width += measure_text(token) as usize;
        if sentence_end(token) {
            break;
        }
    }
    width
}

/// Single newlines are soft wrapping; one or more blank lines end a paragraph
/// and start a fresh page without generating a blank page. Explicit names
/// are substituted by the caller first and protected as complete units.
pub fn paginate(text: &str, protected: &[&str]) -> Vec<DialoguePage> {
    let mut paragraphs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            if !paragraphs.last().unwrap().is_empty() {
                paragraphs.push(Vec::new());
            }
        } else {
            paragraphs.last_mut().unwrap().push(line);
        }
    }
    let mut pages = Vec::new();
    for lines in paragraphs.into_iter().filter(|p| !p.is_empty()) {
        let paragraph = soft_join(&lines);
        let mut tokens = units(&paragraph, protected);
        while !tokens.is_empty() {
            let line1 = take_line(&mut tokens, LINE_WIDTH_PX, false);
            let line2 =
                if sentence_end(&line1) && next_sentence_width(&tokens) > SECOND_LINE_WIDTH_PX {
                    String::new()
                } else {
                    take_line(&mut tokens, SECOND_LINE_WIDTH_PX, true)
                };
            if !line1.is_empty() || !line2.is_empty() {
                pages.push(DialoguePage {
                    line1: line1.into_boxed_str(),
                    line2: line2.into_boxed_str(),
                });
            }
        }
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::{script_bridge, BedroomDialogue};

    fn compact(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }

    fn check(text: &str, names: &[&str]) -> Vec<DialoguePage> {
        let pages = paginate(text, names);
        let all = pages
            .iter()
            .map(|p| format!("{}{}", p.line1, p.line2))
            .collect::<String>();
        assert_eq!(compact(&all), compact(text));
        for page in &pages {
            assert!(!page.line1.trim().is_empty(), "no blank page/first row");
            assert!(measure_text(&page.line1) as usize <= LINE_WIDTH_PX);
            assert!(measure_text(&page.line2) as usize <= SECOND_LINE_WIDTH_PX);
            for line in [&page.line1, &page.line2] {
                assert!(!line.chars().last().is_some_and(no_line_end));
            }
        }
        pages
    }

    #[test]
    fn paragraph_separators_do_not_become_blank_pages() {
        let pages = check("\n\n你好！\n\n\n欢迎来到\n宝可梦中心。\n\n", &[]);
        assert_eq!(pages.len(), 2);
        assert_eq!(&*pages[0].line1, "你好！");
        assert_eq!(&*pages[1].line1, "欢迎来到宝可梦中心。");
        assert!(paginate("\n \n", &[]).is_empty());
        assert!(script_bridge::text_to_dialogue("").is_done());
    }

    #[test]
    fn game_words_names_and_ascii_runs_stay_whole() {
        let text = "你需要自己的宝可梦来保护自己。我知道了！王小明同学，欢迎使用TM24和PP恢复药。";
        let pages = check(text, &["王小明同学"]);
        let lines: Vec<&str> = pages.iter().flat_map(|p| [&*p.line1, &*p.line2]).collect();
        for word in ["宝可梦", "保护自己", "王小明同学", "TM24", "PP"] {
            assert!(
                lines.iter().any(|line| line.contains(word)),
                "split word: {word}"
            );
        }
    }

    #[test]
    fn closing_runs_and_opening_brackets_never_dangle() {
        let text = "一二三四五六七八九十甲乙丙丁……！再来看看《宝可梦图鉴》中的记录。";
        let pages = check(text, &[]);
        for line in pages.iter().flat_map(|p| [&p.line1, &p.line2]) {
            assert!(!line.chars().next().is_some_and(no_line_start), "{line}");
        }
        let text = pages
            .iter()
            .map(|p| format!("{}\n{}", p.line1, p.line2))
            .collect::<String>();
        assert!(text.contains("……！"));
        assert!(text.contains("《宝可梦图鉴》"));
    }

    #[test]
    fn completed_sentence_does_not_start_a_long_sentence_on_last_row() {
        let pages = check(
            "你好！你需要自己的宝可梦来保护自己，然后才能去外面旅行。",
            &[],
        );
        assert_eq!(&*pages[0].line1, "你好！");
        assert!(pages[0].line2.is_empty());
        assert!(pages[1].line1.starts_with("你需要"));
    }

    #[test]
    fn short_complete_sentence_on_second_row_prevents_a_fragment_page() {
        let pages = check(
            "那个老家伙曾经也很强悍、英俊！不过那是几十年前的事了！",
            &[],
        );
        assert_eq!(&*pages[0].line2, "英俊！");
        assert_eq!(&*pages[1].line1, "不过那是几十年前的事了！");
    }

    #[test]
    fn overlong_sentence_keeps_context_on_its_final_page() {
        let pages = check(
            "一二三四五六七八九十甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未了！",
            &[],
        );
        assert_eq!(pages.len(), 2);
        assert!(measure_text(&pages[1].line1) >= 60);
    }

    #[test]
    fn long_custom_words_progress_without_losing_text() {
        check(
            "你好ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789欢迎来到宝可梦中心。",
            &[],
        );
    }

    #[test]
    fn a_unit_fitting_only_the_first_row_moves_to_next_page() {
        let word = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let pages = check(&format!("这里有十四个字一二三四五六七\n{word}。"), &[]);
        assert!(pages.iter().any(|p| p.line1.contains(word)));
    }

    #[test]
    fn placeholder_expansion_precedes_layout_and_preserves_names() {
        use pokered_data::map_json::TextPageJson;
        let d = BedroomDialogue::from_text_pages(
            &[TextPageJson {
                line1: "欢迎<PLAYER>和<RIVAL>来这里！".into(),
                line2: "<STARTER>也一起过来吧！".into(),
            }],
            "王小明同学",
            "张小花同学",
            "妙蛙种子",
        );
        for word in ["王小明同学", "张小花同学", "妙蛙种子"] {
            assert!(d
                .pages()
                .iter()
                .any(|p| p.line1.contains(word) || p.line2.contains(word)));
        }
    }

    #[test]
    fn english_authored_pages_stay_unchanged() {
        let d = script_bridge::text_to_dialogue("Hello!\nWelcome!\n\nAgain.");
        assert_eq!(d.pages().len(), 2);
        assert_eq!(&*d.pages()[0].line2, "Welcome!");
        assert_eq!(&*d.pages()[1].line1, "");
        assert_eq!(&*d.pages()[1].line2, "Again.");
    }
}
