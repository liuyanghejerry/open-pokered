# PC protected confirmation hold, isolated audit224

DisplayTwoOptionMenu calls DelayFrames15 before restoring tiles and returning
on YES, NO and B cancellation. Original controlled PC inputs in four contexts
(Change Box, Release, PC Toss and Oak rating), eight cases each, repeated twice:
UP40, optional NO64, A/A+B90, interference A/B/DOWN100, relative to choice cue.
All32 cases restore at105 exactly15 after acceptance. Release/Potion mutation
and DisplayDexRating occur105 for YES, never early and not for NO/A+B. All
51648 original PNG and complete raw state/hooks repeat byte-exact. Full rendered
frame80 equals95 in all20 YES/NO/A+B/UP+A/DOWN+A context combinations: the held cursor is
not redrawn to NO even when B already sets the raw cursor byte to1. These are
controlled seeded data and real menu input, not natural acquisition/traversal.

Current master4abcafa returns/commits at90: DOWN100 changes the next box cursor,
A100 can choose the box and save prematurely, B100 can dismiss the receipt.
Fix: two private PcScreen fields hold the prior drawn phase/cursor for15 ticks,
ignore all intervening input and return the frozen B-priority decision once.
Direction+A freezes the old drawn cursor independently of the changed choice,
matching primary HandleMenuInput (moves raw selection before returning watched
keys, without another PlaceMenuCursor). Eight inputs/context include these two
combined-button cases; default tests and timing are not substituted for them.
Transactions, cry queuing, rating and cancellation therefore happen105. Shared
Game uses this same core state machine. No new production renderer/GUI/audio
or save-format code. Existing effect tests explicitly advance15, retaining
all stock/no-save/no-release assertions; cached/full test covers actual YES/NO
cursor movements in all contexts. Existing default/priority owner uses the
source105 boundary, not its previously immediate90 boundary. Dedicated core
once-only/input/timing test and actual Game32-case owner guard the new behavior.

PR163 has merged as master4abcafa. The entire tracked tree of frozen native
parent8a765b7 equals master4abcafa, and all candidate source hashes still match.
This is now an independent master fix; own final CI must pass before merging.
Before is actual master4abcafa production with only identical cfg(test) helpers added,
compiled independently; UP40 explicitly selects YES on both sides, isolating
this wait from the separate default fix. Both sides use collision-checked real
Viridian Center13,4 PC event, Exeggutor13 party/box, PC Potion4, Dex flag,
seed42/English/Medium, then genuine menus. Each32cases x191frames x2 repeats
on each side =24448 native/master PNG. Every frame compares cached/full pixels;
all PNG and complete JSON are byte-exact within side. Positive owner holds
phase90..104, stock changes105, early A cannot flip rating or save, early DOWN
cannot choose another box. Master negative fails immediate phase90. Paired
images use same fixture/input/relative frame95 or100, including
UP+A held NO but committed YES and DOWN+A held YES but returned NO; defaults already match after merged PR163; this PR changes only the hold.

Initial master a1 capture completed but failed the exact-repeat verifier:
uncontrolled default-save loading after the premature A100 save polluted the
next fixture. All failed logs/PNG/JSON and private default SRAM remain in the
workspace. Final a4 master/a3 native uses a new explicit unique owned save path per case via
new_with_options(no audio), asserts initially empty party/box, and records
owned_save_exists. Old change/yes-a first writes100; candidate never writes
in these inputs. Private SRAM/flags are removed after each completed case and
never archived. Every final case starts fresh, preserving the same real save
operation rather than disabling saves or masking the observed side effect.

Evidence ZIP retains76096 valid original/master/candidate PNG paths losslessly
as SHA256 blobs, complete raw state/hooks, primary assembly, source/binary hash
manifests, scripts and full diagnostic/regression logs. Every archived file is
read back and compared byte-for-byte. No ROM, emulator state, SRAM or executable
is published. Native core2725/app202 pass(26 opt-in captures ignored). These
probes prove this controlled acceptance/hold/commit boundary, not absolute
originalCPU/PPU alignment, natural mainline traversal, full SAVE/CONTINUE,
PC paragraph/typing/cry-length/audio order, global repeat or layout fidelity;
those audits remain open. Do not replace this scope proof with a broad PC claim.
