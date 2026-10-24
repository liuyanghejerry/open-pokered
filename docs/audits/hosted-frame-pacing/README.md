# Hosted frame pacing

The web shell polled continuously even between simulation deadlines. A local
release build scheduled 305,552 tasks in 10.07 seconds, spending 9.62 seconds
inside main-thread tasks. The native window shell slept in `AboutToWait`, which
also delayed redraw processing. Its X11 path ran at approximately 42 FPS.

[dotzuki #110](https://github.com/liuyanghejerry/dotzuki/pull/110) replaces both
waits with winit deadline scheduling. The simulation still uses the existing
59.7275 Hz clock and late-frame policy. This game change consumes that engine
revision consistently across the workspace and standalone GBA build.

## Measurements

Both builds used game base `23c5c5c4` and the same prepared save. The candidate
used the exact engine source committed as `ce5dc7b0` through a temporary local
Cargo patch; the shipping dependency uses the public Git revision.

| Scenario | Before | After |
| --- | ---: | ---: |
| Web scheduled tasks, approximately 10 seconds | 305,552 | 1,160 |
| Web main-thread task duration in that interval | 9.62 s | 2.09 s |
| Actual WebGL presentations | 58.26 FPS | 59.14 FPS |
| X11 field idle | 42.08 FPS | 55.55 FPS |
| X11 held DOWN, then stationary at a fence | 42.15 FPS | 57.50 FPS |
| X11 wild battle introduction / waiting for input | 42.10 FPS | 60.02 FPS |

These are local Linux measurements, not guarantees for every device. Browser
measurements used headless Chromium with software WebGL. CDP `TaskDuration`
measures main-thread occupied time, not total-system CPU usage. The debug
response timing adds small measurement error to the native frame counts. A
fallback native window path already ran near 59 FPS before the fix.

Browser functional checks also covered CONTINUE, held movement, one-millisecond
START and B taps, complete menu rendering, and audible PCM (44.1 kHz, nonzero
waveform). A 4× CPU throttle retained normal game speed while reducing occupied
main-thread time from 9.91 to 5.40 seconds in approximately 10 seconds.

The two committed screenshots use the same save, seed 42, and frame 10. Both
have SHA-256 `17bd5d49ceb556a126881986d52423530b0cc51be96033ae73faef09148b82c2`.
They verify identical rendering; the timing measurements verify pacing.

## Reproduce

Build each revision with `cargo build --release -p pokered-app --features
debug-server`, then run `measure-desktop.py --binary /path/to/pokered-app
--save pallet-save.json --output before.json` from this directory. It opens a
real window with device audio and uses ordinary paced updates; debug commands
only read state, queue held buttons, and prepare the battle. Select the same
display backend for both runs. Do not run concurrent builds during sampling.

Build the browser with the release commands from `.github/workflows/deploy-web.yml`
and serve its Vite `dist`. Run `measure-browser.cjs URL pallet-save.json output-dir`
with Playwright installed (or `PLAYWRIGHT_MODULE` pointing to that installation).
Inspect its PNGs to confirm the expected scenes; fixed boot key pulses are a
prepared-fixture controller, not an ordinary NEW GAME walkthrough.

For CPU stage costs without window pacing, run:

```sh
cargo run --release -p pokered-app --example profile_frames -- 3600
```

The save is exported from a completed mainline run on the earlier #377/#378
game source. Its use here is prepared-state regression evidence, not a claim
that this new engine pin completed another ordinary NEW GAME playthrough.
