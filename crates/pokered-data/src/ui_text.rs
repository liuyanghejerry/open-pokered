//! Display-layer translation helpers shared by the frontends (pokered-app,
//! pokered-tui). These translate renderer-visible strings (PC status lines,
//! slot-machine messages, PC main-menu labels, reel symbol labels) at draw
//! time — the core strings themselves stay English state-machine data.
//!
//! `zh_name` (names embedded in messages) lives in [`crate::battle_text`].

use crate::battle_text::zh_name;

/// Translate a main-menu label produced by `pc_screen::main_menu_labels`
/// ("BILL's PC", "<NAME>'s PC", "PROF.OAK's PC", "#MON LEAGUE", "LOG OFF").
pub fn zh_main_menu_label(label: &str) -> String {
    match label {
        "BILL's PC" => "正辉的电脑".to_string(),
        "SOMEONE's PC" => "某个人的电脑".to_string(),
        "PROF.OAK's PC" => "大木博士的电脑".to_string(),
        "#MON LEAGUE" => "宝可梦联盟".to_string(),
        "LOG OFF" => crate::lang_data::ui_label("LOG OFF", true).to_string(),
        _ => match label.strip_suffix("'s PC") {
            Some(name) => format!("{}的电脑", name),
            None => label.to_string(),
        },
    }
}

const PC_LINE_ZH: &[(&str, &str)] = &[
    ("Switch on!", "开机！"),
    ("the PC.", "电脑。"),
    ("PC.", "电脑。"),
    ("Accessed BILL's", "访问了正辉的"),
    ("Accessed someone's", "访问了某个人的"),
    ("Accessed my PC.", "访问了自己的电脑。"),
    ("Accessed PROF.", "访问了大木博士的"),
    ("OAK's PC.", "电脑。"),
    ("Accessed #DEX", "访问了图鉴"),
    ("Rating System.", "评价系统。"),
    ("LEAGUE's site.", "联盟的站点。"),
    ("Accessed the HALL", "访问了名人堂"),
    ("OF FAME List.", "名单。"),
    ("What? There are", "什么？这里"),
    ("no #MON here!", "没有宝可梦！"),
    ("You can't take", "你不能带走"),
    ("any more #MON.", "更多的宝可梦。"),
    ("Deposit #MON", "请先存放"),
    ("first.", "宝可梦。"),
    ("You can't deposit", "你不能存放"),
    ("the last #MON!", "最后的宝可梦！"),
    ("Oops! This Box is", "哎呀！这个盒子"),
    ("full of #MON.", "装满了宝可梦。"),
    ("taken out.", "取出来了。"),
    ("released outside.", "放生了。"),
    ("There is nothing", "没有存放"),
    ("stored.", "任何东西。"),
    ("You have nothing", "你没有可"),
    ("to deposit.", "存放的东西。"),
    ("That's too impor-", "这太重要了，"),
    ("tant to toss!", "不能扔掉！"),
    ("No room left to", "没有空间"),
    ("store items.", "存放道具。"),
    ("You can't carry", "你带不了"),
    ("any more items.", "更多的道具。"),
    ("stored via PC.", "已存入电脑。"),
    ("Withdrew", "取出了"),
    ("Threw away", "扔掉了"),
    ("Closed link to", "已断开与大木"),
    ("PROF.OAK's PC.", "博士电脑的连线。"),
    ("#DEX comp-", "图鉴完成"),
    ("letion is:", "度："),
    ("PROF.OAK's", "大木博士"),
    ("Rating:", "评价："),
    // Professor Oak's #DEX rating texts (pokedex_rating.asm table).
    ("You still have", "你还有很多"),
    ("lots to do.", "要做的事。"),
    ("Look for #MON", "去草丛里"),
    ("in grassy areas!", "找宝可梦吧！"),
    ("You're on the", "你正走在"),
    ("right track!", "正确的路上！"),
    ("Get a FLASH HM", "去我的助手"),
    ("from my AIDE!", "那里拿闪光！"),
    ("You still need", "你还需要"),
    ("more #MON!", "更多宝可梦！"),
    ("Try to catch", "试着捕捉"),
    ("other species!", "其他种类！"),
    ("Good, you're", "不错，你"),
    ("trying hard!", "很努力！"),
    ("Get an ITEMFINDER", "去我的助手"),
    ("from my AIDE!", "那里拿探宝器！"),
    ("Looking good!", "看起来不错！"),
    ("Go find my AIDE", "去找我的助手"),
    ("when you get 50!", "凑满50只时！"),
    ("You finally got at", "你终于凑满"),
    ("least 50 species!", "至少50只了！"),
    ("Be sure to get", "记得去拿"),
    ("EXP.ALL from my", "我助手那里的"),
    ("AIDE!", "学习装置！"),
    ("Ho! This is geting", "哦！越来越"),
    ("even better!", "好了！"),
    ("Very good!", "非常好！"),
    ("Go fish for some", "去钓一些"),
    ("marine #MON!", "水里的宝可梦！"),
    ("Wonderful!", "太棒了！"),
    ("Do you like to", "你喜欢"),
    ("collect things?", "收集东西吗？"),
    ("I'm impressed!", "我很佩服！"),
    ("It must have been", "这一定"),
    ("difficult to do!", "很难做到！"),
    ("least 100 species!", "至少100只了！"),
    ("I can't believe", "真不敢相信"),
    ("how good you are!", "你有多厉害！"),
    ("You even have the", "你甚至拥有"),
    ("evolved forms of", "宝可梦的"),
    ("#MON! Super!", "进化形态！厉害！"),
    ("Excellent! Trade", "太棒了！和"),
    ("with friends to", "朋友交换"),
    ("get some more!", "得到更多吧！"),
    ("Outstanding!", "太出色了！"),
    ("You've become a", "你已经成了"),
    ("real pro at this!", "真正的高手！"),
    ("I have nothing", "我已经"),
    ("left to say!", "无话可说了！"),
    ("You're the", "你就是"),
    ("authority now!", "权威了！"),
    ("Your #DEX is", "你的图鉴"),
    ("entirely complete!", "完全完成了！"),
    ("Congratulations!", "恭喜你！"),
    // Change-box confirmation (renderer-built, routed through the same map).
    ("When you change a", "更换宝可梦盒子时，"),
    ("#MON BOX, data", "数据会被"),
    ("will be saved.", "保存。"),
    ("Is that okay?", "这样可以吗？"),
    // Oak's rating prompt.
    ("Want to get your", "想让大木博士"),
    ("#DEX rated?", "评价图鉴吗？"),
];

