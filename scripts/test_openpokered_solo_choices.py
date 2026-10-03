"""Observed irreversible source choices, distinct from Pokémon receipts."""
import json
import unittest
from unittest.mock import Mock

from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.collection_planner import (
    SUPER_ROD_MAP_GROUP, complete_acquisition_graph, infer_solo_choices, solo_plan,
)
from openpokered.story_rules import MAPS_DIR


class ObservedSoloChoiceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        maps = {path.parent.name: json.loads(path.read_text())
                for path in MAPS_DIR.glob('*/map.json')}
        cls.graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)

    def test_held_fossil_uses_observed_branch_before_any_receipt(self):
        for item in ('HELIX_FOSSIL', 'HelixFossil', 'HELIXFOSSIL'):
            with self.subTest(item=item):
                self.assertEqual(infer_solo_choices(['Charmander'], bag={item: 1}),
                                 {'starter': 'Charmander', 'fossil': 'Omanyte'})

    def test_committed_fossil_survives_consumed_item_and_resume(self):
        for flag, branch in (
                ('EVENT_GOT_DOME_FOSSIL', 'Kabuto'),
                ('EVENT_GOT_HELIX_FOSSIL', 'Omanyte'),
                ('EVENT_REVIVING_KABUTO', 'Kabuto'),
                ('EVENT_REVIVING_OMANYTE', 'Omanyte')):
            with self.subTest(flag=flag):
                self.assertEqual(infer_solo_choices([], bag={}, flags={flag: True}),
                                 {'fossil': branch})

    def test_unknown_or_unselected_resources_do_not_commit_a_choice(self):
        for quantity in (0, -1, False, True, None, '1'):
            with self.subTest(quantity=quantity):
                self.assertEqual(infer_solo_choices([], bag={'HELIXFOSSIL': quantity}), {})
        self.assertEqual(infer_solo_choices([], bag={'THUNDERSTONE': 1, 'OLDAMBER': 1},
            flags={'EVENT_GAVE_FOSSIL_TO_LAB': True, 'EVENT_GOT_HELIX_FOSSIL': False,
                   'EVENT_REVIVING_OMANYTE': 'true'}), {})

    def test_conflicting_observations_do_not_silently_choose_a_branch(self):
        for owned, bag, flags in (
                ([], {'DOMEFOSSIL': 1, 'HELIXFOSSIL': 1}, {}),
                (['Kabutops'], {'HELIXFOSSIL': 1}, {}),
                ([], {}, {'EVENT_GOT_DOME_FOSSIL': True, 'EVENT_GOT_HELIX_FOSSIL': True})):
            with self.subTest(owned=owned, bag=bag, flags=flags):
                self.assertNotIn('fossil', infer_solo_choices(owned, bag=bag, flags=flags))

    def test_registered_descendants_and_uncommitted_choices_are_unchanged(self):
        self.assertEqual(infer_solo_choices(['Charizard', 'Omastar', 'Hitmonlee', 'Vaporeon']),
            {'starter': 'Charmander', 'fossil': 'Omanyte', 'dojo': 'Hitmonlee',
             'eevee_evolution': 'Vaporeon'})
        plan = solo_plan(self.graph, ['Charmander'], infer_solo_choices(['Charmander']))
        self.assertEqual(plan['optimal_choices']['fossil'], ['Kabuto', 'Omanyte'])
        self.assertEqual(plan['ceiling'], 124)

    def panel_agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex, agent.index, agent.collection_audit_pending = True, Mock(), {}
        agent.collection_graph = Mock(return_value={})
        agent.complete_collection_graph = Mock(return_value=self.graph)
        agent.neighbourhood = Mock(return_value=[])
        agent.balls_held = Mock(return_value=0)
        agent.collection_resources = Mock(return_value={})
        agent.nearest_ball_source = Mock(return_value=None)
        return agent

    def test_collection_panel_uses_commitment_without_crediting_registration(self):
        observed = {'dex': {'owned': 1, 'owned_species': ['Charmander'],
                            'seen': 1, 'seen_species': ['Charmander']},
                    'bag': {'HELIXFOSSIL': 1}, 'flags': {'EVENT_GOT_HELIX_FOSSIL': True}}
        panel = self.panel_agent().dex_progress(observed)
        self.assertEqual(panel['solo_choices']['fossil'], 'Omanyte')
        self.assertEqual(panel['solo_choice_options']['fossil'], ['Omanyte'])
        self.assertEqual(panel['solo_target_count'], 124)
        self.assertEqual(panel['solo_owned'], 1)
        self.assertEqual(panel['solo_remaining'], 123)
        self.assertIn('Kabuto', panel['always_unreachable_species'])
        self.assertNotIn('Omanyte', panel['policy_unreachable_species'])


if __name__ == '__main__':
    unittest.main()
