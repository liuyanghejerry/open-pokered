"""Rod encounter factors distinguish no bite, registered fish and capture."""
import unittest

from openpokered.collection_planner import (
    SUPER_ROD_GROUPS, SUPER_ROD_MAP_GROUP, fishing_profile,
)


class FishingProfileTests(unittest.TestCase):
    def test_old_rod_fixed_encounter_is_unchanged(self):
        value = fishing_profile('OldRod', 'PalletTown')
        self.assertEqual((value['bite_probability_pct'], value['no_bite_per_attempt_pct']), (100., 0.))
        self.assertEqual((value['targets'][0]['species'], value['targets'][0]['expected_attempts']),
                         ('Magikarp', 1.))
        self.assertIn('not capture probability', value['scope'])

    def test_good_rod_redraws_make_each_fish_one_in_six_per_cast(self):
        value = fishing_profile('GoodRod', 'PalletTown')
        self.assertEqual(value['bite_probability_pct'], 33.3)
        self.assertEqual(value['no_bite_per_attempt_pct'], 66.7)
        self.assertEqual({row['per_attempt_pct'] for row in value['targets']}, {16.7})
        self.assertEqual({row['expected_attempts'] for row in value['targets']}, {6.})
        self.assertEqual(value['expected_attempts_to_any_new_species'], 3.)

    def test_one_owned_fish_is_distinct_from_no_bite(self):
        value = fishing_profile('GoodRod', 'PalletTown', ['Goldeen'])
        self.assertEqual(value['new_species_per_attempt_pct'], 16.7)
        self.assertEqual(value['registered_bite_per_attempt_pct'], 16.7)
        self.assertEqual(value['no_bite_per_attempt_pct'], 66.7)
        self.assertEqual(value['unregistered_encounter_share_pct'], 50.)
        self.assertEqual(value['duplicate_encounter_share_pct'], 50.)
        self.assertEqual(value['expected_attempts_to_any_new_species'], 6.)
        self.assertEqual(value['targets'][0]['encounter_share_pct'], 50.)

    def test_all_fish_owned_has_no_new_registration_claim(self):
        value = fishing_profile('GoodRod', 'PalletTown', ['Goldeen', 'Poliwag'])
        self.assertEqual(value['targets'], [])
        self.assertEqual(value['new_species_per_attempt_pct'], 0.)
        self.assertIsNone(value['expected_attempts_to_any_new_species'])
        self.assertEqual(value['registered_bite_per_attempt_pct'], 33.3)
        self.assertEqual(value['duplicate_encounter_share_pct'], 100.)

    def test_two_entry_super_rod_matches_good_rod_bite_model(self):
        value = fishing_profile('SuperRod', 'PalletTown')
        self.assertEqual(value['bite_probability_pct'], 33.3)
        self.assertEqual({row['expected_attempts'] for row in value['targets']}, {6.})

    def test_three_entry_super_rod_each_entry_is_one_in_seven(self):
        value = fishing_profile('SuperRod', 'CeruleanCity', ['Psyduck', 'Goldeen'])
        self.assertEqual(value['bite_probability_pct'], 42.9)
        self.assertEqual(value['no_bite_per_attempt_pct'], 57.1)
        self.assertEqual(value['targets'][0]['species'], 'Krabby')
        self.assertEqual(value['targets'][0]['per_attempt_pct'], 14.3)
        self.assertEqual(value['expected_attempts_to_any_new_species'], 7.)

    def test_all_four_entry_tables_keep_half_bite_cast_yield(self):
        for map_name, group in SUPER_ROD_MAP_GROUP.items():
            if len(SUPER_ROD_GROUPS[group]) != 4:
                continue
            with self.subTest(map=map_name):
                value = fishing_profile('SuperRod', map_name)
                self.assertEqual(value['bite_probability_pct'], 50.)
                self.assertEqual(value['new_species_per_attempt_pct'], 50.)
                self.assertEqual(value['no_bite_per_attempt_pct'], 50.)
                self.assertEqual(value['expected_attempts_to_any_new_species'], 2.)
                self.assertEqual({row['expected_attempts'] for row in value['targets']}, {8.})
                self.assertEqual({row['encounter_share_pct'] for row in value['targets']}, {25.})

    def test_unknown_rod_or_missing_super_table_stays_unavailable(self):
        self.assertIsNone(fishing_profile('SuperRod', 'Route1'))
        self.assertIsNone(fishing_profile('UnknownRod', 'PalletTown'))


if __name__ == '__main__':
    unittest.main()
