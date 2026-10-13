# Keep healing text visible

The original `DisplayPokemonCenterDialogue_` stays inside the text routine
while healing and during the nurse's bow wait. Its printed page remains on
screen until the next message or routine return.

The previous runtime stored the last printed page but exposed it only under
choice menus. The fix retains its display specifically for `talkNurse`,
without keeping an input-consuming dialogue active. The scope ends when
the script completes or a map is loaded. Runtime snapshots preserve the
flag; older snapshots default to false. Font/layout/text content is unchanged.

Both screenshots are from an actual nurse interaction in ViridianPokecenter,
player `(3,3)` facing Up, five party members and the same 40-frame A-button
timeline. They are elapsed frame 1084, during the bow wait. The base capture
was built and recorded on master `fa1e86179e53f141f95dd8c1e317d8304368a2cf`.
The capture helper is already on master.

```sh
NURSE_CAPTURE_DIR=/tmp/healing-text cargo test -p pokered-app --lib capture_nurse_interaction_641 -- --ignored --nocapture
```

The before/after manifests are identical: gameplay, script progress, dialogue
input and nurse pose are unchanged. Only the last-page display is restored.
The reference routine was recorded twice with identical hardware-frame
manifests and PNGs; source identities and comparison bounds are recorded in
`verification.json`. This PR does not assert identical font/layout pixels or
complete healing-machine timing versus the original.

Regression covers the healing machine and bow wait, routine-return cleanup,
unrelated-script isolation, snapshot restoration, legacy-snapshot default,
and a completed map transition. Core and app full library suites pass.
