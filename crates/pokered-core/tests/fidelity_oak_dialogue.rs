use dotzuki_engine_script::{CommandResult, ScriptCommand};
use pokered_core::overworld::native_script::NativeScriptEngine;

// DexRatingsTable from pret/pokered engine/events/pokedex_rating.asm.
const RATINGS: [&str; 16] = [
    "You still have lots to do.",
    "You're on the right track!",
    "You still need more POKeMON!",
    "Good, you're trying hard!",
    "Looking good!",
    "You finally got at least 50",
    "Ho! This is geting even",
    "Very good!",
    "Wonderful!",
    "I'm impressed!",
    "You finally got at least 100",
    "You even have the evolved",
    "Excellent!",
    "Outstanding!",
    "I have nothing left to say!",
    "Your POKeDEX is entirely",
];

#[test]
fn oak_selects_exactly_one_original_rating_and_no_earlier_story_dialogue() {
    for owned in 0..=151 {
        let mut engine = NativeScriptEngine::new();
        engine.load_embedded_map("OaksLab", pokered_data::embedded_scenes::scene_functions());
        for flag in [
            "EVENT_GOT_POKEDEX",
            "EVENT_GOT_STARTER",
            "EVENT_BATTLED_RIVAL_IN_OAKS_LAB",
            "EVENT_PALLET_AFTER_GETTING_POKEBALLS",
        ] {
            engine.set_flag(flag, true);
        }
        engine.seed_number("pokedexOwned", owned as f64);
        engine.seed_number("pokedexSeen", 151.0);
        let mut next = engine.call_function_no_args("talkOak1").unwrap();
        let mut text = Vec::new();
        for _ in 0..32 {
            let Some(cmd) = next else {
                break;
            };
            if let ScriptCommand::ShowText { text: message } = cmd {
                text.push(message);
            }
            next = engine.signal_done(CommandResult::Void).unwrap();
        }
        assert!(next.is_none(), "rating must terminate for {owned}");
        assert_eq!(
            text.len(),
            3,
            "greeting, counts, one rating only: {owned}: {text:?}"
        );
        assert!(
            text[2].starts_with(RATINGS[(owned / 10).min(15)]),
            "wrong rating for {owned}: {:?}",
            text[2]
        );
    }
}
