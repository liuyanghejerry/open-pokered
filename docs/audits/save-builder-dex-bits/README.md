# SaveBuilder Pokédex bit positions

`SaveBuilder.dex("Bulbasaur")` set bit 1, marking Ivysaur instead of Bulbasaur.
Dex numbers are one-based; the engine and SRAM store species number minus one
as the bit index. The helper now applies that conversion locally. Other uses
of `species_id`, including starter IDs, retain their existing convention.

`dex_complete()` now writes all 151 valid flags and clears the unused final
bit: eighteen `FF` bytes followed by `7F`. Previously it left Bulbasaur missing
and set the invalid 152nd bit. The engine's raw count could still report 151,
so checking only that count concealed the incorrect species list.

Four regression tests cover literal SRAM positions for Bulbasaur, Wartortle,
Blastoise and Mew; independent seen/owned edits and adjacent flags; completion
with preexisting padding; and rejection without mutation. Before the fix,
three failed. All four plus the two existing item-name tests now pass:

```sh
python3 -m unittest discover -s scripts -p 'test_save_builder_*.py' -v
```

Actual engine controls use the same clean frozen master `72912bc1` binary
(debug-server, isolated target888, opt0/debug0). Each helper produces private
preboot snapshots; there are no validation-time state setters. All four boots
read actual `evaluation.pokedex.owned_numbers`, then use ordinary
START → SAVE → YES and independently import/check the resulting SRAM.

| Helper case | Before: actual owned IDs | After: actual owned IDs |
| --- | --- | --- |
| Bulbasaur, Wartortle, Blastoise, Mew | 2, 9, 10; raw count 4 | 1, 8, 9, 151; count 4 |
| Complete dex | 2–151; raw count 151 | 1–151; count 151 |

Saved flags match the generated snapshots, main SRAM checksums are valid,
and party data is unchanged. The later parent `d0873d89` changes evolution
stone use, not the engine Pokédex implementation. The frozen controls remain
labeled `72912bc1`; they are not relabeled as a later binary or full playthrough.

[verification.json](verification.json) records exact helper/engine/fixture/save
hashes and limits. `evidence.zip` retains the full capture script, original
helper source, four snapshots, command timelines, save/readback files and logs.
[The entry manifest](evidence-entry-manifest.json) hashes every archive member.
No ROM or executable is included. The archived capture script keeps its
explicit private paths and is provenance, not a portable public test runner.

Historical fixture bytes and captures are preserved. Their intended labels
must be checked against actual flags; this correction does not retroactively
change them. The 880 wild-capture matrix manually used the correct Weedle bit,
so its 32 controls are unaffected. The early 889 Safari fixture-label mismatch
is retained and corrected in its separate final controls.

This is tooling and tests. Production gameplay, rendering and on-screen output
are unchanged, so the AGENTS.md screenshot requirement does not apply.
