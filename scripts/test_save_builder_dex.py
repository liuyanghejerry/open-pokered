"""Fixture dex flags must use the engine/SRAM's zero-based bit positions."""
import copy
import unittest

from save_builder import SaveBuilder


def builder(seen=None, owned=None):
    result = SaveBuilder.__new__(SaveBuilder)
    result.data = {"game_data": {"pokedex": {
        "seen": list(seen) if seen is not None else [0] * 19,
        "owned": list(owned) if owned is not None else [0] * 19,
    }}}
    return result


class DexFlagRegression(unittest.TestCase):
    def test_first_byte_boundary_and_last_species_use_their_sram_bits(self):
        expected = [0x81, 0x01] + [0] * 16 + [0x40]
        dex = builder()
        for species in ["Bulbasaur", "Wartortle", "Blastoise", "Mew"]:
            dex.dex(species)
        self.assertEqual(dex.data["game_data"]["pokedex"], {
            "seen": expected, "owned": expected,
        })

    def test_seen_and_owned_edits_preserve_adjacent_species(self):
        dex = builder([0x81] + [0] * 18, [0x81] + [0] * 18)
        dex.dex("Bulbasaur", seen=True, owned=False)
        self.assertEqual(dex.data["game_data"]["pokedex"]["seen"], [0x81] + [0] * 18)
        self.assertEqual(dex.data["game_data"]["pokedex"]["owned"], [0x80] + [0] * 18)
        dex.dex("Weedle", seen=True, owned=False)
        self.assertEqual(dex.data["game_data"]["pokedex"]["seen"], [0x81, 0x10] + [0] * 17)
        self.assertEqual(dex.data["game_data"]["pokedex"]["owned"], [0x80] + [0] * 18)

    def test_complete_dex_includes_bulbasaur_and_clears_the_unused_last_bit(self):
        dex = builder([0] * 18 + [0x80], [0] * 18 + [0x80])
        dex.dex_complete()
        expected = [0xFF] * 18 + [0x7F]
        self.assertEqual(dex.data["game_data"]["pokedex"], {
            "seen": expected, "owned": expected,
        })
        self.assertEqual(sum(byte.bit_count() for byte in expected), 151)

    def test_unknown_species_does_not_change_the_fixture(self):
        dex = builder([0x81] + [0] * 18, [0x80] + [0] * 18)
        original = copy.deepcopy(dex.data)
        with self.assertRaises(KeyError):
            dex.dex("MissingNo")
        self.assertEqual(dex.data, original)


if __name__ == "__main__":
    unittest.main()
