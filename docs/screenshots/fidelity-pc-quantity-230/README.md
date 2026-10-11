# PC quantity question: protected typing and menu handoff

Base is actual master 535c1cf7 (after independently merged PR162). This independent
change addresses only the local "How many?" DONE question for ordinary item
Withdraw, Deposit and Toss. Other PC messages, global inherited text-delay flags,
cry/audio alignment, menu questions and optional paths remain under audit.
Fonts, Chinese wording, input methods and dialogue-box geometry are unchanged.

The original storeChosenEntry clears BIT_NO_TEXT_DELAY before printing HowManyText.
PrintLetterDelay distinguishes held typing keys from menu edges and schedules a
one-frame wait when A/B accelerate typing. The question does not require an
acknowledgement. Original nine glyphs start at relative frame3; ordinary FAST,
MEDIUM and SLOW handoffs are12,30,48. Slow A/B at10..11 yields glyphs
3,8,11,12,17,22,27,32,37 and menu42. Slow inputs at3..4 or8..9 yield menu40;
at47..48 yield48; at48..49 yield49. The final DONE wait must distinguish a
previously scheduled one-frame delay from a newly sampled held key.

The master bypasses typing and accepts quantity input immediately. Early A can
therefore already transfer a Potion, and B can abandon the list while the source
is still printing. The repair adds ItemQuantityPrompt, uses configured delay and
held A/B solely for typing, and only enables quantity menu edges after DONE.
Quantity1 and inventories remain unchanged through the protected interval. Key
items and untossable items retain their existing bypass/refusal paths. Hosts
without audio retain immediate completion of the existing mon-cry boundary.
The existing English question is progressively rendered; its character progress
also invalidates the retained renderer. The existing Chinese wording stays intact.

Evidence: nineteen original controlled cases, each recorded twice, with complete
raw frame/hook data and28166 PNG. Nineteen matching actual Game cases are recorded
for master and candidate, each twice:7524 PNG. Game opens the actual hidden PC at
ViridianPokecenter13,4 facingUP, navigates its menus with input, seeds Bag/PC Potion4,
uses English and the selected speed, and records selection-relative frames0..98.
It never forces a PC phase. Native test checks source glyph/handoff clocks and
unchanged inventory, then fresh A once and a held second frame transfer exactly
one or enter TossConfirm without changing stock. The latter native action check
is not claimed as an original full Toss-warning timing comparison. Each frame
compares cached rendering with complete rendering; all repeated raw JSON/PNG are
byte-identical. A test-only helper on master validly fails at immediate quantity
phase. Unique owned save paths are checked absent throughout these probes.

Complete current-base core/app regression:78 library, binary and integration test
programs,3669 passed,0 failed,74 ignored opt-in tests. Quantity capture tests are
run separately. Source and immutable binary hashes plus complete logs are retained.
Five screenshot pairs use identical actual inputs and relative frame: ordinary
Withdraw/Deposit/Toss20, early Withdraw A20, and last-wait boundary A48. Original
CPU/PPU opening prefixes and native fonts are not compared as absolute pixels or
absolute frame clocks. Controlled inventory/position are not natural acquisition
or a complete first-clear proof. Fresh power-on m01-m03 and a separate-process debug-save/CONTINUE
roundtrip both pass (seed42, speed0). The roundtrip checks serialization and
CONTINUE, rather than the normal SAVE menu input sequence; a full current-master49-milestone first-clear audit remains pending.

The ZIP retains35690 PNG paths losslessly through SHA256-addressed blobs and
png-files.json, plus raw JSON, primary assembly, scripts, source manifests and
full logs. Every PNG and other entry is read back and compared byte-for-byte.
No ROM, SRAM, simulator state or executable is published. Earlier failed fixtures
and historical builds remain preserved in the private working evidence directory.
