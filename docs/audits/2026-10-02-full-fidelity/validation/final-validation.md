# Final validation and evidence sources

All commands run from the game repository, after `source /workspace/onboarding/env.sh`, with `CARGO_INCREMENTAL=0`. Offline/locked builds used the public engine code revision `92ba95f9526107e17874df53291ac7758d0930f0` in all 17 manifests and both lockfiles.

The immutable native artifacts were built from `8837a2a32c2ba8c345db16e97b01f49016e88950`. The final native test run includes `9ad9034`, a test-only correction storing Toxic's existing Poison state as `Some`; it changes no production behavior. `final-build-manifest.json` records the binaries and their hashes. Earlier screenshot snapshots retain their own exact provenance instead of adopting the final source label.

| Check | Command / result | Evidence |
| --- | --- | --- |
| Full native packages | `cargo test --offline --locked --no-fail-fast -p pokered-core -p pokered-data -p pokered-app -p pokered-ui -p pokered-renderer -p pokered-audio -p pokered-tui -p pokered-debug-server --features debug-server`; exit 0, 105 targets, 4444 passed executions, 9 ignored; lib/bin repetitions included | `final-native-tests.log` |
| Final native artifacts | `cargo build --offline --locked -p pokered-app -p pokered-tui --features debug-server --bin pokered-app --bin pokered-tui --example fidelity_visual_capture --example pc_tour`; exit 0 | `final-native-build.log`, `final-build-manifest.json` |
| Web | `cargo check --offline --locked -p pokered-web -p pokered-runner-web --target wasm32-unknown-unknown`; exit 0 | `final-web-check.log` |
| GBA | From `crates/pokered-gba`: `cargo +nightly-2025-12-07 build --release --offline --locked`; exit 0; compilation only | `final-gba-build.log` |
| Engine | In engine PR branch, `cargo test --offline --locked -p dotzuki-engine`; production code 92ba95f, test correction 9d09696: 488 unit and 3 doctests passed, 7 doctests ignored | `final-engine-tests.log` |
| Seeded subsystem scenarios | `AUDIT_BINARY=/workspace/onboarding/pokered-fidelity-final/pokered-app python3 -u docs/audits/2026-10-02-full-fidelity/validation/run_subsystems.py`; exit 0, 11/11 | `final-scenarios.log` |
| Captain's actual music wait | Actual no-audio app and loaded native scene waitMusic→setFlag with real healed byte streams; pass; included again in full suite | `final-captain-tests.log` |
| Continuous fresh first clear | `AUDIT_BINARY=/workspace/onboarding/pokered-fidelity-final/pokered-app python3 -u docs/audits/2026-10-02-full-fidelity/validation/run_first_clear.py --until m49 --artifacts /tmp/pokered-fidelity-final-first-clear`; running, independent empty save, seed42, speed0 | Result added after completion |

The 11 seeded scenarios use development state setup and do not constitute a continuous clear. The continuous driver uses actual buttons/navigation/battles and synchronous frame stepping, with no development warp, granted items, flag writes, snapshots, or resume. The final milestone itself checks the game's post-credits save by CONTINUE in a separate process.

The engine PR final CI at 39d8d2e passed Rust workspace, coverage, editor, package gate and English/Chinese documentation checks. Android export failed during Setup Android SDK, before application compilation. Local game GBA build does not establish hardware audio, OBJ framebuffer, or physical serial-link correctness. SRAM layout evidence comes from independently assembled original RAM symbols and field conversion tests; no new original ROM import test is claimed.

Logs without a `final-` prefix are retained baseline/intermediate diagnostics. Their failures are useful reproduction evidence and their earlier successes are not substituted for final source validation.
