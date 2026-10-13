# Restore center music immediately after healing

DisplayPokemonCenterDialogue_ restores the audio bank and calls PlaySound
for the map song immediately after AnimateHealingMachine returns. Actual
original captures show the machine final hold ending and the map song
restarting at hardware frame 894.

The app used the generic PlayMapMusic fade(10) route. The healed song's
selection latch remains set after its channels end, so that route waits
through another fade before starting the center song. Actual five-party
native captures show an extra 89 silent frames after the overlay ends.

RestoreMapMusic is a nurse-specific request. Emit it when the final 32-frame
hold ends, and start the map track directly with saved resume states cleared.
Keep a Done-phase fallback for snapshots restored at that boundary; the app
track-ID guard prevents a duplicate restart. Other map transitions keep their
existing behavior.

After the fix, the song is playing at the overlay-end frame. Two recordings
match exactly, and every one of the 1285 before/after rendered frames matches
byte-for-byte. This is audio-only, so screenshots are not required by the PR
policy. The regression drives the real nurse interaction and verifies the
center track, live channel and no fade at that exact boundary. Numerical
source/native evidence is in verification.json. No complete scene timing
parity is asserted.
