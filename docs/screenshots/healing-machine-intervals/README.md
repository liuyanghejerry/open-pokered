# Healing machine intervals

The original AnimateHealingMachine delays 30 hardware frames after each
party ball. FlashSprite8Times toggles the palette eight times, holding each
for ten frames. Two continuous original recordings from an actual
SAVE/CONTINUE and walk to Viridian confirm these intervals.

The native runtime previously waited an extra zero-counter frame: ball
spacing was 31 and palette spacing 11 frames. It also performed an initial
toggle plus eight more, producing nine intervals. Resume on the Nth update
and count the initial palette toggle as the first of eight.

Before/after screenshots use the same five-member nurse fixture and frame
675. The second ball appears at this frame after the fix and one frame later
before the fix. The base includes the preceding healing-text fix (#203).
Both native recordings match byte-for-byte; numerical source and native
stages are in verification.json. Tests cover ball spacing for party sizes
one through six and all eight ten-frame palette intervals.

These checks prove the measured intervals, not full-scene timing parity.
Audio fade, music completion, and command-dispatch timing still need audit.
