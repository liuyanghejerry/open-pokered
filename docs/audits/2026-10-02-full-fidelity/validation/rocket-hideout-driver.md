# Rocket Hideout driver handoff recovery

Continuous fresh attempt 4 passed **m01–m21**, then m22 asserted a missing
`EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2`. The retained
[failure state](rocket-hideout-fresh4-failure-state.json) has the player at
RocketHideoutB4F (11,3), facing left, with no dialogue/script; the
[NPC observation](rocket-hideout-fresh4-failure-npcs.json) has Rocket text ID 4
at (11,2), facing down. Its displayed battle result belongs to an earlier B1F
trainer, not this B4F trainer. This attempt did not complete m22 or the game.

This is a driver observation race, not a missing trainer battle:

- Original `scripts/RocketHideoutB4F.asm:95–96` gives this trainer sight range
  1. Standing (11,3) triggers the encounter, which locks directional input.
  Thus the player's left-facing snapshot is consistent with an encounter
  already underway; the subsequent upward turn was ignored.
- Original `home/trainers.asm:106–123` prints battle text before engaging;
  `RocketHideoutB4F.asm:189–199` separately reveals the key when the beaten
  trainer is spoken to again.
- Current `overworld/update.rs:1222–1250` promotes its parked trainer intro
  after the final text/script closes. Debug `game.rs:6540–6552` can report
  `control_ready` at the preceding boundary because its predicate does not
  include that pending intro. The old helper checked the screen immediately
  and returned before the normal promotion update ran.

The supplied final native binary reproduces that exact boundary through
CONTINUE and ordinary walking/input from an independent controlled fixture:
the old helper returns at **frame 1018**, still overworld at (11,3), with the
victory flag absent. Advancing **one neutral frame** enters the actual battle
at frame 1019. See [before log](rocket-hideout-driver-before.log) and its
[input/observation record](rocket-hideout-driver-before.json).

`finish_talk` now advances one ordinary update after dialogue completion and
observes the resulting screen before declaring the interaction finished.
It completes a resulting battle and its post-battle script. `talk_npc` checks
an explicitly requested completion event after this flow and retries an
uncompleted interaction; it also preserves the existing on-step victory and
moving-trainer handling. m22 requests the real victory event for the first
conversation and the real key-drop event for the second. Fixed-coordinate
trainer interactions use the same handoff handling.

Validation: **63/63** interaction/navigation/recovery/exploration unit tests
pass, including deferred promotion, fixed-coordinate trainers, an NPC hidden
by on-step victory, a moved trainer, and an unresponsive required interaction.
The [controlled route](rocket-hideout-driver-route.py) uses the same seed 42,
speed 0 and `smart_moves` strategy as fresh m22. Ordinary inputs defeat the
actual Rocket18 team (Koffing/Zubat, level 21), set the victory event, complete
the subsequent admission/key-drop event, and collect **one Lift Key**; the
ground object becomes hidden. Frame **589 → 2598**, 334 commands, all read-only
observations or ordinary input/frame advancement. See [after log](rocket-hideout-driver-after.log),
[input/observation record](rocket-hideout-driver-after.json), and
[test log](rocket-hideout-driver-tests.log).

This fixture is a **seeded driver regression**, not a continuous fresh clear.
It starts at B4F (19,10) with one level-37 Venusaur, four badges and default
event flags; no flags, items, warp or collision bypass is injected during the
route. No product source changed, no Cargo run was needed, and this change
does not claim any milestones after fresh attempt 4's m21.

Reproduce from the repository root:

```sh
python3 docs/audits/2026-10-02-full-fidelity/validation/rocket-hideout-driver-route.py \
  --fixture docs/audits/2026-10-02-full-fidelity/validation/rocket-hideout-driver-fixture.json \
  --binary /path/to/pokered-app --baseline
python3 docs/audits/2026-10-02-full-fidelity/validation/rocket-hideout-driver-route.py \
  --fixture docs/audits/2026-10-02-full-fidelity/validation/rocket-hideout-driver-fixture.json \
  --binary /path/to/pokered-app
PYTHONPATH=scripts python3 -m unittest scripts/test_playthrough_interactions.py \
  scripts/test_playthrough_navigation.py scripts/test_playthrough_recovery.py \
  scripts/test_exploration_playthrough.py
```
