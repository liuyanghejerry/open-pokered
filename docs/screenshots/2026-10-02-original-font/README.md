# Original font and text metrics

The `*-before.png` captures use the preserved audit-base binary at `72ff719b39634c153cb82d3f3ece200bd413c4e0`. Capture the matching `*-after.png` images with the final integrated binary, at the same ten frames:

```sh
pokered-app screenshot --screen main-menu --frames 10 --lang en -o main-menu-after.png
pokered-app screenshot --screen options --frames 10 --lang en -o options-after.png
pokered-app screenshot --screen naming --frames 10 --lang en -o naming-after.png
```

Original glyphs and opaque border tiles come from the pinned pret/pokered PNGs; conversion provenance is in `crates/pokered-renderer/fonts/POKERED.md`. The renderer chooses metrics by character: printable ASCII/original charmap characters use 8px advances in both UI languages; Chinese keeps its existing Fusion Pixel bitmap and 10px advance. No global language-dependent font switch is introduced.

Overworld and battle text use the same metrics for pagination and drawing. Authored newlines/page boundaries remain hard; expanded rows wrap at 144px before two-line paging. Tests cover exact eighteen-cell English rows, Chinese/ASCII number runs, overlong words, punctuation, placeholder growth, and pixels next to the right border. Chinese stats ID/OT values and shop name/price rows receive space for the wider ASCII cells. Original money/price fields suppress leading zeroes and remain right aligned; level 100 omits its level prefix as in `PrintLevel`.

Focused validation: the renderer/data suites, all UI library/integration tests, and core dialogue typewriter/battle pagination tests. These captures document visible changes; they do not claim an emulator pixel match of every screen.
