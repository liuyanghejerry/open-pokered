# Nurse bow after healing

The original common Pokemon Center routine writes nurse image `$18` for
the machine-facing pose. After the fighting-fit message it writes `$14`,
the standing-up sheet image used as her bow, waits 20 frames, then prints
the farewell. Normal sprite updating restores her ordinary facing on exit.

The old scenes turned down immediately after the machine and never showed
the bow. All twelve local nurse scenes plus the shared fallback now set
sprite frame 1 after the health confirmation, keep it through the farewell,
and clear the frame override by restoring Down afterwards. Text is unchanged.

`before.png` and `after.png` use the same real nurse interaction fixture,
five party members, player `(3,3)` facing Up in ViridianPokecenter, 120 idle
setup frames, and the same A-button timeline. Both are elapsed frame 1110.
The before build is master `5da13d1bb85e61d132307dca12f7eb16ee421a64`;
the identical ignored capture helper was added to both builds.

```sh
NURSE_CAPTURE_DIR=/tmp/nurse-capture cargo test -p pokered-app --lib capture_nurse_interaction_641 -- --ignored --nocapture
```

`reference-bow.png` is the original nurse region at `(64,28)`, 16x16 pixels,
from an actual retail healing interaction after SAVE/CONTINUE and ordinary
walking to the center. Five party members were healed. Both full original
recordings are exactly repeatable, and the fixed nurse region matches all
256 reference pixels. `verification.json` records ROM/source identities,
image hashes and exact comparison scope; no ROM/save is committed.

This PR restores the missing pose and its story order. It does not claim
complete healing animation timing parity. The existing pre-heal delay 8
versus original Delay3, retained text during script waits, and command/frame
publication timing remain separate fidelity audit items.

Validation: full core/app library suites and a twelve-location regression
check health message -> bow -> farewell -> cleared override/Down. The native
reference comparison is a static pose check, not a whole-frame timing PASS.
