# TM35 gift session (batch 167)

CinnabarLabMetronomeRoom scientist 1 now keeps the entire reward interaction in its inner text session: introductory PARA, PROMPT acknowledgement before GiveItem, full GetItem1 fanfare before EVENT_GOT_TM35, then an outer fresh A/B acknowledgement. Holding A keeps the final window until release. Full bags preserve the bag and flag and produce no receipt/fanfare; repeat talks show the METRONOME explanation without another gift. Existing dialogue wording is preserved.

Original evidence is source-only: scripts/CinnabarLabMetronomeRoom.asm, text/CinnabarLabMetronomeRoom.asm, and home/text.asm. This batch adds no original-emulator timing claim.

## Before / after

Both fixtures use Bulbasaur, English, player (7,3) below authored scientist (7,2), actual Up/A interaction, PCM audio, and stationary NPC homes. Before is production master 31b1eda (only cfg(test) helpers added); after is this PR. Each side is recorded twice with all PNGs, initial snapshots, selected per-frame state, audio channels, and input bits byte-identical. Independent interaction prefixes reach the first reward SoundStarted cue; the following 261 input bits are identical. Screens show cue-relative HW150: before absolute frame397, after absolute frame396. The difference demonstrates master auto-dismissal versus the retained receipt/fresh outer confirmation. It does not assert absolute prefix/init/close timing equality.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-metronome-gift-167/metronome-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-metronome-gift-167/metronome-after.png)

## Validation and remaining audit

Default-feature suites: core2741, data262, app213 (31 ignored), agent54; all passed. The new actual NPC regression covers intro gating, bag-full and repeat behavior, exact independent PCM jingle duration, B/Down during sound, JSON snapshot restore during sound without replay, source flag timing, idle retained receipt, and A-hold/release closure. This app binary does not include debug-server-only cases; full CI remains required. Forty copied source SHA256s match the GBA perf checkout; all 31 metrics pass the unchanged original baseline budget.

raw-evidence.zip contains the four complete native recordings, source files, original source audit files, build/test/GBA logs and SHA256 manifest. See verification.json for archive checksum and cue-summary.json for frame-level details. Selected per-frame state is not a full runtime snapshot per frame.

remaining-gift-source-audit.json records caller and far-text flow for all twelve remaining candidates. Only TM35 is migrated in this batch. Bill requires TX_PROMPT_BUTTON without ProtectedDelay3; Copycat additionally sets outer-skip-wait and delays doll removal/flag until explicit no-arrow inner WAIT_BUTTON. Shared aides print the success promise and wait before GiveItem even when the bag is full. These eleven migrations, previous PR draft gates, and final mainline/SAVE-CONTINUE checks remain open.
