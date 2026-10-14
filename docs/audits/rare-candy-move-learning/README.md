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
