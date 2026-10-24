# Status gate captions

Original sleep, freeze and full-paralysis gates print their action's reason
when the actor is stopped. Sleep countdown prints fast asleep, while the wake
turn prints woke up and still forfeits the move. See original revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/core.asm:3332-3361,3456,5675-5707,5834`.

The production gates now retain their reason in the existing per-turn blocked
action receipt. A later Haze or Fire defrost no longer replaces the earlier
caption with a guess from final status. The translator also distinguishes a
blocked action's Sleep(n) countdown and wake delta from genuine new sleep:
countdown does not print fell asleep again, and the wake caption appears once.
A fresh Hypnosis before the gate or after waking keeps its own sleep caption.

Fifty-four prepared public-FIGHT turns cover both sides, slower Haze, all four
Fire defrost moves, earlier Fire thaw allowing an actual action, ordinary sleep
countdown/wake, next-turn receipt cleanup, and fresh Hypnosis before/after a gate.
Tests assert exact captions, status outcomes and random draws. The parent has
20 wrong cases in the 24-case matrix; four of five tests fail. All five pass
afterward, including the unchanged earlier-thaw controls.

The full core suite passes 3,707 tests with one existing ignored test. Fresh
native NEW GAME through m06 and all 11 subsystem scenarios pass with source and
binary held fixed. All seven original GBA performance scenarios and 13 memory
scenarios pass with unchanged workloads, gates and timeouts; minimum contiguous
heap is 7,832 bytes and stack is 8,708 bytes. Completed memory logs were losslessly
compressed and verified. These are scoped regressions, not a full first-clear run.

Both screenshots use identical prepared FIGHT input and frame 3. The faster
frozen enemy cannot act; slower Ember subsequently thaws it while it survives.
Before guesses cannot move from the cured final state; after prints frozen solid
at the actual blocked action. Pass an output PNG path and frame `3` to capture.rs.
The exact harness uses BattleVisualEffects and draw_battle, not the outer
PokemonGame/audio clock or original-ROM timing. Hashes and validation identities
are in verification.json. All three remote workflows and six actual GBA runtime
steps must pass before merge.
