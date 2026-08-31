//! Battle-text localization (English → Chinese).
//!
//! Battle messages are generated in `pokered-core` (English), and several
//! animation / visual-effect parsers match against the raw English text (e.g.
//! `message.contains(" used ")`). To localize without breaking those parsers,
//! [`localize`] translates a message **once, before pagination**
//! (`BattleScreen::show_text_then` calls it when `is_zh` is set), so template
//! matching always sees the original English string and the paginator re-wraps
//! the Chinese output. The renderer additionally localizes the few intro texts
//! it builds itself (they bypass `show_text_then`).
//!
//! Unknown messages pass through unchanged, so English data-driven text
//! (e.g. trainer victory quips) degrades gracefully — add exact entries for
//! those as they are identified.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::lang_data;
use crate::moves::MoveId;
use crate::species::Species;

/// Exact (static) message → Chinese. Fragments that only make sense as part of
/// a longer composition are translated here too, so sequential boxes read
/// naturally together.
const EXACT: &[(&str, &str)] = &[
    ("Critical hit!", "会心一击！"),
    ("It's super effective!", "效果拔群！"),
    ("It's not very effective...", "效果不太好……"),
    ("It doesn't affect the enemy!", "对对方没有效果！"),
    ("No PP left!", "PP用完了！"),
    ("Move is disabled!", "招式被封印了！"),
    ("Player blacked out!", "眼前一片漆黑！"),
    ("winning!", "的获胜奖金！"),
    ("give you a tip!", "小费！"),
    ("Aren't I great?", "我很厉害吧？"),
    ("Gyaoo!", "嘎嗷！"),
    ("Darn! The GHOST\ncan't be ID'd!", "可恶！\n无法识别幽灵！"),
    ("SILPH SCOPE unveiled the\nGHOST's identity!", "西尔佛透视镜\n识破了幽灵的真身！"),
    // Battle flow (battle/mod.rs).
    ("You won!", "你赢了！"),
    ("You lost...", "你输了……"),
    ("No other POKeMON!", "没有其他宝可梦了！"),
    ("Items can't be used here.", "这里不能使用道具。"),
    ("No items!", "没有道具！"),
    ("Can't escape!", "不能逃走！"),
    ("Got away safely!", "成功逃走了！"),
    ("But it failed!", "但是失败了！"),
    ("No! There's no running from a trainer battle!", "不行！训练家对战中不能逃走！"),
    ("Can't use that here!", "这里不能使用！"),
    ("Can't use that!", "不能使用！"),
    ("No effect!", "没有效果！"),
    ("There's no will to fight!", "没有战斗的意志！"),
    ("All STATUS changes\nare eliminated!", "所有能力变化\n都消除了！"),
    // Ghost battle.
    ("GHOST: Get out...\nGet out...", "幽灵：出去……\n出去……"),
    ("The GHOST is dodging\nyour POKé BALLs!", "幽灵躲开了\n你的精灵球！"),
    // Confusion self-hit, two-line variant (battle/mod.rs).
    ("It hurt itself in\nits confusion!", "因为混乱\n伤到了自己！"),
    // In-battle item results.
    ("Already at full HP!", "HP已经全满了！"),
    ("Not fainted!", "没有濒死！"),
    ("Status cured!", "治好了异常状态！"),
    ("No status to cure!", "没有可治的异常状态！"),
    ("Can't revive that!", "这个无法复活！"),
    ("Effect applied!", "效果出现了！"),
    ("Played the POKE FLUTE!", "吹响了宝可梦笛！"),
    ("All sleeping POKeMON woke up!", "睡着的宝可梦全都醒了！"),
    // Capture results.
    ("Caught!", "抓到了！"),
    ("Oh no! The ball missed!", "糟了！球没投中！"),
    ("Aww! It broke free!", "唉！它挣脱了！"),
    ("Shoot! It almost had it!", "可恶！就差一点了！"),
    ("Shoot! It was so close too!", "可恶！真的就差一点！"),
    ("It broke free!", "它挣脱了！"),
    // Safari-zone capture attempts.
    ("You missed the POKéMON!", "没投中宝可梦！"),
    ("Darn! The POKéMON\nbroke free!", "可恶！宝可梦\n挣脱了！"),
    ("Aww! It appeared\nto be caught!", "唉！它看起来\n能被抓住！"),
    ("Shoot! It was so\nclose, too!", "可恶！真的\n就差一点！"),
    ("Threw some BAIT!", "扔了诱饵！"),
    ("Threw a ROCK!", "扔了岩石！"),
    ("PA: You're out of\nSAFARI BALLs! Game over!", "广播：狩猎球\n用完了！游戏结束！"),
];

