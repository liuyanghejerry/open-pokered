# Inner PrintText return to choices

Original inner PrintText ending in DONE returns after the final character delay without a new A/B acknowledgement. The native generic ShowDialogue required that additional press. This change adds an explicit PrintFieldText script capability and effect for the inner-print call sites; regular ShowDialogue continues to require acknowledgement. The retained question remains owned by the following choice.

Covered call sites: Safari information worker, Museum ticket price question through both front entrance paths, Museum back-counter AMBER question, and shared gift nickname prompt. Museum money is displayed before printing the ticket text, matching its assembly call order. Native interpreter, Boa host, capability catalog and serialized typed command carry the same explicit effect.

## Actual master before / current candidate after

Master is 6d99bd6 (#152 already merged); candidate source head is 733cacc. Both builds use separate initially empty targets. They run the already committed test-only recorder from #153: real Continue from the same controlled original SRAM, end fixture Safari game, warp to its valid worker position, Medium=3. A at frame0, release1, no further keys; screenshot frame130 on both sides. This is a controlled fixture, not natural traversal. Master production is unchanged; the already committed recorder is unchanged on both sides. The candidate carries its ten intentional source changes. All ten candidate source hashes match the frozen passing build.

Before stays at the question waiting for another press. After returns automatically to YES/NO with YES selected and the final question retained. Both sides record145 full hardware frames twice (580PNG total), byte-exact PNG and full raw snapshot repeats; every frame checks retained against full drawing pixel-for-pixel. Input bits match across sides.

Historical original160 idle recordings and primary assembly are included as original-only reference. First glyph24; YesNoChoice callback114 (+90); viewed complete menu115 (+91). Native first glyph2, ShowChoice effect92 (+90), constructed visible menu93 (+91). The original/native entry, font transmission and CPU/PPU phase are different and not claimed equal. An initial verifier incorrectly expected the visible menu at the effect-entry time; rejected verification is preserved, raw recordings were not changed or rerun.

## Verification

Clean candidate a3 core2716, app195 with19 ignored capture helpers, data262: all pass. Dedicated master a3 build passes and its ignored capture helper runs explicitly twice. Owner regressions include automatic Safari choice with final-wait snapshot restore, gift nickname without premature answer, and Museum early money display. Existing normal-dialogue and choice lifecycle regressions remain enabled.

The first attempt with the shared target failed capability validation despite the current source catalog containing printFieldText: its cached build-script used the old catalog. Failure log is retained and excluded. Both authoritative builds use separate clean targets. Evidence archive includes full logs, complete raw records, primary sources and frozen executable/source hashes; it excludes executable binaries, ROMs and SRAM/state data. Broader text-session initialization/closing, other inner PrintText call sites, map presentation and timing differences remain separate audits.

![前](before.png)
![后](after.png)
