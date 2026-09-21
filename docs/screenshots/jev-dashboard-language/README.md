# Jev dashboard language comparison

Before screenshots use `master` at `384a92f6b3e4b8ca91cb1b56ad0e7c01312477d0`.
After screenshots use the English dashboard on this branch. Both show the same
recorded Jev frame at `12931.366666666667` seconds (3:35:31), with Charizard at
4/210 HP and the restore-party choice at 46%. The paired player is in chapter 8;
the scripted run has already finished the chapter and waits on its recorded still.

- `single-before.png` / `single-after.png`: standalone dashboard, 1800 px viewport.
- `comparison-before.png` / `comparison-after.png`: paired replay, 1800 px viewport.
- `single-mobile.png` / `comparison-mobile.png`: English layouts, 390 × 844 viewport.

To reproduce the scene, open `docs/jev-retrospective-assets/full-run/jev-player.html?lang=en#time=12931.366666666667`,
or open `player.html?lang=en#chapter=8` and select the “Retreat after three wins” commentary moment.
Use a browser directly or serve the repository with an HTTP server supporting Range requests.
Captures are full-page screenshots taken while paused.

Validation: Chrome/Playwright checked both language-switch round trips, precise
playback positions and speed, the frozen earlier finisher, mobile overflow,
key dialogue/battle/move-learning inputs, B and skip_dialogue commands, and the
independent CONTINUE ending. All 2,913 distinct strings in the recorded display
data were checked for untranslated Chinese. Expanded request/response JSON was
compared with the original input record, and rendering did not mutate the data.
The staged Pages redirect preserves the language query and playhead hash.
