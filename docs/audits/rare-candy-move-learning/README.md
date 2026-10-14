# Rare Candy completes move learning before evolution

The field bag discarded `use_rare_candy`'s `move_result`. A free slot gained
its new move without a learned notice; four occupied slots silently lost the
chance to learn it. The bag returned after the level message, without the
original ability-stat box or learning choices.

The correction separates Rare Candy's level/stat step from field learning.
The field continuation displays the level and stats, processes each eligible
move, then runs the existing evolution actor and consumes one item. Bulk core
callers retain their immediate-learning contract. Other bag items and the
battle move-learning flow are unchanged.

The continuation includes free-slot notices, the four-move question, selecting
an old move, declining, cancelling the selection, confirming abandonment,
rejecting abandonment, and the HM deletion guard. HM refusal returns to cursor
zero as the original does. Existing PP and PP Ups survive; only a replaced
slot receives the new move's full PP and zero PP Ups. Learned notices wait for
the learning sound before acknowledgement. Evolution cancellation still
consumes one candy, and an evolution's newly eligible move receives a notice
or replacement prompt without another level increase or evolution.

## Original actual menu evidence

Reference source is `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, retail Red
ROM SHA-1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`, PyBoy 2.8.1.
`engine/items/item_effects.asm` `.useRareCandy` performs stats/HP calculation,
party redraw, `PrintStatsBox`, wait, `LearnMoveFromLevelUp`, `TryEvolvingMon`,
and finally `RemoveUsedItem`. `engine/pokemon/learn_move.asm` provides the
replacement/abandonment questions, cancellation and `IsMoveHM` retry loop.

Each constructed initial state contains a healthy level-12 Bulbasaur, DV
bytes `9a78`, two candies, 3000 cash, and a healthy second Bulbasaur. Free-slot
cases know Tackle/Growl; full cases know Tackle/Growl/Leech Seed/Cut. PP starts
at `[35,40,0,0]` or `[35,40,10,30]`, with zero PP Ups. Construction ends before
the saved-state/input boundary. Validation uses actual START / ITEM / USE /
party / learning buttons and ordinary SAVE/YES. It makes no state writes in
the validation window and does not bootstrap into an item routine.

All four cases repeat twice. Source records, construction hashes, actual
inputs, final party bytes, image hashes, valid SRAM checksums and saved money
are archived in `source-verification.json`.

| Case | Result after learning and ordinary SAVE | Source checksum |
| --- | --- | --- |
| Free slot | Tackle/Growl/Vine Whip; PP `[35,40,10,0]` | 186 |
| Replace Growl | Tackle/Vine Whip/Leech Seed/Cut; PP `[35,10,10,30]` | 130 |
| Decline, confirm abandonment | Original four moves and PP | 30 |
| Attempt to delete Cut, retry Growl | HM remains; Growl becomes Vine Whip | 85 |

Every case reaches level 13 and HP 37/37, with stats `[37,20,20,18,23]`,
consumes one candy, preserves the healthy partner's 44 party bytes, and saves
3000 cash. At learned/choice/refusal screens, the item is still present twice.
The source learned notice is frame 818 in the free-slot case; the full-slot
question is frame 1428. These are source observations, not native frame targets.

## Matched native before and after

Before is the frozen application built from master `1b5f69ae`. The subsequent
master `72f9d593` production difference is PR231's removal of the HP-zero
Rare Candy guard; it does not change these four healthy cases. After is built
from `e196942e`, including that merged fix and the deferred field flow.
Executable hashes and source identities are in `replay-comparison.json`.
The review tree subsequently merges master `6a0ef88b`; its only additional
production change is vitamin success-message wording, outside these Rare
Candy cases. Tests and captures above retain their real frozen `e196942e`
identity; they are not relabelled as a post-merge build.

The same initial fixtures and real input prefix produce paired level/stat
screens at frame 640 and first-acknowledgement screens at frame 676. Before
returns directly to the bag at 676. After shows the free-slot learned notice
or starts the four-slot learning dialogue. Subsequent inputs necessarily
differ between builds because before lacks the continuation. Each complete
case repeats twice with identical actions, observed party fields and PNG
bytes. Each run copies the frozen executable into its own directory; sidecars
are absent before boot, live flags are recorded, and no state is edited after
boot. Native persistence is verified through ordinary SAVE/YES, not debug SAVE.

All sixteen native runs complete. Final after SRAM checksums are 254 (free),
172 (replacement), 159 (abandonment) and 169 (HM retry). The after results
match the source's level, current/max HP, stats, StatExp, moves, PP and PP Ups.
One candy is consumed; the partner and money remain unchanged. Of the first
member's 44 stored party bytes, 42 agree with the source. Byte 3 is the boxed
level (source still 12, native writes 13); byte 7 is catch rate (45 versus 0).
Those existing serialization differences are recorded explicitly and are not
part of this correction. This is not a complete save-format parity claim.

The stat box starts one native tile below the original position to retain
current/max HP readability in the existing native party layout. A framebuffer
regression verifies that the selected HP-number pixels remain unchanged.
Fonts, dialogue layout and original animation cadence are outside this audit.
The first after capture set used the higher box and was superseded; final
screenshots all use the rebuilt application with the readable HP row.

Paired PNGs and additional choice/forget/HM/source references are under
`docs/screenshots/rare-candy-move-learning/`. The PR must use absolute raw URLs
pinned to its branch for before/after images.

## Validation and replay

