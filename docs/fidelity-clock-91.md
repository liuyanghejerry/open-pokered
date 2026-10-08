# Ordinary field loop cadence: repair in progress

Reference pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c, `home/overworld.asm`: OverworldLoop delays two frames, checks the walk counter before reading JoypadOverworld, initializes eight and immediately advances to seven. Initial LoadCurrentMapView crosses a hardware-frame boundary. Bicycle speedup performs the first and second advances on opposite sides of that redraw; subsequent loops subtract two. NoDirection arms the turn check, which compares LastStop rather than visible sprite facing and consumes a loop before walking. A turn calls NewBattle but does not run the collision/boulder attempt.

The ordinary runtime now uses this loop clock. Physical dialogue edges and background tile animation still tick each hardware frame. Blocking boulder/field animations return above the ordinary clock. Scripted paths, ledges and connection transitions still use separate clocks and require further audit; they are not verified by the ordinary probes.

## Matched actual-SRAM replay

Both native variants start from the same source SRAM through Title / Continue, then actual bag Bicycle use where applicable and 120 idle frames. Source pretrigger states come from the same saved road setup and actual Bicycle use. Hold Down from t0 through t31; press START at t5 for one or forty frames. Each variant was captured twice for 101 frames. Current native coordinate and walk-counter traces equal the original at every frame in all four cases. Current native repeats match PNG and JSON byte-for-byte. Native START processing occurs at t19 on foot / t12 biking, matching original DisplayTextID. Short START is ignored in both modes.

**Partial only:** original draws the START menu20 frames after DisplayTextID; native currently draws immediately. Sprite poses, visible facing and background scroll require a presentation delay/counter repair. NPC, scripted walk, edge transition and boulder initiation phase remain open. Complete story regression still required after the final change.

An earlier driver held Bicycle Down16frames while the source START probe held32. That comparison is rejected for whole-sequence fidelity; its files and mismatch report remain in task artifacts. The archive here contains the corrected32-frame inputs, source routine hooks, actualmaster31b1eda before captures with readonly helper, current after captures, repeat data and driver source. Manifest scopes the claims and pins driver hashes.

## Regression adjustments

Core tests previously assumed8 hardware frames walking/4 biking and one-frame A pulses. They now assert the primary-reference counter traces (15/7 frames after initiation) and hold/release field A for two hardware frames so an OverworldLoop sample can observe each edge. Dialogue physical-edge behavior remains checked; no story-state or item-reward assertions were removed. Initial core run had18 failures, second had1 full-bag retry failure; logs are retained. Latest core run:2716 passed. Latest debug-server app suite:195 passed,17 ignored. Initial app run had5 failures, second had1 link-computer test failure: the test needed a sampled Left turn and a continuous A press rather than two synthetic fresh A edges. All interaction and link-delay assertions remain. No merged or final whole-game verdict yet.

## Combined boulder regression (open)

Replaying the actual Seafoam SRAM and actual Strength input with this ordinary clock produces53 differing RGB frames out of201 against the original Down push. Pretrigger RGB matches; the first mismatch is image frame10. Current logical boulder destination commits at t8 and native smoke is active t48..72, so initiation timing needs to be revised together with the previously isolated boulder routine. Do not cite the older PR143 ordinary-push pixel PASS as proof for this integrated clock. Failed raw capture and pixel-count comparison remain under `.task-tmp/fidelity-39-plus/clock-91-first-dust-down-1` and `clock-91-first-dust-comparison.json`. Cross-encoder PNG byte differences were rejected; this finding uses decoded RGB pixels without shifting sequences.
