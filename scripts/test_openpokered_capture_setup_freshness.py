"""Historical failed setup is evidence, not a verdict on newly held tools."""
from copy import deepcopy
import unittest

from openpokered import autonomous_story as story


class CaptureSetupFreshnessTests(unittest.TestCase):
    def party(self):
        return [
            {'species': 'Parasect', 'level': 31, 'hp': 83, 'max_hp': 83,
             'status': 'None', 'experience': 29979,
             'moves': ['Scratch', 'StunSpore', 'LeechLife', 'Spore'],
             'pp': [35, 30, 15, 15]},
            {'species': 'Charizard', 'level': 66, 'hp': 227, 'max_hp': 227,
             'status': 'None', 'moves': ['Slash'], 'pp': [20]},
            {'species': 'Hypno', 'level': 42, 'hp': 143, 'max_hp': 143,
             'status': 'None', 'moves': ['Hypnosis'], 'pp': [20]},
        ]

    def failure(self):
        return {'map': 'PowerPlant', 'species': 'Zapdos', 'level': 50,
                'observation': {'party': [
                    {'species': 'Charizard', 'level': 57, 'hp': 0, 'max_hp': 192,
                     'status': 'None', 'moves': ['Slash'], 'pp': [20]},
                    {'species': 'Drowzee', 'level': 13, 'hp': 0, 'max_hp': 41,
                     'status': 'None', 'moves': ['Pound', 'Hypnosis'], 'pp': [35, 20]},
                ]}}

    def reference(self, party, failures):
        builder = getattr(story, 'capture_setup_history_reference', None)
        self.assertTrue(callable(builder), 'History/setup comparison must be explicit')
        tools = story.capture_support_level_reference(party[0], 32, party, ['Parasect'])[
            'current_conscious_non_damaging_support_tools']
        return builder(party, tools, failures)

    def test_actual_learned_spore31_to32_is_stat_only_not_another_status_unlock(self):
        party = self.party()
        ref = story.capture_support_level_reference(party[0], 32, party, ['Parasect'])
        self.assertEqual(ref.get('selected_step_outcome_type'), 'stats_or_other_move_offers_only')
        self.assertEqual(ref.get('selected_step_new_capture_status_move_offers'), [])
        self.assertEqual(ref['next_capture_status_move_offers'], [])
        self.assertIn('Spore', [tool['move'] for tool in ref['current_conscious_non_damaging_support_tools']])

    def test_spore30_is_an_offer_not_a_known_tool_before_learning(self):
        mon = self.party()[0]
        mon.update(level=29, experience=26923, moves=['Scratch', 'StunSpore', 'LeechLife', 'None'])
        ref = story.capture_support_level_reference(mon, 30, [mon], ['Parasect'])
        self.assertEqual(ref.get('selected_step_outcome_type'), 'capture_status_move_offer')
        self.assertEqual([row['move'] for row in ref['selected_step_new_capture_status_move_offers']], ['Spore'])
        self.assertNotIn('Spore', [row['move'] for row in ref['current_conscious_non_damaging_support_tools']])

    def test_unregistered_level_evolution_is_separate_from_status_unlock(self):
        mon = {**self.party()[0], 'species': 'Geodude', 'level': 24,
               'moves': ['Tackle'], 'pp': [35]}
        ref = story.capture_support_level_reference(mon, 25, [mon], ['Geodude'])
        self.assertEqual(ref.get('selected_step_outcome_type'), 'unregistered_evolution_offer')
        self.assertEqual(ref['unregistered_level_evolution_offers'][0]['species'], 'Graveler')

    def test_held_zero_pp_offer_is_not_a_new_learning_capability(self):
        mon = self.party()[0]
        mon.update(level=29, experience=26923)
        mon['pp'][3] = 0
        ref = story.capture_support_level_reference(mon, 30, [mon], ['Parasect'])
        self.assertEqual(ref.get('selected_step_outcome_type'), 'stats_or_other_move_offers_only')
        self.assertEqual(ref['selected_step_new_capture_status_move_offers'], [])
        self.assertNotIn('Spore', [row['move'] for row in ref['current_conscious_non_damaging_support_tools']])

    def test_actual_old_zapdos_roster_did_not_test_current_spore_or_hypno_tools(self):
        row = self.reference(self.party(), [self.failure()])[0]
        self.assertTrue(row['historical_party_snapshot_available'])
        self.assertEqual(row['failed_capture_species'], 'Zapdos')
        self.assertEqual(row['current_conscious_tool_pairs_not_in_failure_party'], [
            {'species': 'Hypno', 'move': 'Hypnosis'},
            {'species': 'Parasect', 'move': 'Spore'},
            {'species': 'Parasect', 'move': 'StunSpore'}])
        changes = row['same_species_level_and_max_hp_changes']
        char = next(change for change in changes if change['species'] == 'Charizard')
        self.assertEqual((char['level_difference'], char['max_hp_difference']), (9, 35))
        self.assertIsNone(next(change for change in changes if change['species'] == 'Hypno')['level_difference'])
        self.assertIn('not a current-setup failure', row['scope'])
        self.assertIn('not individual identity', row['scope'])

    def test_same_current_tool_pair_is_not_falsely_called_absent_because_old_hp_was_zero(self):
        failure = self.failure()
        failure['observation']['party'].append(deepcopy(self.party()[0]))
        failure['observation']['party'][-1]['hp'] = 0
        failure['observation']['party'][-1]['pp'][3] = 0
        row = self.reference(self.party(), [failure])[0]
        self.assertEqual(row['current_conscious_tool_pairs_not_in_failure_party'],
                         [{'species': 'Hypno', 'move': 'Hypnosis'}])
        spore = next(tool for tool in row['historical_status_tool_observations'] if tool['move'] == 'Spore')
        self.assertEqual((spore['hp'], spore['pp']), (0, 0))

    def test_missing_failure_party_is_unknown_not_proof_that_tools_were_absent(self):
        failure = self.failure()
        del failure['observation']['party']
        row = self.reference(self.party(), [failure])[0]
        self.assertFalse(row['historical_party_snapshot_available'])
        self.assertIsNone(row['current_conscious_tool_pairs_not_in_failure_party'])
        self.assertIsNone(row['historical_status_tool_observations'])

    def test_missing_old_hp_and_pp_remain_unknown(self):
        failure = self.failure()
        del failure['observation']['party'][1]['hp']
        del failure['observation']['party'][1]['pp']
        row = self.reference(self.party(), [failure])[0]
        tool = row['historical_status_tool_observations'][0]
        self.assertIsNone(tool['hp'])
        self.assertIsNone(tool['pp'])

    def test_null_old_pp_remains_unknown_without_hiding_the_held_move(self):
        failure = self.failure()
        failure['observation']['party'][1]['pp'] = None
        row = self.reference(self.party(), [failure])[0]
        self.assertEqual(row['historical_status_tool_observations'][0]['move'], 'Hypnosis')
        self.assertIsNone(row['historical_status_tool_observations'][0]['pp'])

    def test_missing_old_moves_do_not_prove_that_current_tools_were_absent(self):
        failure = self.failure()
        failure['observation']['party'][1]['moves'] = None
        row = self.reference(self.party(), [failure])[0]
        self.assertTrue(row['historical_party_snapshot_available'])
        self.assertFalse(row['historical_move_roster_complete'])
        self.assertIsNone(row['current_conscious_tool_pairs_not_in_failure_party'])

    def test_invalid_old_stats_are_not_used_as_a_training_delta(self):
        failure = self.failure()
        failure['observation']['party'][0].update(level=0, max_hp=-1)
        row = self.reference(self.party(), [failure])[0]
        char = next(change for change in row['same_species_level_and_max_hp_changes'] if change['species'] == 'Charizard')
        self.assertIsNone(char['level_difference'])
        self.assertIsNone(char['max_hp_difference'])

    def test_duplicate_same_species_does_not_invent_one_individual_stat_delta(self):
        party = self.party()
        failure = self.failure()
        failure['observation']['party'].append(deepcopy(failure['observation']['party'][0]))
        row = self.reference(party, [failure])[0]
        char = next(change for change in row['same_species_level_and_max_hp_changes'] if change['species'] == 'Charizard')
        self.assertEqual(len(char['historical_same_species_observations']), 2)
        self.assertIsNone(char['level_difference'])
        self.assertIsNone(char['max_hp_difference'])

    def test_inputs_and_failure_evidence_are_unchanged(self):
        party, failures = self.party(), [self.failure()]
        before = deepcopy((party, failures))
        self.reference(party, failures)
        self.assertEqual((party, failures), before)


if __name__ == '__main__':
    unittest.main()
