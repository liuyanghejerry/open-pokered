# NPC gift receipt followup

> 存储说明：逐帧录制、压缩日志和配套运行报告已移出版本库。下文的录制文件名及哈希仅描述历史验证；复现输出请放在 `.task-tmp/` 或仓库之外，完成后清理。见[审计材料约定](../README.md)。


Coin Case, TM18, HM02 and Master Ball receipts previously disappeared after the fanfare. They now remain until the player's final acknowledgement. Master Ball commits its reward flag after the real fanfare; the other three preserve their original early flags. Intro/repeat paragraphs and full-bag exits follow the original callers.

This patch is based on master 95152068, which already contains the TM35/HM03 text API. Production changes are limited to four scene files; two test files cover the embedded callers.

The original and candidate success/full-bag controls, repeated talk, real sequencer PCM, held/released input and separate-process CONTINUE are recorded. Prepared saves use documented original-game preparation and SAVE. Native persistence uses debug SAVE after formal interaction, then ordinary CONTINUE in a new process; it is not a manual SAVE-menu test.

All eight screenshots were recaptured twice from the current master binary and the new branch binary. Each pair uses the same prepared save, initial screen and absolute host frame. Dialogue confirmations are adaptive per side; their full input timelines are not claimed identical. Captures match the historical images byte for byte. See ../../screenshots/field-gift-receipts-followup/.

The 2369-file source comparison against the frozen recording/fresh binary found only the documented FLY test assertion update and the already-merged PR424 agent presentation-command classification. The new binary has its own tests and screenshot replays. Binary identities and historical sources are kept distinct in verification.json.

Local checks: core 3290, application 409 (40 existing ignored), data/Boa 479 (1 existing ignored), agent 64, ARM release check. The initial ARM check hit Cargo workspace discovery because the worktree was nested in another workspace; moving the same unmodified source outside that workspace passed. The original failed command is preserved.

The frozen four-gift gameplay binary completed NEW GAME -> champion -> Hall of Fame -> credits -> ending save -> independent CONTINUE in 7874.9 seconds, with unchanged source and binary. A normal champion loss triggered the existing league retry before success. The separate process restored Pallet Town, eight badges and one Hall of Fame record. See fresh-run.json and the compressed raw m49 artifact. This is not relabeled as a fresh run of the newer branch binary; their two source differences and separate validation are above. The earlier missing-driver-input m11 failure remains historical, not a game failure or successful clear. PR CI is a separate gate.
