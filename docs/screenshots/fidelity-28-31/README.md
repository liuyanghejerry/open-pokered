# Fidelity fixes 28–31

Base: master `65d98240470e8013a9d130e8bda67cd0f7c17d8c`.

Before captures were taken with master checked out; after captures use the
repair branch. Both use identical seeded, walkable positions and button input.
Absolute capture frames match: Snorlax 835, Warden 1300, Giovanni 701.

- Snorlax: carrying the flute and pressing A leaves it asleep. Only BAG → USE
  wakes it; its battle starts after the played-flute text without another talk.
- Giovanni: after interception text, DOWN ×3, reciprocal facing, Delay3, battle.
- Warden: teeth handover fanfare, “The WARDEN popped in his teeth!”, thanks,
  HM04 receipt fanfare. A full bag retains only the handover fanfare.
- Other receipts: TM35, roof TM13/TM48/TM49 use GET_ITEM_1; Master Ball uses
  GET_KEY_ITEM. Repeat/full-bag reward paths do not play a receipt fanfare.

Replay with an app test binary built with `--features debug-server`:

```bash
python3 docs/screenshots/fidelity-28-31/capture.py --binary APP_TEST_BINARY --label before
python3 docs/screenshots/fidelity-28-31/capture.py --binary APP_TEST_BINARY --label after
python3 docs/screenshots/fidelity-28-31/verify_runtime.py --binary APP_TEST_BINARY --output runtime-results.json
```

`verification.json` summarizes full core, app, and data/Boa tests.
`runtime-results.json` and `runtime.log` retain button-driven seeded checks;
`playthrough.log` is the successful fresh m01–m49 run through saved CONTINUE.
The first fresh run stopped at a Victory Road navigation return to the adjacent
floor. The second fresh run passed without changing navigation; both used the
same game binary. No restored milestone save was used.

The headless transport verifies receipt effects and sound IDs, not audible
speaker output. Audio-only receipts require no additional comparison frame.

## Final verification scope

The retained successful full playthrough used repair commit `a11fb0e`.
The subsequent `80aad7f` changes only the per-frame Snorlax flag lookup from a
copied map to direct event bits. Its complete core/app tests, target runtime
checks and identical screenshot replay passed again. Two additional full
playthrough attempts stopped in the navigation driver at MtMoonB2F → Route4
and a PokemonTower6F path lookup, before reaching the changed Snorlax/Silph
scenes. These later full attempts did not complete; do not interpret the
retained successful log as a fresh full run of `80aad7f`.
A control run of unchanged master completed m01–m25. Navigation failures in
later reruns remain a verification limitation, without a reproduced causal
connection to these four fixes.