/// Species EN (uppercase) → ZH, built once from `lang_data`.
fn species_zh() -> &'static HashMap<String, &'static str> {
    static MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        (1..=151u8)
            .filter_map(|i| {
                let s = Species::from_index_id(i);
                (s != Species::None).then(|| (lang_data::species_name(s, false).to_string(), lang_data::species_name(s, true)))
            })
            .collect()
    })
}

/// Move EN (uppercase) → ZH, built once from `lang_data`.
fn move_zh() -> &'static HashMap<String, &'static str> {
    static MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        (1..=165u8)
            .filter_map(|i| {
                let m = MoveId::from_id(i);
                (m != MoveId::None).then(|| (lang_data::move_name(m, false).to_string(), lang_data::move_name(m, true)))
            })
            .collect()
    })
}

/// Item EN (uppercase) → ZH, built once from `lang_data`.
fn item_zh() -> &'static HashMap<String, &'static str> {
    static MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        (1..=83u8)
            .filter_map(|i| {
                let id = crate::items::ItemId::from_id(i);
                (id != crate::items::ItemId::NoItem)
                    .then(|| (lang_data::item_name(id, false).to_string(), lang_data::item_name(id, true)))
            })
            .collect()
    })
}

/// Translate a stat name in a stat-change message.
fn zh_stat(s: &str) -> &str {
    match s {
        "ATTACK" => "攻击",
        "DEFENSE" => "防御",
        "SPEED" => "速度",
        "SPECIAL" => "特攻",
        "ACCURACY" => "命中率",
        "EVASIVENESS" | "EVASION" => "闪避率",
        _ => s,
    }
}

/// Known proper names (gym leaders, rivals, staff) + trainer class display
/// names, EN → ZH, as they appear in battle messages.
const NAME_ZH: &[(&str, &str)] = &[
    ("OAK", "大木博士"),
    ("PROF.OAK", "大木博士"),
    ("BROCK", "小刚"),
    ("MISTY", "小霞"),
    ("LT.SURGE", "马志士"),
    ("ERIKA", "莉佳"),
    ("KOGA", "阿桔"),
    ("BLAINE", "夏伯"),
    ("SABRINA", "娜姿"),
    ("GIOVANNI", "坂木"),
    ("BRUNO", "希巴"),
    ("AGATHA", "菊子"),
    ("LANCE", "阿渡"),
    ("LORELEI", "科拿"),
    ("BILL", "正辉"),
    ("GARY", "小茂"),
    ("MR.FUJI", "富士老人"),
    ("ENEMY", "敌人"),
    ("TRAINER", "训练家"),
    ("OLD MAN", "老头"),
    ("GHOST", "幽灵"),
    ("YOUNGSTER", "短裤小子"),
    ("BUG CATCHER", "捕虫少年"),
    ("LASS", "迷你裙"),
    ("SAILOR", "水手"),
    ("JR.TRAINER", "训练家"),
    ("POKeMANIAC", "宝可梦狂"),
    ("SUPER NERD", "理科男"),
    ("HIKER", "登山男"),
    ("BIKER", "摩托车手"),
    ("BURGLAR", "盗贼"),
    ("ENGINEER", "工程师"),
    ("JUGGLER", "魔术师"),
    ("FISHER", "垂钓者"),
    ("SWIMMER", "泳装青年"),
    ("CUE BALL", "光头男"),
    ("GAMBLER", "赌徒"),
    ("BEAUTY", "美女"),
    ("PSYCHIC", "超能力者"),
    ("ROCKER", "摇滚青年"),
    ("TAMER", "驯兽师"),
    ("BIRD KEEPER", "养鸟人"),
    ("BLACKBELT", "空手道王"),
    ("RIVAL", "劲敌"),
    ("SCIENTIST", "科学家"),
    ("ROCKET", "火箭队队员"),
    ("COOLTRAINER", "精英训练家"),
    ("GENTLEMAN", "绅士"),
    ("CHANNELER", "通灵者"),
];

