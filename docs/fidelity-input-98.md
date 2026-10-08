# Field input sampling (98): partial repair, timing audit remains open

Reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c, Red ROM SHA-1 ea9bcae617fdf159b045185467ae58b2e4a48b9a.
`home/overworld.asm` checks the walk counter before `JoypadOverworld`, checks START before A, and completes `AdvancePlayerSprite` before returning to the loop.

The runtime now retains the last field input sample during a step. New physical A/B edges continue updating for dialogue handling, while field A/START edges are sampled only after landing. Held buttons carried through a menu are synchronized to the previous physical frame, so closing UI neither invents a press nor discards a genuinely new press. Landing no longer consumes the next direction in the same call. Scripted follow paths remain a separate path; their destination comparison projects an NPC already in motion.

## Reproduction and limits

Base screenshots use an actual checkout of master 31b1eda with an identical ignored capture helper, not reverted production code on the repair branch. Both start from the same original SRAM through actual Title / Continue. The PC probe then uses a controlled warp to Viridian Pokecenter (13,4), a real Down step, and stages facing Up at (13,5), because native turning remains an open audit. The original uses the normal map-load path with a controlled destination/view pointer and actual Down/Up input. These are controlled probes, not travel or reachability proofs. Source has six party members; native SRAM has five, so menu pixel identity is not asserted.

Hold Up from t0 through t15. At t5 press A for one frame, A for forty frames, or A+START for forty frames. Each original/before/after case was captured twice for 101 frames. Native PNGs and JSON match byte-for-byte between repeats. Original captures and routine hooks are retained in the archive.

| Input | Original | master | Current repair |
|---|---|---|---|
| Short A during step | ignored | ignored | ignored |
| Held A during step | opens PC at t18 | no PC | opens PC at t10 |
| Held A+START during step | START text at t18, menu draw t38; no PC | menu at t5 midstep | menu at t9 after landing; no PC |

**Partial:** field input sampling and priority behave as intended in these probes. **FAIL/PENDING:** ordinary walking cadence, menu entry timing, turn delay, and sprite poses are not yet aligned. Do not infer full RGB or full escort fidelity from these results.

Evidence: `screenshots/fidelity-input-98/manifest.json` and `raw-captures.zip` contain repeat records, source hooks, inputs and driver hashes. Images labeled master-before/native-after use the same fixture and frame; held-A landing images use t11, other preview images t6.

Validation on the production changes before this capture-helper extension: pokered-core 2711 passed; debug-server pokered-app 194 passed, 16 ignored. Existing held-A dialogue and initial Oak-follow regressions pass. Two earlier failures and their logs remain in the task artifacts. A fresh complete story run remains required after the ordinary clock fix. The preceding dust 4d5fa79 diagnostic reached m49; it is not evidence for this branch.