Nine scoped Rare Candy core tests pass. Nine application tests pass both with
`debug-server` and with default features. They exercise real bag inputs, spent
PP and nonzero PP Ups, free/full slots, choice/abandon/cancel/HM paths, HP-zero
revival followed by pending learning, real evolution cancellation and a real
Magikarp-to-Gyarados movie followed by the Bite notice. A final debug-server
application builds, and fresh m01–m06 reaches OaksLab `(5,6)` at frame 12227,
overworld with one party member and no active effect.

The complete application suite reports **247 passed, 1 failed, 38 ignored**.
The existing debug test
`skip_dialogue_stops_at_gift_question_without_selecting_yes` also fails in an
untouched master `1b5f69ae` checkout. Its isolated setup and failure are retained
separately; this correction does not fix or ignore it, and the full suite is
not reported as passing. Source menu captures cover the four level-12 learning
cases; source evolution movie/cancellation was not captured in this audit.

The reusable fixture and native ordinary-menu replay are included here:

```bash
python3 docs/audits/rare-candy-move-learning/replay.py \
  --binary /absolute/path/to/frozen/pokered-app \
  --kind after --output /absolute/path/to/new-evidence-directory
```

Use `--kind before` for the baseline. The application must include
`--features debug-server`; set any platform library search path required by
that build before invoking the script. The script refuses to reuse a case
run directory and retains inputs, snapshots, flags, PNGs and SRAM readback.
It does not build or mutate source code. The ordinary SAVE input checks are
limited to these constructed cases; they do not claim a complete playthrough.


## Snapshot restore and player soft reset follow-up

The new blocking continuation also needs to be dropped when its runtime is abandoned. An old overworld `save_state` slot restored during the learned notice previously replaced SaveData but kept the old RareCandy flow. Opening POKEMON and pressing A could consume a candy without the restored level-12 mon leveling up. Desktop soft reset had the same new-flow leak: actual full-slot replacement input, a held A+B+Start+Select chord for 16 frames, real Title/CONTINUE and the party menu resumed an abandoned prompt and consumed a candy while the mon stayed level 12.

Restore now clears this continuation after the existing movie rejection; the actual soft-reset entry clears it before reload/replacement. Capture still rejects party menus and active movies. Menu continuations were never part of the existing Overworld/Battle snapshot contract; no new menu snapshot format is introduced. Existing unrelated pending-item/evolution fields are outside this correction.

Two added application tests use real item/menu input, without constructing a RareCandyFlow. One saves an overworld slot, reaches the learned notice with two candies, restores the old slot, opens the normal party action menu and uses a fresh candy. The other prepares a private committed SRAM **before input** with `external_saves=false`, reaches a full-slot replacement prompt, holds the real reset chord, selects and acknowledges CONTINUE, then verifies normal party reentry and another actual replacement. The private SRAM stays byte-identical; this is not an ordinary SAVE claim. Both final runs preserve level12/old moves/two candies after restoration, then reach level13/VineWhip/one candy after intentional reuse.

All **11** scoped application tests pass with default features and with `debug-server`; a new debug application builds. The old nine gameplay tests still pass. The frozen older executable reproduces the snapshot loss, and the pre-soft-reset-fix desktop test fails specifically on the observed wrong quantity (1 instead of 2); the corrected test passes. Same-input party screenshots match at snapshot-reentry frame256 and soft-reset-reentry frame841. Initial harness failures at CONTINUE input gates, a test compile error, and an acknowledgement before learning SFX finished are retained and excluded from the gameplay proof.

`lifecycle-verification.json` records this separate build/source identity and bounded restore/reset evidence. Earlier source/native ordinary SAVE captures and m01–m06 retain their actual frozen `e196942e` build identity and were not recaptured or relabelled. The original learning implementation is unchanged by the lifecycle cleanup. No new full application-suite, original reset/animation parity or local GBA runtime PASS is claimed for this follow-up.

发布前合并 master `5941a7f1` 的 Python 导航工具修复。该更新未改变本次已验证的生产游戏代码；普通 SAVE、生命周期和 m06 各自仍使用其记录中的冻结构建，不改标成发布头。

## GBA performance follow-up

PR238's first GBA performance run exceeded the existing Oak draw budget
(3093 versus 3004.9 timer ticks). With the same pinned nightly and mGBA0.10.5,
master906 measured 2958 and the unchanged PR measured 3092, both with 307
redraws. Boxing the cold flow, shrinking the render key, and outlining Party
renderers did not restore the budget; those experiments are not shipped.

The selected change keeps Oak's substituted Unicode character prefixes and
typewriter cadence, while truncating the already-owned UTF-8 lines, avoiding
character vectors and the first-line clone, and constructing line 2 only when
it becomes visible. Token-free lines take an ordinary string-copy path. The
final candidate includes mastere481. Its seven GBA scenario gates pass, and a
second run of the same ROM has identical metrics: Oak 2994 ticks with the
unchanged 3004.9 limit, 600 samples and 307 redraws. No baseline, thresholds,
frame/input cadence, font or layout changes.

Final-source validation: 15 Oak core tests (including fixed expected Unicode,
empty-name, repeated-token and boundary prefixes), the full 2859 core tests,
and all 11 Candy application tests under both default and debug-server
features pass. The debug client builds. Actual snapshot restore, ordinary
POKEMON/A entry and a fresh Candy reuse pass; this uses agent snapshots, not
SRAM SAVE. Six real soft-reset/CONTINUE/reuse captures and seven selected Oak
CLI captures are byte-identical to the prior lifecycle build's captures.

`gba-performance-verification.json` records the experiment identities, final
metrics, repeat, source/build hashes and scoped native checks. This is a local
performance gate result, not a claim that the newly pushed GitHub CI has
finished, or an original animation/frame-parity claim. The earlier original
8 SAVE/native 16 SAVE evidence, screenshots, m01–m06 and full-application
known-failure result retain their historical build identities.