/// Translate one message line produced by `pokered_core::pc_screen` (or the
/// renderer's own confirmations) at display time. Exact lines hit
/// [`PC_LINE_ZH`]; dynamic templates (names/numbers embedded) are handled
/// below; anything unknown passes through unchanged.
pub fn zh_pc_line(line: &str) -> String {
    if let Some(zh) = PC_LINE_ZH.iter().find(|(en, _)| *en == line).map(|(_, zh)| *zh) {
        return zh.to_string();
    }
    // "<NAME> turned on" (PC boot).
    if let Some(name) = line.strip_suffix(" turned on") {
        return format!("{}打开了", zh_name(name));
    }
    // "<NAME> was ..." — deposit / release results.
    if let Some(name) = line.strip_suffix(" was") {
        return format!("{}被", zh_name(name));
    }
    // "<NAME> is ..." — withdraw result.
    if let Some(name) = line.strip_suffix(" is") {
        return format!("{}被", zh_name(name));
    }
    // "stored in Box N."
    if let Some(n) = line.strip_prefix("stored in Box ") {
        return format!("存入了盒子{}。", n);
    }
    // "Got <NAME>."
    if let Some(name) = line.strip_prefix("Got ") {
        return format!("得到了{}", zh_name(name));
    }
    // "Bye <NAME>!"
    if let Some(name) = line.strip_prefix("Bye ").and_then(|s| s.strip_suffix('!')) {
        return format!("再见了{}！", zh_name(name));
    }
    // "<N> #MON seen" / "<N> #MON owned" (Oaks rating summary).
    if let Some(n) = line.strip_suffix(" #MON seen") {
        return format!("已见{}只", n);
    }
    if let Some(n) = line.strip_suffix(" #MON owned") {
        return format!("拥有{}只", n);
    }
    // "<ITEM>." — the trailing period line of a withdraw/toss result.
    if let Some(name) = line.strip_suffix('.') {
        return format!("{}。", zh_name(name));
    }
    line.to_string()
}

/// Display label for a reel symbol (core symbols stay as-is; names get a
/// Chinese label). The core `symbol_label` strings are state-machine data —
/// this render-layer mapping only affects what the player sees.
pub fn zh_slot_symbol(label: &str) -> String {
    match label {
        "  7 " | "BAR " => label.to_string(),
        "CHER" => "樱桃".to_string(),
        "FISH" => "鱼".to_string(),
        "BIRD" => "鸟".to_string(),
        "MOUS" => "老鼠".to_string(),
        _ => label.to_string(),
    }
}

/// Status-line translation for the messages produced by
/// `pokered_core::slots_screen` (display-layer only — the core strings are
/// untouched).
pub fn zh_slots_message(msg: &str, is_zh: bool) -> String {
    if !is_zh {
        return msg.to_string();
    }
    match msg {
        "BET 1-3 COINS" => "下注1-3个代币".to_string(),
        "OUT OF COINS!" => "代币用完了！".to_string(),
        "STOP THE REELS!" => "停止转轮！".to_string(),
        "NO MATCH..." => "没有中奖……".to_string(),
        _ => {
            if let Some(n) = msg.strip_prefix("WIN! ").and_then(|s| s.strip_suffix(" COINS")) {
                return format!("中了！{}个代币", n);
            }
            if let Some(n) = msg
                .strip_prefix("BET ")
                .and_then(|s| s.strip_suffix(" COINS").or_else(|| s.strip_suffix(" COIN")))
            {
                return format!("下注{}个代币", n);
            }
            msg.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc_lines_translate_and_pass_unknown_through() {
        assert_eq!(zh_pc_line("Switch on!"), "开机！");
        assert_eq!(zh_pc_line("RATTATA turned on"), "小拉达打开了");
        // The trailing period is part of the captured number segment —
        // preserved as-is from the historical app-side helper.
        assert_eq!(zh_pc_line("stored in Box 3"), "存入了盒子3。");
        assert_eq!(zh_pc_line("some unknown line"), "some unknown line");
    }

    #[test]
    fn pc_main_menu_labels_translate() {
        assert_eq!(zh_main_menu_label("BILL's PC"), "正辉的电脑");
        assert_eq!(zh_main_menu_label("RED's PC"), "RED的电脑");
        assert_eq!(zh_main_menu_label("LOG OFF"), "退出登录");
    }

    #[test]
    fn slots_messages_and_symbols_translate() {
        assert_eq!(zh_slots_message("BET 1-3 COINS", true), "下注1-3个代币");
        assert_eq!(zh_slots_message("WIN! 300 COINS", true), "中了！300个代币");
        assert_eq!(zh_slots_message("BET 1-3 COINS", false), "BET 1-3 COINS");
        assert_eq!(zh_slot_symbol("CHER"), "樱桃");
        assert_eq!(zh_slot_symbol("BAR "), "BAR ");
    }
}
