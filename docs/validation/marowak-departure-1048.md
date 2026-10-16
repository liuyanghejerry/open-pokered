# Tower 6F Marowak departure caller

The mother-identification window now stays open while the real Marowak cry
finishes, then for 30 further frames, before the calmed/departed text replaces it.
Only this caller opts into window retention. Existing `playCry` calls remain
nonblocking; the new `waitSound()` command waits on the existing logical SFX
sequencer input. Text, fonts and layout are unchanged.

## Original caller

At original source commit `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
[`PokemonTower6FMarowakDepartedText`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/PokemonTower6F.asm#L129)
prints the mother text, calls `PlayCry(RESTLESS_SOUL)`, waits for sound completion,
delays 30 frames, and prints the departure text. The
[mother text ends in `done`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/text/PokemonTower6F.asm#L1).
[`DoneText`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/text.asm#L221)
returns to that caller; the outer
[`DisplayTextID`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/text_script.asm#L87)
waits/closes after the caller returns. Thus the mother window must survive the
cry and delay without a final extra A acknowledgement.

`printFieldText(text, true)` explicitly owns this retained window; the optional
argument defaults to false in native, Boa and typed command conversion. The bit
round-trips through snapshots with an old-snapshot default of false. Subsequent
text replaces its ownership; script completion, reload/cancellation, warp,
connection, save loading and soft reset clear it. No cry duration is hardcoded.

## Actual source and checks

This development validation uses actual master base
`f35d9f4de826a51b9e60a91333aae610b299e4f0` plus Tower-only production commit
`c6a4c940a9e2cd7bea257709fffca84ef2abf8b4`. App fixture-only child
`e709eff5a853176f729c30a277adbb11654fb0b7` changes nine lines of leading
whitespace inside the new test DSL string. Production, core/data tests and map
bytes remain identical to c6a4. The temporary four-PR combination was not used as
this validation's master base.

Follow-up source commit `be89c290148fd2f00c6f13ffb992e4b3370c1e3a` adds one
production call at the direct `soft_reset` entry plus an App regression for an
unreadable save destination. During evidence review, an earlier agent summary
was found to have mistaken the existing `load_from_path` cleanup for reset-entry
cleanup. The direct entry is now explicit. Its real Game/PCM wait test and the
new missing-save direct-reset test both passed under be89; its debug CLI built
successfully. The historical e709 native pair is retained with its actual
identity and contains no reset. It is not claimed as reset proof or a be89 rerun.

| Actual command scope | Tested source | Result |
| --- | --- | --- |
| Full `cargo test -p pokered-core` | c6a4 | 3405 passed: 2993 unit + 412 integration; 0 failed, 1 doc test ignored |
| New core caller/order/legacy snapshot filters | c6a4 | 2 + 1 + 1 passed, also included in full core |
| Data optional command argument | c6a4 | 1 passed |
| Data `script_api::tests::`, feature `script-boa` | c6a4 | 13 passed, including new typed suspension and registrar capability check |
| App `field_cry_wait_fidelity_tests`, `debug-server` | e709 | 1 passed using real Game request drain and logical PCM under `no_audio=true` |
| Historical debug CLI build | e709 | exit 0 |
| App `field_cry_wait_fidelity_tests`, `debug-server` | be89 | 2 passed: original real PCM wait plus missing-save direct-reset cache release; 0 failed |
| Debug CLI build | be89 | exit 0 |

All builds used offline/locked, one Cargo job, no incremental builds and this
worktree's dedicated target. Game/core inputs use the normal public engine pin
`e533a3d23a0cdbed1524a1ac91184733da4a6f7e`; 702 GFX file hashes were frozen.
The full core run is attributed to c6a4, not claimed as rerun under e709.

The first App test attempt under c6a4 stopped at DSL parsing because its new
raw-string fixture had invalid 12/16/20-space indentation. Its exit 101 and raw
log remain preserved. The e709 child only normalizes that literal to 0/2/4;
all PCM/window/30-frame assertions are unchanged. No production adjustment was
made to satisfy a failing assertion.

## Ordinary-input native comparison

The committed [fixture and inputs](fixtures/marowak-departure-1048/provenance.json)
make this case explicit: a preboot SaveData JSON at Tower 6F `(10,17)`, the three
Channelers already beaten, Ghost Marowak not beaten, one standard Bulbasaur level
30 at HP 73/73 and PP 35/40/0/0, and one Poké Doll as the sole bag item. The
fixture was built offline with SaveBuilder from an existing engine-exported
canonical save. The complete changed leaves and byte hashes are recorded.

Both processes cold-boot normally with `--snapshot`, `--headless --speed 0`,
`--seed 42 --lang en --no-audio` and an explicit frozen `--scripts-dir`. Ordinary
buttons pass the splash/language/title/Continue screens, walk up onto the real
coordinate trigger, acknowledge the real ghost encounter, and select ITEM →
POKé DOLL. No live setters, skip helpers, warp, encounter injection, runtime RNG
changes, snapshot restore, or automatic retries were used. No physical SAVE was
performed; a launch-time save destination is not represented as an executed Save.

Before uses immutable actual a1 CLI
`a1efdac02d41946002e35051e4f8412264221afc` (SHA-256
`a8747aac9cd73ea64e855782e40353699b41070a69e7ca8f300e235c4e7154a2`).
Its Rust, engine and Tower.scene bytes equal f35; its **full compiler roster is
not equal**: Daycare.scene is the sole difference. Explicit runtime frozen f35
maps override that scene input. This does not relabel its compiled identity or
cold-boot provenance. After uses actually built e709 CLI (SHA-256
`ecf5eecb11a6a0bec4b0addc4339197700d5fd0528c30c7c837628f67f07a640`)
and production-identical frozen c6a4 maps. These are development matched captures
on the stated f35 base, not relabeled captures of a later master.

The before discovery yielded a fixed 1945-frame button vector. After replayed it
once unchanged. All 1946 public state observations are exactly equal through
frame 1597. Frame 1598 first differs at the intended mother command
`ShowDialogue` → `PrintFieldText`. Screen/map/position, party, battle HP/PP,
inventory, money, badges and evaluation remain equal throughout the full pair.
Public observations are not claimed as direct RNG-state equality.

| Absolute frame after update | Before | After |
| --- | --- | --- |
| 1677 | Mother text waits for final A | Mother DONE has installed PlayCry; full window retained |
| 1678–1719 | Proceeds to calmed text immediately after A | WaitSound; real logical PCM cry id 27 is playing, mother window retained |
| 1702 | Calmed text already appearing while cry still plays | Complete mother text remains visible |
| 1720–1749 | Calmed text awaiting input | Exactly 30 Delay states, remaining 30 down to 1, SFX no longer playing |
| 1750 / 1751 | Calmed dialogue | Next ShowDialogue installed / next text pending; old window ownership replaced |
| 1945 | Script idle, window closed | Script idle, window closed |

Screenshots at frame 1702 show the cry interval; frame 1722 shows the subsequent
30-frame pause (remaining 28). Captures were inspected at their original 160×144
size and copied without image manipulation. Next normal A presses page through
and close the departure text in both runs. The after final observation proves
script idle/window cleared; no separate after `get_flags`/`export_fixture` query
was performed.

Raw pair analysis SHA-256:
`96ed93496237d27811dd8a8a4eca82e80b261e37b6abb58664f71946418ecea3`.
Full raw RPC, state streams, command argv, process terminal receipts, source
freezes and initial failure are preserved in the session's audit records; the
committed provenance records their identities and the shared inputs.

The be89 reset regression reaches real logical PCM `WaitSound` with retained
owner/cache and no pending dialogue, then directly calls `game.soft_reset()`
with a nonexistent save path. It asserts TitleScreen, owner false, cached text
None and no displayed window. The normal PCM/30-frame assertions are unchanged.
Actual be89 CLI SHA-256:
`6ba401d3ad8ef41ad67d2bd2f09c23d03698e7b5b4678f62a2a92de974f3f06a`.
Its actual two-test/build result SHA-256 is
`e0e550adea05fc6599e50b6849ecbfc17589378326bb6993fc00bccde2b18c68`;
source/engine/GFX before and after are identical (freeze SHA-256
`b2920b3362520009561c3b3ac124c574c5f86b1ad5700a61bdef4d1dff1775d9`).
No new full-core or ordinary-input native run is attributed to be89.
