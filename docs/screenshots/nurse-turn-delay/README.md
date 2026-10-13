# Nurse pre-healing delay

DisplayPokemonCenterDialogue_ turns the nurse toward the machine and calls
Delay3. Delay3 sets C=3 and jumps to DelayFrames. Actual original captures
show turn at frame 531, Delay3 return at 534 and machine entry at 535.

All twelve center scenes, the shared scene and the old JS fallback used 8.
Change only the wait immediately after faceNpc(left) to 3. The JS fallback's
other post-heal behavior is outside this patch and still needs audit.

All twelve native center interactions count down 3,2,1 before the machine.
Two full five-party app recordings match exactly. Same input/frame 650
screenshots show the first ball arriving five frames earlier. Base is #207;
refresh against merged master before merging this dependent patch.

The native machine entry is still five updates after the nurse turn, versus
four hardware frames in the original. Script dispatch/publication remains
unproven; do not infer full animation parity from this constant correction.
Source identities and numeric evidence are in verification.json.

Follow-up: the source party has mixed moves while the native fixture uses
five Tackle-only members. HealParty performs move-dependent AddNTimes work
when restoring PP. The measured one-frame startup gap is not a confirmed
dispatch bug in this mismatched setup. Match the party/moves/PP-up inputs
before judging full startup timing. Delay3 itself is verified independently.
