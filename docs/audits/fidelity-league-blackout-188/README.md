# League blackout regression driver

The real NEW GAME seed42 run reached Agatha, lost with all three battle-party members at zero HP, and returned to IndigoPlateau (9,6), frame926716. `talk_npc` attempted to find Agatha again in an empty outside-map NPC list and raised `StopIteration`. This bypassed the existing `AssertionError`/`RuntimeError` recovery handler.

Missing NPCs now produce a descriptive `RuntimeError`. The existing recovery handler still requires overworld IndigoPlateau, a losing battle phase and a nonempty wholly fainted battle party. It retains the five-retry cap, earned route entry, real shopping and required victory flags. Other missing NPCs stay failures.

Two added tests fail with `StopIteration` on head21c3583 and pass on the corrected driver. All85 navigation, interaction and recovery tests pass. The recorded failure state separately satisfies the unchanged confirmed-blackout predicate.

This changes only the regression driver. The previous full run is retained as a failure, without a completion claim. A new immutable full NEW GAME through m49 is running with driver188, the same unchanged production binary and2451 frozen data/driver/asset files. m49 requires Hall of Fame, credits, the game's own save, and Continue in a separate process. Its terminal result remains a merge gate.
