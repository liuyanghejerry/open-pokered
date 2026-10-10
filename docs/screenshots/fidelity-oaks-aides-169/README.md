# Oaks aides text sessions (batch 169)

Route2Gate, Route11Gate2F and Route15Gate2F now follow shared OaksAide source flow. The three intro paragraphs end in DONE and go directly to YES/NO. The successful count report has a PARA before Here you go! and a PROMPT before GiveItem, including full-bag cases. The receipt plays SFX_GET_ITEM_1 and returns automatically; only then does the caller set its flag and print the separate description. Final outer confirmation remains. Low-count, NO and repeat paragraphs use the original paragraph/outer-wait sequence. Existing authored English/Chinese wording is preserved.

## Actual interaction comparisons

Production master31b1eda was frozen with cfg(test) capture helpers only; its prefix before session_guard_tests was checked byte-identical. Bulbasaur/English, empty bag, real Pokedex owned bits for10/30/50, stationary NPC homes; player coordinates (1,5)/(2,7)/(4,3) under the authored aides. Fixtures warp and control owned counts; these are actual Up/A interactions, not natural route traversal.

Independent acknowledgement prefixes reach each semantic cue. Receipt pictures are first SoundStarted + HW150, with identical261 post-cue input bits on each side. Here you go! paragraph pictures use acknowledgement + HW10, with identical ACK plus10 idle input bits. After has BlankDelay remaining10 and prints its first glyph at HW20; master already prints Here. Paragraph source supports the20-frame delay, with no new original-emulator recording. Each side/case is recorded twice; all raw PNG, selected state, input and initial snapshot files are byte-identical. Final a3 recordings also match a2 byte-for-byte.

aide2: receipt cues 729/780; paragraph acknowledgement cues 654/680.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide2-paragraph-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide2-paragraph-after.png)

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide2-receipt-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide2-receipt-after.png)

aide11: receipt cues 777/828; paragraph acknowledgement cues 672/698.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide11-paragraph-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide11-paragraph-after.png)

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide11-receipt-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide11-receipt-after.png)

aide15: receipt cues 759/810; paragraph acknowledgement cues 663/689.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide15-paragraph-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide15-paragraph-after.png)

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide15-receipt-before.png)

![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-oaks-aides-169/aide15-receipt-after.png)

## Validation and scope

Core2742, data262, app216 (31 ignored), agent54 pass with default features. Actual owning-NPC tests cover successful/full-bag/low-count/NO and repeat branches, promise confirmation before inventory changes, full PCM73-frame sound, B/Down pulses discarded during the sound, JSON mid-sound restore without replay, flag update after sound, automatic separate description, retained final wait and A-hold/release close. Count/flag tests retain both English/Chinese and thresholds0, required-1, required, required+1,151.

The final46 copied source hashes match the GBA performance checkout. Release build and all31 original-budget metrics pass; baseline unchanged. Previous committed head bafcb000 had22 checks completed (21 success, editor skipped), but new final-head CI remains a separate requirement. First app fixture and old core assertion adapter failures are retained in the archive and explained in verification.json.

raw-evidence.zip includes12 complete native recordings, initial snapshots, selected per-frame state (not every full runtime snapshot), original ASM, final sources, tools, immutable binary manifests and build/test/failure/GBA logs. Its per-entry SHA256 manifest was fully verified. Six other gift handlers and earlier PR draft/full-mainline/SAVE-CONTINUE gates remain open. Fonts, Chinese, pinyin and dialogue layout are excluded; screenshots are cue-relative evidence, without whole-pixel or absolute entry-timing parity claims.
