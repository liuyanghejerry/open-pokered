# Escape moves: findings 36–38

Base: master `7585724` (PR #124 merged after all CI passed).
Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/battle/effects.asm:810–911` and `data/text/text_3.asm:126–140`.

36. TELEPORT, ROAR and WHIRLWIND were treated as connecting unconditionally on the production stack path. Original trainer battles always reject them; in a wild battle, a user at least as strong as the target always succeeds. Otherwise, BattleRandom rejects bytes >= user level + target level + 1, then fails below target level / 4. The shared stack accuracy hook now performs that exact sampling without an accuracy/evasion roll.
37. Successful escape moves previously completed the entire turn: a slower enemy could attack or lower stats after TELEPORT, and residual damage could still run. A game-side escaped marker now suppresses further actions and poison/burn/Toxic/Leech Seed ticks. The generic engine stays unchanged; battle-type context travels through provider-owned resources.
38. Trainer TELEPORT omitted “But it failed!”; successful flee moves used the ordinary RUN message. Narration now uses the original move-specific user/target messages and failure text. Normal menu RUN keeps its separate behavior.

## Validation

- Full core suite: 3,110 passed, one doc test ignored. After extending test-only coverage, all five escape tests passed, including exact rejected-byte/threshold/draw checks, both users and all three moves, cancelled opposing action, unchanged Toxic counter/HP, trainer failure narration, and Mirror Move resolving to TELEPORT. The original guaranteed-success test now uses equal levels; it no longer assumes a weaker user always succeeds.
- Shared app with debug-server: 160 passed, three ignored harness/capture tests.
- Production input harness: 64 independent level-5 ABRA vs level-50 METAPOD battles escaped **64/64 before**, **50/64 after**. The exact probability is checked by scripted-byte tests; the runtime sweep proves both outcomes are reachable.
- Eight level-50 ABRA vs level-5 CATERPIE cases: before, CATERPIE still used TACKLE/STRING SHOT, and five cases lost one HP. After, no opposing move executed and all eight retained 95 HP.
- Against Brock, TELEPORT remains unable to escape, consumes its turn, and now prints the missing failure text before GEODUDE's attack.
- Fresh m01–m49 passed, including one ordinary League blackout retry, ending autosave and independent-process CONTINUE. No seeded milestone resume or driver weakening.
- Three paired screenshots at absolute frame 4000: successful TELEPORT, trainer failure, and low-level failure. Base captured with master checked out; after captured in the escape branch checkout. Same fixture/seed, inputs, player position and battle screen. Images are in `docs/screenshots/fidelity-36-38/`.

Reproduce with `python3 scripts/fidelity_escape_moves.py DRIVER ARTIFACT_DIR`;
add `--before` to capture the base without asserting the repaired behavior.
The driver is the app's ignored `game::fidelity_stdio::driver` test executable.

The long-running fidelity goal remains active; this is not an exhaustive no-gap claim. Fonts, Chinese, pinyin input and dialogue-box layout remain excluded. Previous sporadic navigation failures still require separate investigation.
