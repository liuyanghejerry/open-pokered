# Bicycle music dispatch

Original source: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c, engine/items/item_effects.asm:638–667. Both successful transport changes call PlayDefaultMusic before printing their result. Refused uses leave transport and music unchanged.

The native Bag handler already invokes use_field_item and returns to the field. It neither restarts map music on Bag return nor queues it in the Bicycle branches. The existing PlayMapMusic audio consumer selects MUSIC_BIKE_RIDING (32) while biking and the current map theme while walking. This repair queues that existing request only after a successful transport change; the existing field-item preservation flag retains it through the first field frame.

Regression exercises both mount and dismount, key-item non-consumption, request survival on the first update, and absence on the following update. Indoor, surfing, and forced-bike refusals retain transport and issue no request. Test-only additions against unchanged master production fail on the missing request. No PCM, sample-exact or CPU/PPU timing claim is made; existing music-handler fade policy is preserved.

This is an audio-only change, so the repository screenshot policy does not require visual captures. Direct Bicycle selection and refusal returning to the Bag list are separate, unfinished audit 211 work.

Validation on base 6435d31: full native core suite **2716 passed**. Frozen manifests and full logs are in evidence.zip. Initial a1 setup had a wrong helper working directory and missing gfx symlink, and failed compilation; retained as excluded diagnostics. Corrected test-only a2 build passed, then the new music test failed (exit 101) on unchanged production. Fixed a1 build and full suite both passed. Evidence excludes executable binaries, ROMs and save data.

After syncing master 6d99bd6 (#152), a fresh a2 build and full native core suite passed **2716 tests**, no failures/ignored. Its frozen manifest and full logs are also archived.
