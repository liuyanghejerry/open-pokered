# Misty blackout recovery

The fresh candidate run completed m01–m12, then lost to Misty: Ivysaur had 0 HP and Starmie 3 HP. The engine returned to Cerulean City, halved money and healed the party without awarding the victory flag. The old m13 driver stopped at its one-attempt victory assertion.

m13 now uses the existing strict town recovery helper, with at most three attempts. Each attempt uses normal healing and walking. An unrelated assertion, a missing flag after a win, a non-town location or a party that did not faint still fails. Rewards and the real Misty flag remain required.

The 39 existing recovery tests passed. The preserved post-blackout SaveData fixture was booted through normal CONTINUE, healed and walked back into the gym on the unchanged candidate executable. The real challenge awarded the Cascade Badge and victory flag. This fixture replay resets runtime/RNG and is diagnostic evidence, not a resumed or fresh whole-route verdict.

To replay, boot `post-blackout-fixture.json` with the CLI `--snapshot` option, use `playthrough.Game` with seed42/speed0, call `resume_reentry`, set the controller's `smart_moves=True`, then invoke `playthrough_late.m13_misty`. No runtime game-state setters are used.

No production on-screen output changes; this PR changes tooling only.
