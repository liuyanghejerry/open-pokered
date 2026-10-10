# Five inner gift text sessions (batch 170)

SilphCo11F MASTER_BALL, CeladonDiner COIN_CASE, Route16FlyHouse HM02, SafariZoneSecretHouse HM03 and CeladonMart3F TM18 now preserve explicit authored PARA, final introductory PROMPT before GiveItem, complete receipt sound and the outer fresh confirmation. Silph/Safari set the received flag after sound returns; Diner/Fly/Mart set it before receipt sound. The item is already in the bag while the sound plays. The first successful talk ends with the receipt; descriptions occur only on repeat. Full bag prints refusal without a receipt sound or flag. Authored English/Chinese wording is retained.

## Actual NPC receipt comparison

Master31b1eda has only cfg(test) capture helpers added in game.rs; production prefix before session_guard_tests is byte-identical, all other tracked sources unmodified. Both sides build the same core/data/app/agent default feature union (app031fe43e). First app-only base build was rejected before recording because its feature union differed. Frozen binaries and source manifests are archived.

Bulbasaur/English, empty bag, stationary NPC homes, controlled warp to legal positions: president(7,6), diner(0,2), fly(2,4), surf(3,4), mart(16,4) above clerk16,5. Silph boss/door state is controlled; these are actual NPC interactions, not natural story traversal. Mart uses Down/A, other cases Up/A. Fixture coordinates and facing are asserted. Independent prefixes acknowledge real paragraphs/PROMPT. Gift capture creates a release when A is already held so each manual wait gets a new press; post-cue inputs retain the same policy as previous captures.

Every screenshot uses first actual receipt SoundStarted + HW150. Following261 input bits match between master and after. Each side/case is recorded twice; all PNG, selected state, controls and initial snapshots are byte-identical. Selected per-frame state is not a full runtime snapshot every frame. No absolute prefix/init/close parity claim. Master automatically closes its receipt; after retains it for the fresh outer confirmation.

masterball: master/after sound cues648/716, screenshot frames798/866.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/masterball-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/masterball-after.png)

coincase: master/after sound cues434/482, screenshot frames584/632.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/coincase-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/coincase-after.png)

fly: master/after sound cues364/362, screenshot frames514/512.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/fly-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/fly-after.png)

surf: master/after sound cues523/583, screenshot frames673/733.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/surf-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/surf-after.png)

counter: master/after sound cues277/276, screenshot frames427/426.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/counter-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-inner-gifts-170/counter-after.png)

## Validation and limits

Core2742, data262, app217 (31 ignored), agent54 default-feature tests pass. One actual NPC regression exercises five gifts x success/full bag plus all five repeat talks: intro paragraph clearing20 frames, PROMPT inventory/flag retention while idle, full PCM sound duration, B/Down discarded during sound, JSON mid-sound restore without replay, source-specific flag order, no automatic first-talk description, retained final outer wait, A hold and release, no duplicate gift on repeat. Fly first/repeat explanation and bike menu price/no-item/no-money core assertions are retained through a text-command adapter.

Several preparation/control failures are preserved, not counted as successful evidence: wrong Mart bookshelf-side position; short Down/A input discarded by field cadence; held A at short Diner paragraph needing a new edge; fixture y type mismatch; wrong assumed GBA source count and app-only base feature union. Details are in preparation-failures.json. Diagnostics include250+250+4000 PNG with initial snapshots; the earliest failed captures did not write selected per-frame JSON before their assertion, which is stated rather than reconstructed. Final owning-NPC cases and20 complete valid capture trials all pass. Capture now writes raw state/controls before its final receipt assertion. No production collision/idle/paragraph confirmation rule was altered to satisfy the fixture.

Final50 source hashes match the GBA checkout; final release build and31 unchanged original-budget metrics pass. Original evidence is source-only caller/far-text/text-opcode ASM, without new original-emulator recording. Fonts, Chinese, pinyin and dialogue layout remain excluded. raw-evidence.zip has a fully checked per-entry SHA256 manifest, final sources, tools, successful native recordings, clearly rejected diagnostics, immutable binary manifests and all failure/build/test/GBA records. Older GBA revisions are diagnostic only; final accepted GBA is a8, native a7. BikeShop and earlier draft/mainline/SAVE-CONTINUE gates remain open.
