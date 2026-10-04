# Presentation split evidence

Base: `cb131bbde9168cb7486aeff0d220cc9b21849b30` (includes #110). Production capture: `7ecdf9b`. Source scope: #111 `0e6c005`.

Both checkouts run the identical archived `capture.rs` against their own production libraries at 160×144. Copy it temporarily to `crates/pokered-app/examples/capture_split_presentation.rs` and run:

```sh
cargo run -p pokered-app --features debug-server --example capture_split_presentation -- /tmp/shots before
```

Use `after` on the PR checkout. Remove the temporary example afterward. Phase-relative credits frames are asserted; other initial states, inputs, version and frame counts are explicit in the fixture. These captures use the Fusion Pixel font present in each checkout. Pokémon naming capture uses a neutral species value supported by both base and PR; species selection has separate unit coverage.

The four `zh-*` pairs come from the unchanged `visual_verify_zh_descriptions::capture_zh_descriptions` ignored capture test, run on each checkout, and exercise the #110 pagination paths. `captures.json` records all image hashes. PNG pairs are in `docs/screenshots/fidelity-presentation-split/`.

Validation: core + UI preview 3014 passed; renderer 395; UI 102; app lib/bin/integration executions 331 passed, 10 ignored; TUI and UI-preview cargo check; native debug-server build; fresh playthrough m01–m02 passed (2045 frames). These are test executions, not a unique-test aggregate. Later full playthrough is limited by the independently reproduced master m03 stair-navigation failure; no full-game completion is claimed.

## Review corrections

The final `mart-buy-En`, `mart-buy-Zh` and `trainer-card` after images were regenerated with the identical archived fixture after review. The preceding PR images at `f5b9348` remain as `*-review-before.png`. Prices now share item baselines and use measured widths for right alignment; the full ten-pixel BADGES heading area is cleared before text. The original master images remain unchanged.

GBA CI reported Oak update time 167 → 193 ticks (limit 192.1). `TextPage::total_chars` now counts placeholder substitutions without allocating strings each tick; a Unicode/repeated-placeholder test compares against actual substitution. The performance baseline and gate are unchanged. Local core 2958 and UI 102 tests passed; TUI check passed. GBA timing awaits the new CI run.
