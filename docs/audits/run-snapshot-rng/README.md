# RUN uses the saved battle RNG

RUN previously obtained its deciding byte from the hosted global `rand::random`
facade. `set_seed` and runtime snapshots own `BattleScreen.rng`, so restoring an
identical battle and pressing the same buttons could either escape or fail.

Canonical RUN now draws lazily from that existing battle RNG. The original
`TryRunningFromBattle` (`engine/battle/core.asm:1496–1576`, reference revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`) calls `BattleRandom` only at
`.compareWithRandomValue`. Trainer refusal, automatic escape, attempt counting,
inclusive comparison and the failed-run enemy turn retain their existing rules.
The public helper accepting an injected byte remains compatible. No extra RNG,
state owner or snapshot field is introduced. Capture, Safari and disobedience
RNG calls are separate candidates and are outside this fix.

## Actual inputs and ordinary SAVE

The native fixture has a legal level-12 Rattata with paralysis and HP31, plus a
healthy Rattata with HP31; normal learnsets, money3000 and one unused Poké Flute.
The only change to the earlier fixture is clearing the bench's status before
boot. A CLI map bootstrap places the party on Route1, seed19 is set before the
first input, and ordinary walking produces a natural encounter. Each trial has
its own executable directory, save file and port; there is no in-battle editing,
forced encounter or selected catch/run outcome.

At frame976, actual menu inputs select RUN. Before every fork the runtime slot
is restored and independently reserialized, with its hash checked. Every fork
then receives exactly A1 followed by 119 neutral frames.

| Native build | Repeats × forks | Initial snapshot | First-RUN result | Distinct posthashes |
| --- | --- | --- | --- | --- |
| clean `05f6e76c` BEFORE | 2 × 12 | `dc76bdb70df6ae4c` in both | 4/8, then 3/9 escape/fail | 2 |
| `d07da73e` AFTER | 2 × 12 | same hash as BEFORE | fail in all24 | 1 |
| integrated `0b4b2813` AFTER | 2 × 12 | `de4f067de8118667` in both | fail in all24 | 1 |

The first RUN fails consistently after the fix; later ordinary attempts escape.
Only the final fork in each trial continues adaptively to ordinary START → SAVE.
This is two completed saves per build, not one save per fork. The two d07 AFTER
SRAMs and two integrated AFTER SRAMs are byte-identical, SHA256
`8a6c0a18464ea0956657db8fef33b24afa39018a51a7cc019660b5be4cc3a8a8`.
All main checksums pass. Saved HP is28/31 and status64/0. Aside from the lead's
expected HP byte, its other43 bytes and all44 bench bytes match construction.
Independent frozen-binary SAVE imports preserve money, bag, old boxes, Pokédex,
player identity, event/object flags and other recorded save fields. BEFORE and
AFTER full SRAMs are not expected to equal each other: their battle outcomes and
adaptive finishing timelines differ.

Original-ROM controls independently prepare two natural encounters, restore
four full emulator states per encounter and finish all eight branches through
ordinary SAVE. All eight first RUNs escape and full SRAMs match each other.
The source actual matrix therefore covers success only. Native seed19 and
original hardware RNG, DVs, damage, frames and whole SRAM are not equated.

## Checks and identities

Production/test commit `d07da73e279efbfe272c00967144ba790f9e56ae` was checked and
frozen before integration. Merge `0b4b2813ce71166cf4ee0d2e8f39460e01b789c0`
then integrates master `4c364111dd2a5d4e6322b13b53fbb3caa56a3d88` and has its
own builds, checks and actual controls. Historical8eb, paired05f6/d07 and latest4c
records retain their original source and binary identities.

The integrated source passed:

- Core2895, escape-flow6 and explicit debug-server app build.
- Full app default285 / debug296, with37 /39 existing ignored tests.
- Agent determinism9 in each feature configuration.
- All seven unchanged GBA performance gates twice; complete metric JSONs equal.
  Trainer update232 ticks, idle382 and Oak draw2994. These are local mGBA
  results; remote CI and full playthrough success are not claimed.

Six focused core tests cover lazy no-draw branches, inclusive odds, attempt
bonus/carry, exact owned-stream advancement, failure enemy-turn execution and
snapshot replay, including trainer/Ghost/Link behavior. One application test
uses real menu buttons and restores seeded and entropy-initialized streams for
12 repeats each. Before fixing production, both owner advancement and real
application replay regressions failed.

The current integrated debug binary SHA256 is
`76e79f833f6c2161201b90d08e0420a201c90068c7322802ad71090dc8776a6e`.
Full Rust/Cargo source manifests, test/build logs, GBA mirror manifests, ROM
hashes, inputs, API responses, PNGs, saves and emulator states are in the archive.
ROMs and executable binaries are excluded.

## Evidence and limits

`verification.json` is the compact summary. `archive.json` records the ZIP hash,
entry counts and exact metadata allowlist. `evidence.zip` deduplicates exact
content while retaining every original relative path in `entry-manifest.json`.
It includes six explicitly selected artifact directories and an explicit list
of metadata files; it does not glob shared audit-prefix metadata.

```bash
python3 extract_evidence.py evidence.zip --verify-only
python3 extract_evidence.py evidence.zip /tmp/run-snapshot-evidence
```

Retained failures and errata are separate from successful controls: historical
nativev1 missed SAVE because its START pulse was too short; an initial new test
incorrectly expected wild enemy PP34 instead of35; and a readback checker used a
numeric item ID where typed JSON returns `PokeFlute`. The corrected checks pass;
none of these drafting errors is presented as another engine bug.

This changes game logic and tests, with no renderer/layout changes. AGENTS.md's
before/after renderer screenshot requirement is therefore inapplicable. Actual
screenshots remain in the evidence archive. Runtime snapshot logic is verified;
presentation reset, complete frame/audio parity, source failure-path actual
coverage and other global RNG callers are not claimed.

已整合最新 master `f0787e19`（#268，巨石测试隔离；包含 #258 中毒与进门顺序）：core 2895 通过/0 忽略；app 296 通过/39 忽略；determinism 9 通过/0 忽略，全部 0 失败。当前受测源码 `24ee8a081d598b9d6e30b9128c7873a81a7b782c`、编译包装脚本与日志见 `master-f078-integration.json` 和独立日志包。历史原作/原生录制与 SAVE 保留各自冻结身份；本次新 head 仍需远程三个工作流全部通过。

## Integration after medicine feedback and performance fixes

The current source includes actual master `a2fa94f185c08dfb568929a022b2ca66da0e0aa4` after PR267 and PR272 merged. Prepared source `0250869ec5d6db0ee6253958c518c468dedaef46` passed core **2905**, full debug app **311** (39 ignored), and determinism **9**. After merging actual master, all 2170 compiler-source paths and every non-documentation file are byte-identical to that tested source. `parent267-master-integration.json` and its log archive preserve the original source, manifests and terminal logs. No source test, native/original input, ordinary SAVE, snapshot fork, or recording was relabeled. The earlier GBA and default-app records retain their historical measured sources; the current public head must pass all three latest remote workflows before merge.
