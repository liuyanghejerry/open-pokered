use pokered_core::{
    game_state::Lang,
    oak_speech::{self, entrance_frames, OakSpeechInput, OakSpeechPhase, OakSpeechState},
    pokedex_screen::chinese_description_lines,
    text::zh_dialogue::{no_line_end, no_line_start},
};
use pokered_data::dialogue_layout::measure_text;

#[test]
fn all_sixteen_chinese_pc_ratings_use_complete_messages_and_readable_pages() {
    use pokered_core::{
        main_menu::MenuInput,
        pc_screen::{dex_rating_text, PcContext, PcEntry, PcOpenContext, PcPhase, PcScreen},
        save::SaveData,
    };
    for owned in (0..=150).step_by(10) {
        let mut save = SaveData::new();
        for n in 1..=owned {
            save.game_data
                .pokedex
                .set_owned(pokered_data::species::Species::from_index_id(n));
        }
        let mut pc = PcScreen::new_with_language(
            PcEntry::PokemonCenter,
            &PcOpenContext {
                has_pokedex: true,
                met_bill: true,
                beaten_league: false,
                player_name: "张三丰".into(),
                hof_teams: Vec::new(),
            },
            Lang::Zh,
        );
        let mut ctx = PcContext {
            party: &mut save.party,
            pc_storage: &mut save.pc_storage,
            bag: &mut save.game_data.bag,
            pc_items: &mut save.game_data.pc_items,
            pokedex: &save.game_data.pokedex,
        };
        let a = MenuInput {
            a: true,
            ..MenuInput::none()
        };
        while pc.phase() == PcPhase::Message {
            pc.update_frame(a, &mut ctx);
        }
        for _ in 0..2 {
            pc.update_frame(
                MenuInput {
                    down: true,
                    ..MenuInput::none()
                },
                &mut ctx,
            );
        }
        pc.update_frame(a, &mut ctx);
        while pc.phase() == PcPhase::Message {
            pc.update_frame(a, &mut ctx);
        }
        assert_eq!(pc.phase(), PcPhase::OaksConfirm);
        pc.update_frame(
            MenuInput {
                up: true,
                ..MenuInput::none()
            },
            &mut ctx,
        );
        pc.update_frame(a, &mut ctx);
        for _ in 0..15 {
            pc.update_frame(MenuInput::none(), &mut ctx);
        }
        assert_eq!(pc.phase(), PcPhase::Message);
        let pages = pc.message_page_count();
        for _ in 0..pages {
            pc.update_frame(a, &mut ctx);
        }
        let expected = pokered_data::ui_text::zh_pc_message(
            &dex_rating_text(owned as u32)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
        .concat();
        assert_eq!(pc.message_lines().concat(), expected);
        assert_eq!(pc.message_page_count(), 1, "rating {owned}");
        assert!(pc.message_lines().len() <= 3, "Hall of Fame rating {owned}");
        for line in pc.message_lines() {
            assert!(!line.is_empty());
            assert!(measure_text(line) <= 144, "rating {owned}: {line}");
            assert!(!line.chars().next().is_some_and(no_line_start));
            assert!(!line.chars().last().is_some_and(no_line_end));
        }
        match owned {
            10 => assert!(expected.contains("闪光")),
            30 => {
                assert!(expected.contains("探宝器"));
                assert!(!expected.contains("闪光"));
            }
            50 => assert!(expected.contains("学习装置")),
            _ => {}
        }
        pc.update_frame(a, &mut ctx);
        assert_eq!(pc.message_lines().concat(), "已断开与大木博士电脑的连线。");
    }
}

fn phases() -> Vec<OakSpeechPhase> {
    vec![
        OakSpeechPhase::Greeting {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
        OakSpeechPhase::ShowNidorino {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
        OakSpeechPhase::Explanation {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
        OakSpeechPhase::IntroducePlayer {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
        OakSpeechPhase::IntroduceRival {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
        OakSpeechPhase::FinalSpeech {
            page_index: 0,
            char_index: 0,
            waiting_for_input: false,
        },
    ]
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn oak_all_localized_pages_drive_the_actual_typewriter_and_transition() {
    let a = OakSpeechInput {
        a: true,
        ..OakSpeechInput::none()
    };
    for language in [Lang::En, Lang::Zh] {
        for name in ["RED", "小智", "王小明同学", "ABCDEFG"] {
            for phase in phases() {
                let mut state = OakSpeechState::new();
                state.language = language;
                state.player_name = Some(name.into());
                state.phase = phase;
                state.phase_frame = entrance_frames(&state.phase);
                let mut shown = Vec::new();
                while let Some(page) = state.current_text_page() {
                    let index = shown.len();
                    let expected = oak_speech::text_pages_for_lang(&state.phase, language).unwrap();
                    assert_eq!(&page, expected);
                    let chars = page.total_chars(Some(name));
                    assert!(chars > 0, "blank localized page {language:?}:{index}");
                    for tick in 1..=chars {
                        state.update_frame(OakSpeechInput::none());
                        assert_eq!(state.current_char_index() as usize, tick);
                        assert_eq!(state.is_waiting_for_input(), tick == chars);
                    }
                    let text = page.get_display_text(Some(name), u16::MAX);
                    if language == Lang::Zh {
                        assert!(measure_text(&text.0) <= 144);
                        assert!(measure_text(&text.1) <= 136);
                        for line in [&text.0, &text.1] {
                            assert!(!line.chars().last().is_some_and(no_line_end));
                            assert!(
                                !line.chars().next().is_some_and(no_line_start)
                                    || line.starts_with("……")
                            );
                        }
                    }
                    shown.push(format!("{}{}", text.0, text.1));
                    let previous = std::mem::discriminant(&state.phase);
                    state.update_frame(a);
                    if std::mem::discriminant(&state.phase) != previous {
                        break;
                    }
                    assert_eq!(state.current_char_index(), 0);
                }
                assert!(!shown.is_empty());
            }
        }
    }
}

#[test]
fn all_chinese_pokedex_pages_preserve_text_and_fit_three_rows() {
    let mut audit = Vec::new();
    let mut count = 0;
    for entry in pokered_data::pokedex::POKEDEX_ENTRIES {
        assert!(
            measure_text(entry.category_for(true)) <= 80,
            "category overlaps sprite {:?}",
            entry.species
        );
        let lines = chinese_description_lines(entry);
        let original = entry.flavor_text_pages_zh.concat();
        assert_eq!(
            compact(&original),
            compact(&lines.concat()),
            "{:?}",
            entry.species
        );
        let mut pages = Vec::new();
        for (page_index, rows) in lines.chunks(3).enumerate() {
            count += 1;
            assert!(!rows[0].is_empty());
            for row in rows {
                assert!(measure_text(row) <= 144);
                assert!(
                    !row.chars().next().is_some_and(no_line_start),
                    "{:?}: {row}",
                    entry.species
                );
                assert!(!row.chars().last().is_some_and(no_line_end));
            }
            if page_index + 1 < lines.len() / 3 {
                let ending = rows.iter().rev().find(|s| !s.is_empty()).unwrap();
                assert!(
                    ending.ends_with(['。', '！', '？']),
                    "{:?}: page splits a sentence: {ending}",
                    entry.species
                );
            }
            pages.push(serde_json::json!({"line1":rows[0],"line2":rows[1],"line3":rows[2]}));
        }
        let mut dex = pokered_core::pokemon::pokedex::Pokedex::new();
        dex.set_seen(entry.species);
        dex.set_owned(entry.species);
        let mut state = pokered_core::pokedex_screen::PokedexScreenState::new_entry(
            dex,
            entry.species,
            pokered_data::wild_data::GameVersion::Red,
        );
        assert_eq!(state.entry_total_pages(), entry.flavor_text_pages.len());
        state.language = Lang::Zh;
        assert_eq!(state.entry_total_pages(), pages.len());
        for index in 0..pages.len() {
            assert_eq!(state.entry_page(), index);
            assert_eq!(
                state.mode(),
                pokered_core::pokedex_screen::PokedexScreenMode::Entry
            );
            let action = state.update_frame(pokered_core::pokedex_screen::PokedexScreenInput {
                a: true,
                ..Default::default()
            });
            assert_eq!(
                action,
                if index + 1 == pages.len() {
                    pokered_core::pokedex_screen::PokedexScreenAction::Closed
                } else {
                    pokered_core::pokedex_screen::PokedexScreenAction::Active
                }
            );
        }
        audit.push(serde_json::json!({"source":format!("pokedex::{:?}",entry.species),"text":original,"pages":pages}));
    }
    assert_eq!(audit.len(), 151);
    assert!(count >= 151 && count <= 302);
    if let Ok(path) = std::env::var("ZH_DESCRIPTION_AUDIT") {
        std::fs::write(path, serde_json::to_string_pretty(&audit).unwrap()).unwrap();
    }
}

#[test]
fn oak_chinese_edits_preserve_the_complete_original_script() {
    let pairs = [
        (
            oak_speech::OAK_SPEECH_TEXT1_PAGES_ZH,
            "你好！欢迎来到宝可梦的世界！我是大木博士！大家都叫我宝可梦博士！",
        ),
        (
            oak_speech::OAK_SPEECH_TEXT2A_PAGES_ZH,
            "这个世界生活着一种叫做宝可梦的神奇生物！",
        ),
        (
            oak_speech::OAK_SPEECH_TEXT2B_PAGES_ZH,
            "对一些人来说，宝可梦是宠物。另一些人会用它们来对战。而我……则以研究宝可梦为职业。",
        ),
        (
            oak_speech::INTRODUCE_PLAYER_TEXT_PAGES_ZH,
            "首先，请问你叫什么名字？",
        ),
        (
            oak_speech::INTRODUCE_RIVAL_TEXT_PAGES_ZH,
            "这是我的孙子。从你还是婴儿时他就是你的竞争对手了。……呃，他叫什么名字来着？",
        ),
        (
            oak_speech::OAK_SPEECH_TEXT3_PAGES_ZH,
            "<PLAYER>！属于你的宝可梦传说就要开始了！一个充满梦想与冒险的世界正等着你！出发吧！",
        ),
    ];
    for (pages, original) in pairs {
        assert_eq!(
            pages
                .iter()
                .map(|p| format!("{}{}", p.line1, p.line2))
                .collect::<String>(),
            original
        );
    }
}
