# Bill and Copycat text sessions (batch 168)

The source has three distinct inner waits: PROMPT uses ProtectedDelay3 before ManualTextScroll; TX_PROMPT_BUTTON has an arrow and no protected delay; TX_WAIT_BUTTON has neither arrow nor protected delay. New typed commands waitFieldPromptButton and waitFieldButton preserve those opcode distinctions. closeFieldText models the explicit outer-skip path: hold A until release, without requiring another fresh A/B. All are registered through typed command conversion, native/Boa APIs, sorted capability catalog and strict no-state analyzer list. Existing PROMPT and ordinary ground pickup behavior stays intact.

Bill's received-ticket text now retains the window and waits for its inner prompt button before setting EVENT_GOT_SS_TICKET or changing Cerulean guards, then prints the explanation and runs the ordinary outer confirmation. Copycat keeps POKE_DOLL and EVENT_GOT_TM31 unset while the received TM31 sound, leading PARA and explanation run. Only the final no-arrow inner wait acknowledgement permits doll removal and flag setting. The unreceived branch skips the outer confirmation but still holds A until release, including no-doll and bag-full cases. Repeat talks retain their ordinary outer confirmation. Authored wording is preserved with explicit paragraph commands.

## Receipt comparison

Production master is 31b1eda; only cfg(test) helpers were added for capture (production prefix checked byte-identical). Bulbasaur/English, Bill player (4,5) below restored human Bill (4,4), Copycat player (4,4) below authored NPC (4,3), PCM audio, and stationary NPC homes. Bill's restored state/visibility is controlled via live flags, not a natural subplot traversal; Copycat starts with a POKE_DOLL. Both sides use real Up/A interaction. Initial snapshots and raw controls are archived.

Independent prefixes reach the first actual reward SoundStarted cue. The next 261 input bits are identical, and screenshots use cue-relative HW150. Master/after Bill cues584/606, frames734/756; Copycat cues666/782, frames816/932. Each side/case is repeated twice, every PNG/initial snapshot/selected state/input file byte-identical. No absolute prefix/init/closure timing parity claim.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/bill-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/bill-after.png)

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/copycat-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/copycat-after.png)

At HW150 after the cue, master Bill has already set the ticket flag and started the explanation; after still awaits the receipt prompt. Master Copycat has already removed the doll/set its flag; after still retains the receipt, doll and unset flag.

## Copycat final explanation wait

Both recordings continue actual independent paragraph acknowledgements: after acknowledges its second PARA at sound-cue+600/+601, while master's automatic text pagination requires no corresponding second input. The screenshot is HW40 after reaching the same completed final explanation waiting phase: master cue920/frame960, after cue1475/frame1515. The following 41 input bits are all zero on both sides; each extended recording is repeated twice with all files byte-identical. This is phase-relative, not a matched absolute input prefix. The old outer arrow is visible; the new inner TX_WAIT_BUTTON has no arrow. After still owns the doll and has not set the flag.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/copycat-final-wait-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-bill-copycat-168/copycat-final-wait-after.png)

## Validation and scope

Default-feature suites pass: core2742, data262, app215 (31 ignored), agent54. Actual NPC tests cover full PCM item/key jingles, discarded B/Down pulses, JSON mid-sound and mid-no-arrow-wait restore, original item/flag/guard order, 20-frame paragraph clearing, idle retained waits, A-hold/release, successful/repeat/full-bag/no-doll branches. The opcode test proves first-sample acknowledgement without delay and separate hold-A close behavior; its initial idle-VM fixture failure is preserved and explained in verification.json. Production idle cleanup was not changed.

Forty-three copied source SHA256s match the final GBA perf checkout; release build and all31 unchanged original-budget metrics pass. CI additionally checks pacing/slow-cart/opening scenarios and must finish on the final head. Original evidence for this batch is complete ASM source only, not new emulator recording. Fonts/Chinese/pinyin/dialogue layout remain outside scope; no full pixels/waveforms/CPU-boundary parity claim.

raw-evidence.zip contains 12 complete native recordings, controls/initial snapshots/selected per-frame state, source files, original ASM, tools, failure/build/test/GBA logs, immutable binary manifests, and a per-entry SHA256 manifest. Selected per-frame state is not the full runtime snapshot every frame. Nine remaining gift migrations and previous PR draft/final-mainline/SAVE-CONTINUE gates remain open; see remaining-gift-source-audit.json.
