# Oak approach routes

FindPathToPlayer takes the larger remaining distance, with X winning ties.
Natural original NEW GAME captures at both northern exit columns verify
five destinations for column 10 and six for column 11. The original also
moves Oak and the player left before the common lab escort from column 11.
The current generic moveNpcTo route instead takes one axis to completion.

Change the Pallet scene to use the verified approach destinations and
explicit right-column alignment. No renderer or movement-clock changes.
A regression observes each real NPC step start, compares all destinations
to the original raw capture oracle, and requires player (10,1) at escort
entry. Original hardware timestamps stay in the oracle but are not claimed
matched by this geometry-only fix.

The base is master 934ba3db. Base and fix share the same ignored capture
helper, initial pixels, inputs and frame numbers. Screenshots are x10 frame
202 and x11 frame 186; both use the same walk counter 10 and player position.
Each fixed capture was repeated exactly, and both complete the lab entry.
Core 2800 and all 41 app test executables pass. Complete animation pacing,
NPC publication and the later final northbound Oak step still need audit.
