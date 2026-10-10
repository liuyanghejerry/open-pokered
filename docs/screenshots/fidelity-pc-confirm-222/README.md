# PC confirmation controls, isolated audit 222

Original Release, Change Box, PC Toss and Oak rating all use YesNoChoice.
InitYesNo clears the menu ID; DisplayTwoOptionMenu starts cursor 0 (YES),
HandleMenuInput clamps with wrapping disabled and gives UP priority.
B cancels even when A is pressed simultaneously. Actual master 0c58485
instead starts NO and toggles on either direction. This independent fix
changes only these defaults and directional controls in PcScreen, with
actual PokemonGame regression tests. Existing NO tests now explicitly
select DOWN and retain their no-release/no-save assertions.

Original controlled pre-PC fixture is map41 at13,4 facingUP, Medium,
Exeggutor13 in box1 and Potion4 in PC items, Pokedex flag seeded. Genuine
PC input navigation reaches all four choices. Seven cases each, twice:
idle, UP, DOWN twice, DOWN then UP, simultaneous UP+DOWN, A+B, and A.
All 28 cases start YES; repeated direction clamps; UP wins; A+B cancels.
A alone releases one Pokemon, tosses one Potion, opens the box list or
prints the Dex rating. Entire raw JSON/hooks and all44072 original PNG
repeat byte-exact within each case; seeded data is not natural acquisition.

Actual Game also uses a controlled collision-checked Viridian Center13,4
fixture with Exeggutor13, PC Potion4, Pokedex flag, seed42/English/Medium.
Real A opens the map's hidden PC event; actual menus are navigated through
inputs, not by forcing PcPhase. Four contexts x seven cases x161 frames
x two repeats x actual master/candidate =18032 PNG. Measurements begin
on first native choice frame, with UP/DOWN at40/64 and A/A+B at90.
Every recorded frame checks retained render against full redraw, all pixels.
Within each side every raw JSON and PNG repeats byte-exact. Unmodified
master plus identical cfg(test) helper fails the source-driven owner test
at initial NO; candidate passes all28 cases and complete core/app tests.
Before/after images use the same fixture/input/frame30 for four contexts.

62104 PNG paths are stored losslessly as SHA256 blobs with png-files.json;
every image and every other archived file is read back and byte-compared.
Primary assembly, full raw records, logs, helpers and source/binary hash
manifests are included. No ROM, emulator state, SRAM or executable is
published. These are controlled real-runtime inputs, not natural traversal
or absolute original CPU/PPU timing and pixel alignment. PC text paragraphs,
typing, global repeat timing, choice post-confirm wait15, audio order,
layout and broader PC lifecycle are outside this narrow fix and remain
separate audits. Native transactions still commit immediately after A;
this PR does not claim original post-confirm delay alignment.