/// Translate a name embedded in a message: `"Enemy X"` / `"Wild X"` prefixes
/// get a Chinese qualifier, and known species/move/item/proper names become
/// Chinese.
pub fn zh_name(s: &str) -> String {
    let (prefix, rest) = if let Some(r) = s.strip_prefix("Enemy ") {
        ("对方的", r)
    } else if let Some(r) = s.strip_prefix("Wild ") {
        ("野生的", r)
    } else {
        ("", s)
    };
    let core = species_zh()
        .get(rest)
        .copied()
        .or_else(|| move_zh().get(rest).copied())
        .or_else(|| item_zh().get(rest).copied())
        .or_else(|| NAME_ZH.iter().find(|(en, _)| *en == rest).map(|(_, zh)| *zh))
        .unwrap_or(rest);
    format!("{}{}", prefix, core)
}

/// Translate a rendered battle message to Chinese. Unknown messages pass
/// through unchanged. `is_zh == false` returns the input untouched.
pub fn localize(text: &str, is_zh: bool) -> String {
    if !is_zh {
        return text.to_string();
    }

    // 1. Static messages + data-driven trainer victory quips (matched with
    //    trailing blank lines trimmed, as `dialog_text` does, so page
    //    padding can't break the lookup).
    let key = text.trim_matches('\n');
    if let Some(zh) = EXACT.iter().find(|(en, _)| *en == key).map(|(_, zh)| *zh) {
        return zh.to_string();
    }
    if let Some(zh) = crate::battle_quips::QUIPS
        .iter()
        .find(|(en, _)| *en == key)
        .map(|(_, zh)| *zh)
    {
        return zh.to_string();
    }

    // 2. Dynamic templates. Order matters: more specific patterns first.

    // "{a} used\n{b}!" — ball throw (the line break is part of the original
    // template); matched BEFORE the generic " used " rule.
    if let Some(idx) = text.find(" used\n") {
        if text.ends_with('!') {
            let (a, b) = text.split_at(idx);
            let b = &b[" used\n".len()..];
            let b = b.strip_suffix('!').unwrap_or(b);
            return format!("{}使用了\n{}！", zh_name(a), zh_name(b));
        }
    }

    // "{a} used {b}!" — move or item use.
    if let Some(idx) = text.find(" used ") {
        if text.ends_with('!') {
            let (a, b) = text.split_at(idx);
            let b = &b[" used ".len()..];
            let b = b.strip_suffix('!').unwrap_or(b);
            return format!("{}使用了{}！", zh_name(a), zh_name(b));
        }
    }

    // "{a}'s attack missed!"
    if let Some(a) = text.strip_suffix("'s attack missed!") {
        return format!("{}的攻击没有命中！", zh_name(a));
    }
    // "{a} fainted!"
    if let Some(a) = text.strip_suffix(" fainted!") {
        return format!("{}倒下了！", zh_name(a));
    }
    // "{a} was poisoned!"
    if let Some(a) = text.strip_suffix(" was poisoned!") {
        return format!("{}中毒了！", zh_name(a));
    }
    // "{a} was burned!"
    if let Some(a) = text.strip_suffix(" was burned!") {
        return format!("{}被灼伤了！", zh_name(a));
    }
    // "{a} was frozen solid!"
    if let Some(a) = text.strip_suffix(" was frozen solid!") {
        return format!("{}被冻住了！", zh_name(a));
    }
    // "{a} fell asleep!"
    if let Some(a) = text.strip_suffix(" fell asleep!") {
        return format!("{}睡着了！", zh_name(a));
    }
    // "{a} woke up!"
    if let Some(a) = text.strip_suffix(" woke up!") {
        return format!("{}醒来了！", zh_name(a));
    }
    // "{a} is paralyzed!\nIt may be unable\nto move!"
    if let Some(a) = text.strip_suffix(" is paralyzed!\nIt may be unable\nto move!") {
        return format!("{}麻痹了！\n可能无法\n行动！", zh_name(a));
    }
    // "{a} is hurt by POISON!"
    if let Some(a) = text.strip_suffix(" is hurt by POISON!") {
        return format!("{}受到毒\n的伤害！", zh_name(a));
    }
    // "{a} is hurt by its BURN!"
    if let Some(a) = text.strip_suffix(" is hurt by its BURN!") {
        return format!("{}受到灼伤\n的伤害！", zh_name(a));
    }
    // "{a}'s\nHEALTH is sapped\nby LEECH SEED!"
    if let Some(a) = text.strip_suffix("'s\nHEALTH is sapped\nby LEECH SEED!") {
        return format!("{}的体力\n被寄生种子\n吸取了！", zh_name(a));
    }
    // "Fire defrosted\n{a}!"
    if let Some(a) = text.strip_prefix("Fire defrosted\n") {
        if let Some(a) = a.strip_suffix('!') {
            return format!("火焰融化了\n{}身上的冰！", zh_name(a));
        }
    }
    // Status-blocked move lines: "{a} is fast asleep!" etc.
    for (suffix, zh_suffix) in [
        (" is fast asleep!", "正在熟睡！"),
        (" is frozen solid!", "被冻住了！"),
        (" is fully paralyzed!", "全身麻痹了！"),
        (" hurt itself in confusion!", "因为混乱\n伤到了自己！"),
        (" can't move!", "动弹不得！"),
    ] {
        if let Some(a) = text.strip_suffix(suffix) {
            return format!("{}{}", zh_name(a), zh_suffix);
        }
    }

    // Obedience lines.
    if let Some(a) = text.strip_suffix(" began\nto nap!") {
        return format!("{}开始\n打瞌睡！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" is\nloafing around.") {
        return format!("{}在\n偷懒。", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" turned\naway!") {
        return format!("{}转过头\n去！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" won't\nobey!") {
        return format!("{}不服从\n命令！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix("\nignored orders!") {
        return format!("{}无视了\n命令！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" must recharge!") {
        return format!("{}需要\n恢复能量！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" is angry!") {
        return format!("{}生气了！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" is eating!") {
        return format!("{}在吃东西！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" is too\nscared to move!") {
        return format!("{}吓得\n不敢动！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" ran away!") {
        return format!("{}逃走了！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix(" ran!") {
        return format!("{}逃走了！", zh_name(a));
    }

    // "Go! {a}!"
    if let Some(a) = text.strip_prefix("Go! ") {
        if let Some(a) = a.strip_suffix('!') {
            return format!("去吧，{}！", zh_name(a));
        }
    }
    // "{a} sent out {b}!"
    if let Some(idx) = text.find(" sent out ") {
        if text.ends_with('!') {
            let (a, b) = text.split_at(idx);
            let b = b[" sent out ".len()..].strip_suffix('!').unwrap_or("");
            return format!("{}派出了{}！", zh_name(a), zh_name(b));
        }
    }
    // "{a} withdrew {b}!"
    if let Some(idx) = text.find(" withdrew ") {
        if text.ends_with('!') {
            let (a, b) = text.split_at(idx);
            let b = b[" withdrew ".len()..].strip_suffix('!').unwrap_or("");
            return format!("{}收回了{}！", zh_name(a), zh_name(b));
        }
    }
    // "{b}, come back!"
    if let Some(a) = text.strip_suffix(", come back!") {
        return format!("{}，回来！", zh_name(a));
    }
    // "Gotcha!\n{a} was caught!" / "All right!\n{a} was caught!"
    for (prefix, zh_prefix) in [("Gotcha!\n", "中了！\n"), ("All right!\n", "太好了！\n")] {
        if let Some(a) = text.strip_prefix(prefix) {
            if let Some(a) = a.strip_suffix(" was caught!") {
                return format!("{}{}被抓住了！", zh_prefix, zh_name(a));
            }
        }
    }
    // "{a} gained {n} exp. points!"
    if let Some(idx) = text.find(" gained ") {
        if let Some(rest) = text[idx + " gained ".len()..].strip_suffix(" exp. points!") {
            return format!("{}获得了{}点经验值！", zh_name(&text[..idx]), rest);
        }
    }
    // "{a} grew to level {n}!"
    if let Some(idx) = text.find(" grew to level ") {
        if let Some(rest) = text[idx + " grew to level ".len()..].strip_suffix('!') {
            return format!("{}升到了{}级！", zh_name(&text[..idx]), rest);
        }
    }
    // "{a} learned {b}!"
    if let Some(idx) = text.find(" learned ") {
        if let Some(rest) = text[idx + " learned ".len()..].strip_suffix('!') {
            return format!("{}学会了{}！", zh_name(&text[..idx]), zh_name(rest));
        }
    }
    // "{a} defeated {b}!"
    if let Some(idx) = text.find(" defeated ") {
        if let Some(rest) = text[idx + " defeated ".len()..].strip_suffix('!') {
            return format!("{}打败了{}！", zh_name(&text[..idx]), zh_name(rest));
        }
    }
    // "{a} lost to {b}!"
    if let Some(idx) = text.find(" lost to ") {
        if let Some(rest) = text[idx + " lost to ".len()..].strip_suffix('!') {
            return format!("{}输给了{}！", zh_name(&text[..idx]), zh_name(rest));
        }
    }
    // "{a} is about to use {b}!\nWill {c} change POKéMON?"
    if let Some(idx) = text.find(" is about to use ") {
        let after = &text[idx + " is about to use ".len()..];
        if let Some(newline) = after.find("!\nWill ") {
            let (b, c) = after.split_at(newline);
            let b = b.strip_suffix('!').unwrap_or(b);
            let c = c["!\nWill ".len()..].strip_suffix(" change POKéMON?").unwrap_or("");
            return format!("{}要使用{}了！\n{}要更换宝可梦吗？", zh_name(&text[..idx]), zh_name(b), zh_name(c));
        }
    }
    // "{a} is already out!" (party menu; the EN side is split into two pages —
    // see the call site — the zh side recombines into one).
    if let Some(a) = text.strip_suffix(" is already out!") {
        return format!("{}已经在场上了！", zh_name(a));
    }

    // Money fragments.
    if let Some(a) = text.strip_prefix("Player got $") {
        if let Some(n) = a.strip_suffix(" for") {
            return format!("获胜获得了${}元", n);
        }
    }
    if let Some(n) = text.strip_prefix("Plus $") {
        if let Some(n) = n.strip_suffix(" from Pay Day!") {
            return format!("聚宝功\n追加${}元！", n);
        }
    }
    if let Some(n) = text.strip_prefix("Total: $") {
        if let Some(n) = n.strip_suffix('!') {
            return format!("合计${}元！", n);
        }
    }
    if let Some(a) = text.strip_suffix(" wants to") {
        return format!("{}想给你", zh_name(a));
    }

    // Trainer-intro / wild-appearance lines (some built in the renderer).
    if let Some(a) = text.strip_prefix("Wild ") {
        if let Some(a) = a.strip_suffix(" appeared!") {
            return format!("野生的{}出现了！", zh_name(a));
        }
    }
    if let Some(a) = text.strip_prefix("Enemy ") {
        if let Some(a) = a.strip_suffix(" appeared!") {
            return format!("对方的{}出现了！", zh_name(a));
        }
    }
    if let Some(a) = text.strip_prefix("The hooked ") {
        if let Some(a) = a.strip_suffix("\nattacked!") {
            return format!("被钓上的{}\n发动攻击！", zh_name(a));
        }
    }
    if let Some(a) = text.strip_suffix(" wants to fight!") {
        return format!("{}想和你\n对战！", zh_name(a));
    }

    // Charge-turn narration ("{a} flew up high!" etc.).
    for (suffix, zh_suffix) in [
        (" flew up high!", "飞到了高处！"),
        (" dug a hole!", "挖了一个洞！"),
        (" took in sunlight!", "吸收了阳光！"),
        (" made a whirlwind!", "卷起了旋风！"),
        (" lowered its head!", "低下了头！"),
        (" is glowing!", "浑身发出了光芒！"),
        (" began charging!", "开始蓄力！"),
    ] {
        if let Some(a) = text.strip_suffix(suffix) {
            return format!("{}{}", zh_name(a), zh_suffix);
        }
    }

    // Substitute.
    if let Some(a) = text.strip_suffix(" put in a SUBSTITUTE!") {
        return format!("{}使用了替身！", zh_name(a));
    }
    if let Some(a) = text.strip_suffix("'s SUBSTITUTE broke!") {
        return format!("{}的替身碎了！", zh_name(a));
    }
    // "{a} has no\nmoves left!"
    if let Some(a) = text.strip_suffix(" has no\nmoves left!") {
        return format!("{}没有能用的招式了！", zh_name(a));
    }

    // In-battle item results.
    if let Some(n) = text.strip_suffix("!")
        .and_then(|t| t.strip_prefix("HP restored by ")) {
        return format!("回复了{}点HP！", n);
    }
    if let Some(n) = text.strip_suffix("!")
        .and_then(|t| t.strip_prefix("Revived! HP restored by ")) {
        return format!("复活了！回复了{}点HP！", n);
    }

    // Stat-change lines: "{a}'s {stat} rose!" / "{a}'s {stat} fell!" — must run
    // BEFORE the ownerless "{STAT} rose!" rule (which would otherwise eat the
    // whole name+stat prefix), and the "greatly" variants must run FIRST
    // (they would otherwise be mangled by the plain-verb suffixes).
    if let Some(idx) = text.find("'s ") {
        let (a, rest) = text.split_at(idx);
        let rest = &rest["'s ".len()..];
        if let Some(stat) = rest.strip_suffix(" greatly rose!") {
            return format!("{}的{}大幅上升了！", zh_name(a), zh_stat(stat));
        }
        if let Some(stat) = rest.strip_suffix(" greatly fell!") {
            return format!("{}的{}大幅下降了！", zh_name(a), zh_stat(stat));
        }
        if let Some(stat) = rest.strip_suffix(" rose!") {
            return format!("{}的{}上升了！", zh_name(a), zh_stat(stat));
        }
        if let Some(stat) = rest.strip_suffix(" fell!") {
            return format!("{}的{}下降了！", zh_name(a), zh_stat(stat));
        }
    }

    // Ownerless stat item boost: "{STAT} rose!" (X Attack etc.) — checked
    // after the owned variant above.
    if let Some(stat) = text.strip_suffix(" rose!") {
        return format!("{}上升了！", zh_stat(stat));
    }

    // "{a} is fast asleep!"-style leftovers and anything unknown pass through.
    text.to_string()
}

/// Chinese name for a trainer class (battle intro / "wants to fight!").
pub fn trainer_class_zh(class: crate::trainer_data::TrainerClass) -> &'static str {
    use crate::trainer_data::TrainerClass::*;
    match class {
        Nobody => "训练家",
        Youngster => "短裤小子",
        BugCatcher => "捕虫少年",
        Lass => "迷你裙",
        Sailor => "水手",
        JrTrainerM => "训练家",
        JrTrainerF => "训练家",
        Pokemaniac => "宝可梦狂",
        SuperNerd => "理科男",
        Hiker => "登山男",
        Biker => "摩托车手",
        Burglar => "盗贼",
        Engineer => "工程师",
        UnusedJuggler => "魔术师",
        Fisher => "垂钓者",
        Swimmer => "泳装青年",
        CueBall => "光头男",
        Gambler => "赌徒",
        Beauty => "美女",
        PsychicTr => "超能力者",
        Rocker => "摇滚青年",
        Juggler => "魔术师",
        Tamer => "驯兽师",
        BirdKeeper => "养鸟人",
        Blackbelt => "空手道王",
        Rival1 => "劲敌",
        ProfOak => "大木博士",
        Chief => "科学家",
        Scientist => "科学家",
        Giovanni => "坂木",
        Rocket => "火箭队队员",
        CooltrainerM => "精英训练家",
        CooltrainerF => "精英训练家",
        Bruno => "希巴",
        Brock => "小刚",
        Misty => "小霞",
        LtSurge => "马志士",
        Erika => "莉佳",
        Koga => "阿桔",
        Blaine => "夏伯",
        Sabrina => "娜姿",
        Gentleman => "绅士",
        Rival2 => "劲敌",
        Rival3 => "劲敌",
        Lorelei => "科拿",
        Channeler => "通灵者",
        Agatha => "菊子",
        Lance => "阿渡",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_not_zh() {
        assert_eq!(localize("PIKACHU used THUNDERSHOCK!", false), "PIKACHU used THUNDERSHOCK!");
    }

    #[test]
    fn static_messages() {
        assert_eq!(localize("Critical hit!", true), "会心一击！");
        assert_eq!(localize("It's super effective!", true), "效果拔群！");
        assert_eq!(localize("It's not very effective...", true), "效果不太好……");
        assert_eq!(localize("It doesn't affect the enemy!", true), "对对方没有效果！");
        assert_eq!(localize("You won!", true), "你赢了！");
        assert_eq!(localize("Got away safely!", true), "成功逃走了！");
        assert_eq!(localize("But it failed!", true), "但是失败了！");
    }

    #[test]
    fn dynamic_messages() {
        assert_eq!(localize("PIKACHU used THUNDERSHOCK!", true), "皮卡丘使用了电击！");
        assert_eq!(localize("Enemy PIDGEY used GUST!", true), "对方的波波使用了起风！");
        assert_eq!(localize("PIKACHU fainted!", true), "皮卡丘倒下了！");
        assert_eq!(localize("Go! CHARMANDER!", true), "去吧，小火龙！");
        assert_eq!(localize("PIKACHU's attack missed!", true), "皮卡丘的攻击没有命中！");
        assert_eq!(localize("PIKACHU is fast asleep!", true), "皮卡丘正在熟睡！");
        assert_eq!(localize("PIKACHU gained 120 exp. points!", true), "皮卡丘获得了120点经验值！");
        assert_eq!(localize("PIKACHU grew to level 12!", true), "皮卡丘升到了12级！");
        assert_eq!(localize("PIKACHU learned THUNDERBOLT!", true), "皮卡丘学会了十万伏特！");
        assert_eq!(localize("Gotcha!\nCATERPIE was caught!", true), "中了！\n绿毛虫被抓住了！");
        assert_eq!(localize("BROCK wants to fight!", true), "小刚想和你\n对战！");
        assert_eq!(localize("Wild CATERPIE appeared!", true), "野生的绿毛虫出现了！");
        assert_eq!(localize("Enemy CATERPIE appeared!", true), "对方的绿毛虫出现了！");
        assert_eq!(localize("BROCK used FULL HEAL!", true), "小刚使用了万灵药！");
        // The old man's scripted throw has an explicit line break.
        assert_eq!(localize("OLD MAN used\nPOKé BALL!", true), "老头使用了\n精灵球！");
    }

    #[test]
    fn stat_lines() {
        assert_eq!(localize("PIKACHU's ATTACK rose!", true), "皮卡丘的攻击上升了！");
        assert_eq!(localize("PIKACHU's DEFENSE fell!", true), "皮卡丘的防御下降了！");
        // "greatly" must not be mangled by the plain-verb pattern.
        assert_eq!(localize("PIKACHU's SPEED greatly rose!", true), "皮卡丘的速度大幅上升了！");
        assert_eq!(localize("PIKACHU's SPECIAL greatly fell!", true), "皮卡丘的特攻大幅下降了！");
        // Ownerless X-item boost.
        assert_eq!(localize("ATTACK rose!", true), "攻击上升了！");
        assert_eq!(localize("EVASION rose!", true), "闪避率上升了！");
    }

    #[test]
    fn ball_throw_and_capture() {
        assert_eq!(localize("RED used\nGREAT BALL!", true), "RED使用了\n超级球！");
        assert_eq!(localize("Caught!", true), "抓到了！");
        assert_eq!(localize("Oh no! The ball missed!", true), "糟了！球没投中！");
        assert_eq!(localize("The GHOST is dodging\nyour POKé BALLs!", true), "幽灵躲开了\n你的精灵球！");
    }

    #[test]
    fn charge_and_substitute() {
        assert_eq!(localize("PIDGEY flew up high!", true), "波波飞到了高处！");
        assert_eq!(localize("DIGLETT dug a hole!", true), "地鼠挖了一个洞！");
        assert_eq!(localize("ODDISH took in sunlight!", true), "走路草吸收了阳光！");
        assert_eq!(localize("PIKACHU put in a SUBSTITUTE!", true), "皮卡丘使用了替身！");
        assert_eq!(localize("PIKACHU's SUBSTITUTE broke!", true), "皮卡丘的替身碎了！");
    }

    #[test]
    fn items_and_money() {
        assert_eq!(localize("HP restored by 20!", true), "回复了20点HP！");
        assert_eq!(localize("Revived! HP restored by 10!", true), "复活了！回复了10点HP！");
        assert_eq!(localize("Player got $300 for", true), "获胜获得了$300元");
        assert_eq!(localize("winning!", true), "的获胜奖金！");
        assert_eq!(localize("Plus $100 from Pay Day!", true), "聚宝功\n追加$100元！");
        assert_eq!(localize("Total: $400!", true), "合计$400元！");
    }

    #[test]
    fn switch_prompts() {
        assert_eq!(localize("PIKACHU is already out!", true), "皮卡丘已经在场上了！");
        assert_eq!(localize("There's no will to fight!", true), "没有战斗的意志！");
        assert_eq!(
            localize("BROCK is about to use ONIX!\nWill RED change POKéMON?", true),
            "小刚要使用大岩蛇了！\nRED要更换宝可梦吗？"
        );
    }

    #[test]
    fn trainer_quips_translate_exactly() {
        // Data-driven endBattleText (map.json) — exact table.
        assert_eq!(localize("You\nbeat me again!", true), "你又\n打败我了！");
        assert_eq!(localize("You're\nmean!", true), "你真\n坏！");
        assert_eq!(localize("Arrgh!", true), "啊——！");
    }

    #[test]
    fn trainer_classes() {
        use crate::trainer_data::TrainerClass;
        assert_eq!(trainer_class_zh(TrainerClass::Youngster), "短裤小子");
        assert_eq!(trainer_class_zh(TrainerClass::Misty), "小霞");
        assert_eq!(trainer_class_zh(TrainerClass::Rocket), "火箭队队员");
    }
}
