# Indigo Plateau nurse English message

IndigoPlateauLobbyNurseText uses script_pokecenter_nurse in the original,
so it prints the common _PokemonFightingFitText (data/text/text_4.asm), not
an alternate fully-healed message. Replace only the English first argument
of the localized literal. All ten Chinese literals remain byte-identical.

The existing all-center nurse regression no longer accepts fully healed;
it requires fighting fit before the bow on every center map. The capture
helper accepts NURSE_CAPTURE_MAP=indigo and derives the standing position
from the nurse object, retaining the same input/frame timeline and five
party members in both base and fix.

Before/after are the same frame 1150 of actual nurse interactions. Two full
fixed captures match byte-for-byte. Differences are confined to the text
box. Base is #208; refresh against merged master before merging this
patch. The text-preservation tool intentionally flags this corrected
English difference against the old HEAD; Chinese preservation was checked
separately. No font, Chinese or textbox-layout changes are intended.
