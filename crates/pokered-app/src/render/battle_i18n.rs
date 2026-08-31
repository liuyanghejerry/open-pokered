//! Battle-display localization.
//!
//! The translation tables live in `pokered-data::battle_text` so the CORE can
//! localize battle messages before pagination (`BattleScreen::show_text_then`
//! → `battle_text::localize` when `is_zh` is set). This module keeps the
//! historical app-side paths working: the renderer still localizes the intro
//! texts it builds itself, and the PC / elevator / link renderers reuse
//! `zh_name` for names embedded in their messages.

pub use pokered_data::battle_text::{localize as zh_battle_dialog, trainer_class_zh, zh_name};
