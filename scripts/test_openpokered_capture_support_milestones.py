"""Future natural support offers are explicit costs, not learned tools/rewards."""
from copy import deepcopy
import unittest
from unittest.mock import patch

from openpokered.autonomous_story import capture_support_level_reference, data


class CaptureSupportMilestoneTests(unittest.TestCase):
    def mon(self):
        return {'species': 'Parasect', 'level': 28, 'hp': 70, 'max_hp': 75,
                'status': 'None', 'experience': 23173,
                'moves': ['Scratch', 'StunSpore', 'LeechLife', 'None'],
                'pp': [22, 30, 10, 0]}

    def test_actual28_to29_reports_spore30_and_full_remaining_cost(self):
        mon = self.mon()
        value = capture_support_level_reference(mon, 29, [mon], ['Parasect'])
        self.assertEqual(value['natural_move_offers'], [])
        self.assertIn('next_capture_status_move_offers', value)
        offer = value['next_capture_status_move_offers'][0]
        self.assertEqual((offer['level'], offer['move'], offer['effect'], offer['base_accuracy']),
                         (30, 'Spore', 'SleepEffect', 100))
        self.assertEqual(offer['levels_remaining'], 2)
        self.assertFalse(offer['selected_target_reaches_offer'])
        self.assertEqual(offer['training_cost']['remaining_experience_min'], 3827)
        self.assertEqual(offer['training_cost']['remaining_experience_max'], 3827)
        self.assertIn('not guaranteed to be learned', offer['scope'])
        self.assertIn('not a hit or capture guarantee', offer['scope'])

    def test_target30_has_both_immediate_offer_and_same_future_milestone(self):
        mon = self.mon()
        value = capture_support_level_reference(mon, 30, [mon], ['Parasect'])
        self.assertEqual(value['natural_move_offers'][0]['move'], 'Spore')
        self.assertTrue(value['next_capture_status_move_offers'][0]['selected_target_reaches_offer'])
        self.assertNotIn('Spore', [tool['move'] for tool in value['current_conscious_non_damaging_support_tools']])

    def test_known_spore_with_zero_pp_is_recovery_not_a_future_unlock(self):
        mon = self.mon()
        mon['moves'][3] = 'Spore'
        value = capture_support_level_reference(mon, 29, [mon], ['Parasect'])
        self.assertIn('next_capture_status_move_offers', value)
        self.assertEqual(value['next_capture_status_move_offers'], [])
        self.assertNotIn('Spore', [tool['move'] for tool in value['current_conscious_non_damaging_support_tools']])

    def test_ordinary_stat_or_damaging_offer_is_not_a_capture_status_milestone(self):
        mon = {**self.mon(), 'species': 'Hypno', 'level': 42, 'experience': 74261,
               'moves': ['Headbutt', 'Hypnosis', 'Disable', 'PsychicM'], 'pp': [15, 20, 20, 10]}
        value = capture_support_level_reference(mon, 43, [mon], ['Hypno'])
        self.assertEqual(value['natural_move_offers'][0]['move'], 'Meditate')
        self.assertIn('next_capture_status_move_offers', value)
        self.assertEqual(value['next_capture_status_move_offers'], [])
        self.assertFalse(value['direct_ball_roll_uses_support_level'])
        self.assertFalse(value['existing_move_base_accuracy_changes_with_level'])

    def test_missing_xp_stays_a_bound_and_inputs_and_current_tools_are_unchanged(self):
        mon = self.mon()
        del mon['experience']
        hypno = {**mon, 'species': 'Hypno', 'level': 42, 'moves': ['Hypnosis'], 'pp': [20]}
        party, owned = [mon, hypno], ['Parasect', 'Hypno']
        before = deepcopy((party, owned))
        value = capture_support_level_reference(mon, 29, party, owned)
        self.assertIn('next_capture_status_move_offers', value)
        cost = value['next_capture_status_move_offers'][0]['training_cost']
        self.assertLess(cost['remaining_experience_min'], cost['remaining_experience_max'])
        self.assertEqual((party, owned), before)
        self.assertEqual([tool['move'] for tool in value['current_conscious_non_damaging_support_tools']],
                         ['StunSpore', 'Hypnosis'])

    def test_all_status_offers_at_nearest_level_remain_visible(self):
        mon = self.mon()
        catalog = deepcopy(data.species_data('Parasect'))
        catalog['learnset'] = [{'level': 30, 'moveId': 'Spore'},
                              {'level': 30, 'moveId': 'SleepPowder'},
                              {'level': 39, 'moveId': 'Hypnosis'}]
        with patch.object(data, 'species_data', return_value=catalog):
            value = capture_support_level_reference(mon, 29, [mon], ['Parasect'])
        self.assertEqual([offer['move'] for offer in value['next_capture_status_move_offers']],
                         ['Spore', 'SleepPowder'])


if __name__ == '__main__':
    unittest.main()
