# Gift skip-dialogue regression fixture

The failing debug-server test never started the gift interaction. It sent one
neutral hardware frame and then a one-frame A pulse. The ordinary field loop
samples JoypadOverworld once per two VBlanks; after that neutral frame, A landed
in the wait frame. `control_ready` was already true, the warp fade was Idle,
both NPCs were visible, and every subsequent SkipDialogue returned `stepped=0`.
This was a stale input fixture, not a gift, initial-fade, or skip-dialogue bug.

The fixture now asserts `control_ready` before interacting and holds A for two
hardware frames, followed by release frames. Tests cover both initial sampling
phases (one or two neutral frames), without forcing fade, script, dialogue, or
choice state. The original YES/NO, inactive naming screen, and empty-party
assertions remain. Additional checks require the default YES cursor and an
unset gift flag; calling SkipDialogue again must advance zero frames and leave
the question untouched.

The ordinary-input control reaches the same dialogue, choice, party, script
state and effect using only real A taps. It leaves the question unanswered
through neutral frames, then explicitly selects NO and presses A. Only then
does the normal runtime grant one level-25 Eevee, set the gift flag and return
player control. This is in-process PokemonGame input evidence, not an ordinary
SRAM SAVE, source-emulator comparison or screenshot-parity claim.

Only the `#[cfg(all(test, feature = "debug-server"))]` test module changes. The
surrounding production file content is byte-identical to base
`db612ae63b22106632bc1b2f62b835614442d4f7`; no player mechanics or rendering
changes. Before/after screenshots are unnecessary under the test/tooling
exception in AGENTS.md.

Evidence is retained externally under `fidelity-39-plus/audit-887-*`: the exact
failing log/executable and base source, diagnostic source/logs, the two-frame
control, final source freeze, scoped/full debug-library results and final hash
manifest. The fresh target is `fidelity-887-debug-gift-native-target`; targets
882 and 307 were not used. Validation counts are recorded in the final report.

The first full parallel run of the final gift source had a separate boulder
fixture failure before its push started. That failed log is preserved. The
old executable passed the boulder test both alone and in its full parallel
run; the old full run failed only the original gift test. The new serial full
run and a temporary boulder-telemetry parallel full run passed. The boulder
failure did not reproduce with telemetry, so its cause remains unknown;
parallel interference is not established. No boulder input or assertion was
changed, and temporary instrumentation was removed before final validation.

Final source validation: exact SkipDialogue test 1/1, debug gift/control module
4/4, and the same final debug-library executable with default parallelism
251/251 enabled tests passed (38 pre-existing ignored tests). The final source
SHA matches its pre-build freeze; final full parallel validation used no
boulder instrumentation. Earlier failure and control logs retain their own
identities rather than being relabeled as final-binary results.
