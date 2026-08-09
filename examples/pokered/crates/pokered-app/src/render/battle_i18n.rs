//! Battle-display localization.
//!
//! The battle messages are generated in `pokered-core` (English), and the
//! animation / visual-effect parsers in `render/battle.rs` match against the
//! raw English text (e.g. `message.contains(" used ")`). To localize without
//! breaking those parsers, this module translates the message **at display
//! time** only — the parsers still see the original English string.
//!
//! `zh_battle_dialog` handles the message templates produced by
//! `pokered_rules::runtime` (`move_announcement` / `translate_turn`) and by
//! `battle/mod.rs` (send-out, catch, exp/level-up, money, trainer intro), plus
//! the intro texts built in the renderer itself.

use std::collections::HashMap;
use std::sync::OnceLock;

use pokered_data::lang_data;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

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
    ("Player blacked out!", "眼前一片\n漆黑！"),
    ("winning!", "的获胜奖金！"),
    ("give you a tip!", "小费！"),
    ("Aren't I great?", "我很厉害吧？"),
    ("Gyaoo!", "嘎嗷！"),
    ("Darn! The GHOST\ncan't be ID'd!", "可恶！\n无法识别幽灵！"),
    ("SILPH SCOPE unveiled the\nGHOST's identity!", "西尔佛透视镜\n识破了幽灵的真身！"),
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
                let id = pokered_data::items::ItemId::from_id(i);
                (id != pokered_data::items::ItemId::NoItem)
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
        "EVASIVENESS" => "闪避率",
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
/// Chinese. `pub(crate)` so the PC / elevator / link renderers can reuse the
/// same EN→ZH name table for names embedded in their messages.
pub(crate) fn zh_name(s: &str) -> String {
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

fn num(s: &str) -> String {
    s.to_string()
}

/// Translate a rendered battle message to Chinese. Unknown messages pass
/// through unchanged.
pub fn zh_battle_dialog(text: &str) -> String {
    // 1. Static messages.
    if let Some(zh) = EXACT.iter().find(|(en, _)| *en == text).map(|(_, zh)| *zh) {
        return zh.to_string();
    }

    // 2. Dynamic templates. Order matters: more specific patterns first.

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
            return format!("{}获得了{}点经验值！", zh_name(&text[..idx]), num(rest));
        }
    }
    // "{a} grew to level {n}!"
    if let Some(idx) = text.find(" grew to level ") {
        if let Some(rest) = text[idx + " grew to level ".len()..].strip_suffix('!') {
            return format!("{}升到了{}级！", zh_name(&text[..idx]), num(rest));
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
            return format!("野生的{}出现了！", zh_name(a));
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

    // Stat-change lines: "{a}'s {stat} rose!" / "{a}'s {stat} fell!"
    if let Some(idx) = text.find("'s ") {
        let (a, rest) = text.split_at(idx);
        let rest = &rest["'s ".len()..];
        if let Some(stat) = rest.strip_suffix(" rose!") {
            return format!("{}的{}上升了！", zh_name(a), zh_stat(stat));
        }
        if let Some(stat) = rest.strip_suffix(" fell!") {
            return format!("{}的{}下降了！", zh_name(a), zh_stat(stat));
        }
    }

    // "{a} is fast asleep!"-style leftovers and anything unknown pass through.
    text.to_string()
}

/// Chinese name for a trainer class (battle intro / "wants to fight!").
pub fn trainer_class_zh(class: pokered_data::trainer_data::TrainerClass) -> &'static str {
    use pokered_data::trainer_data::TrainerClass::*;
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
    fn static_messages() {
        assert_eq!(zh_battle_dialog("Critical hit!"), "会心一击！");
        assert_eq!(zh_battle_dialog("It's super effective!"), "效果拔群！");
        assert_eq!(zh_battle_dialog("It's not very effective..."), "效果不太好……");
        assert_eq!(zh_battle_dialog("It doesn't affect the enemy!"), "对对方没有效果！");
    }

    #[test]
    fn dynamic_messages() {
        assert_eq!(zh_battle_dialog("PIKACHU used THUNDERSHOCK!"), "皮卡丘使用了电击！");
        assert_eq!(zh_battle_dialog("Enemy PIDGEY used GUST!"), "对方的波波使用了起风！");
        assert_eq!(zh_battle_dialog("PIKACHU fainted!"), "皮卡丘倒下了！");
        assert_eq!(zh_battle_dialog("Go! CHARMANDER!"), "去吧，小火龙！");
        assert_eq!(zh_battle_dialog("PIKACHU's attack missed!"), "皮卡丘的攻击没有命中！");
        assert_eq!(zh_battle_dialog("PIKACHU is fast asleep!"), "皮卡丘正在熟睡！");
        assert_eq!(zh_battle_dialog("PIKACHU gained 120 exp. points!"), "皮卡丘获得了120点经验值！");
        assert_eq!(zh_battle_dialog("PIKACHU grew to level 12!"), "皮卡丘升到了12级！");
        assert_eq!(zh_battle_dialog("PIKACHU learned THUNDERBOLT!"), "皮卡丘学会了十万伏特！");
        assert_eq!(zh_battle_dialog("Gotcha!\nCATERPIE was caught!"), "中了！\n绿毛虫被抓住了！");
        assert_eq!(zh_battle_dialog("BROCK wants to fight!"), "小刚想和你\n对战！");
        assert_eq!(zh_battle_dialog("Wild CATERPIE appeared!"), "野生的绿毛虫出现了！");
        assert_eq!(zh_battle_dialog("BROCK used FULL HEAL!"), "小刚使用了万灵药！");
    }

    #[test]
    fn money_fragments() {
        assert_eq!(zh_battle_dialog("Player got $300 for"), "获胜获得了$300元");
        assert_eq!(zh_battle_dialog("winning!"), "的获胜奖金！");
        assert_eq!(zh_battle_dialog("Plus $100 from Pay Day!"), "聚宝功\n追加$100元！");
        assert_eq!(zh_battle_dialog("Total: $400!"), "合计$400元！");
    }

    #[test]
    fn trainer_classes() {
        use pokered_data::trainer_data::TrainerClass;
        assert_eq!(trainer_class_zh(TrainerClass::Youngster), "短裤小子");
        assert_eq!(trainer_class_zh(TrainerClass::Misty), "小霞");
        assert_eq!(trainer_class_zh(TrainerClass::Rocket), "火箭队队员");
    }
}
