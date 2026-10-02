# Fixed-frame fidelity captures

Base: `72ff719`. Reference source: pret/pokered `fbcf7d0`. These are software-renderer before/after captures of the same deterministic inputs and frame count, **not new original-ROM frame comparisons**.

The CLI capture pairs use `pokered-app screenshot --screen SCREEN --frames N --lang en`; the source-compatible fixture in `crates/pokered-app/examples/fidelity_visual_capture.rs` provides initialized evolution, party, Pokédex, trade, nickname, Blue-title and Chinese-diploma states. The nickname input is LAPRAS; the fix preserves this species metadata, which the base discarded. Party frames 6/17/33 exercise the original GB green/yellow/red cadence. Gengar is six tiles wide and exposes whole-buffer mirrored padding; Raichu is seven tiles wide and intentionally has no picture difference.

The first after set includes the visual fixes before the separate English-font change. Trainer-card and diploma after images must be refreshed from the final integrated binary to include the corrected trainer_info asset category. The CLI's uninitialized elevator screen is blank; its invalid pair was removed and the fixture now initializes five real floors.

`before-battle-mimic-menu.png` belongs to the separate battle fix and shows the base's copied Growl/5-PP menu. Its after capture and the EXP-share pair are supplied from the integrated battle implementation. All other named pairs have identical capture frame counts; timing fixes can therefore legitimately put them in different phases.

Coverage: copyright and Game Freak custom tiles/setup/palette; title signed clipping/version/ball timing; ordinary-GB evolution; Nidorino and trade front-picture padding/mirroring; Pokédex six-column mirrored padding; all ten party icon kinds and three HP speeds; naming; trainer card; diploma (English and Chinese); Hall of Fame scene/fades; credits four palette steps and THE END.

The standalone base/fix binaries used during development are copied outside the shared Cargo target under `/workspace/onboarding/pokered-visual-{base,after}-fixture` and `/workspace/onboarding/pokered-{audit-base,visual-after}-app`. Final integration capture commands should be recorded with the resulting implementation commit.
