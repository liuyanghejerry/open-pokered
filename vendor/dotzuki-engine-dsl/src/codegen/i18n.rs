// Shared i18n codegen helpers for the `@t("en", "中文")` localized literal.
//
// Kept in its own submodule (rather than `codegen/mod.rs`) because the crate's
// `build.rs` reconstructs the module tree via `include!` of each submodule
// file; helpers placed in `mod.rs` are invisible to that build-script
// compilation. This file is included by both `codegen/mod.rs` and `build.rs`.

/// Look up a locale's text within ordered `@t(...)` `(locale, text)` pairs,
/// falling back to `en`, then the first pair, then `""`.

pub(crate) fn locale_text<'a>(pairs: &'a [(String, String)], locale: &str) -> &'a str {
    pairs
        .iter()
        .find(|(l, _)| l == locale)
        .or_else(|| pairs.iter().find(|(l, _)| l == "en"))
        .or_else(|| pairs.first())
        .map(|(_, t)| t.as_str())
        .unwrap_or("")
}

/// Emit a localized `@t(...)` literal as a JavaScript `game.t("en", "zh")`
/// call. The runtime `game.t(en, zh)` selects by the active language.
pub(crate) fn localized_pairs_to_js_t(pairs: &[(String, String)]) -> String {
    let en = locale_text(pairs, "en");
    let zh = locale_text(pairs, "zh");
    format!(
        "game.t({}, {})",
        serde_json::to_string(en).unwrap_or_else(|_| format!("{en:?}")),
        serde_json::to_string(zh).unwrap_or_else(|_| format!("{zh:?}")),
    )
}
