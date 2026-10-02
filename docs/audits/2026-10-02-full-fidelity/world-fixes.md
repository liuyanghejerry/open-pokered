# World fidelity fixes and verification

The audit baseline is `72ff719`; the original is `pret/pokered@fbcf7d0`. The complete independent table comparison and baseline findings are in `data-world.md` and `data-world-check.json` (the coordinator's audit commit). These fixes cover D-W01–03 and D-W05–09; the original NPC RNG strategy D-W04 is coordinated separately.

- Encounter probability samples the bottom-right 8px tile in the current 16px player cell, including the map's eastern half. Grass/water list selection still follows the original bottom-left tile.
- Map entry updates rates and preserves the last nonzero grass/water lists, reproducing legal-species left-shore encounters. Blue propagates from existing GameConfig through overworld and snapshots. Native launch accepts `--game-version blue`; Red remains the default.
- Route1's first NPC uses the original vertical axis. The second NPC stays horizontal.
- All 104 ground balls use the original success/failure rule and persistent toggle bits. Seven rewards use the correct TM IDs; five PowerPlant balls and ViridianGym Revive gain their missing handlers.
- Successful found text prints, plays GET_ITEM_1, waits for the sequencer, and closes without a new A press. Holding A retains the original HoldTextDisplayOpen behavior. Failed pickups keep ordinary manual dialogue and play no found-item jingle. Native and TUI render no prompt arrow during automatic found text.

## Checks actually run

| Check | Result |
| --- | --- |
| Independent ground scene scan | All 104 connected; zero remaining reported defects, `ground-pickup-after.json` |
| `cargo test -p pokered-core --lib overworld::tests_wild_encounters` | 62 passed, including four added original-coordinate/cache/Blue-snapshot/Route1 regressions |
| `cargo test -p pokered-core --lib ground_pickup_fidelity_tests` | 3 passed: all 104 compiled handlers, 5 real A-button entries, and text/sound/input/snapshot phase behavior |
| Native and TUI builds | `cargo build -p pokered-app -p pokered-tui --features debug-server` passed |
| Native before/after driver | Actual Inventory and controller interaction confirm all eight baseline/fixed outcomes below |
| `git diff --check` | Passed |

The 104-handler test fills 20 actual item names excluding the target, verifies failed pickup keeps the object/flags/SRAM bit, frees one slot, verifies exactly one correct item request and jingle, then restores into a fresh map and checks the ball stays hidden. The native driver independently fills the real frontend Inventory and records bag/NPC/flag state; its end assertions reproduce baseline loss, duplication, wrong TM and missing pickup.

| Native scenario | Baseline | Fixed |
| --- | --- | --- |
| Route2 Moon Stone, full bag | No reward, ball hidden and collected flag set | No reward, ball and flags preserved |
| MtMoonB2F HP Up, two interactions | Quantity 2 | Quantity 1; collected object hidden |
| MtMoon1F Water Gun ball | TM34 | TM12 |
| PowerPlant Carbos ball | Empty bag after interaction | Carbos ×1 |

## Screenshots and reproducible drivers

Both binaries start with seed 123 and driven-only headless input. All PNGs are 160×144 direct game renders in `docs/screenshots/fidelity-world/`. Logs are `ground-native-trace.json` and `route1-npc-trace.json`.

```sh
python3 docs/audits/2026-10-02-full-fidelity/check-ground-pickups.py
python3 docs/audits/2026-10-02-full-fidelity/capture-ground-regressions.py \
  --before /workspace/onboarding/pokered-audit-base-app \
  --after /workspace/onboarding/pokered-world-fixed-app
python3 docs/audits/2026-10-02-full-fidelity/capture-route1-axis.py \
  --before /workspace/onboarding/pokered-audit-base-app \
  --after /workspace/onboarding/pokered-world-fixed-app
```

| Scenario | Before | After |
| --- | --- | --- |
| Route1 NPC, same frame 240 | [Before](../../screenshots/fidelity-world/route1-npc-before-240.png) | [After](../../screenshots/fidelity-world/route1-npc-after-240.png) |
| Full-bag pickup, dialogue closed | [Before](../../screenshots/fidelity-world/ground-full-bag-before-closed.png) | [After](../../screenshots/fidelity-world/ground-full-bag-after-closed.png) |
| Repeated pickup, first dialogue closed | [Before](../../screenshots/fidelity-world/ground-repeat-pickup-before-closed.png) | [After](../../screenshots/fidelity-world/ground-repeat-pickup-after-closed.png) |
| Correct TM, found text | [Before](../../screenshots/fidelity-world/ground-correct-tm-before-dialogue.png) | [After](../../screenshots/fidelity-world/ground-correct-tm-after-dialogue.png) |
| Missing PowerPlant handler | [Before](../../screenshots/fidelity-world/ground-missing-handler-before-dialogue.png) | [After](../../screenshots/fidelity-world/ground-missing-handler-after-dialogue.png) |

The NPC screenshots isolate the axis data change using the baseline generic movement implementation: frame 240 is `(6,24)` before and `(5,24)` after. The corrected NPC attempts Up/Down and is blocked by local terrain in these sampled frames; these captures do not claim a matching original RNG path. The separate classic movement strategy is tested by the coordinator. Headless captures disable audio output; sound wait uses a controlled sequencer status in the phase regression, while normal frontends sample the actual backend.

The preserved encounter buffers cover the original quirk with valid species. The old-man player-name RAM overlay, MissingNo and invalid internal species remain outside this implementation. Snapshot tests preserve the logical sound phase; live audio sample positions remain in the frontend backend rather than the existing runtime snapshot schema.
