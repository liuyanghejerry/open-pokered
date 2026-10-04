"""Field-planning failure must not fabricate unrelated paid scene prerequisites."""
from copy import deepcopy
import json
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import test_openpokered_story as fixtures
from openpokered.autonomous_story import (AutonomousStoryAgent,
                                        navigation_failure_is_field_prerequisite)
from openpokered.story_rules import Rule


class NavigationFailureKindTests(unittest.TestCase):
    def fixture(self, detail, obstacle=None):
        flag = 'OPTIONAL_PAID_SESSION'
        guard = fixtures.call('getFlag', flag)
        movement = Rule('gate', 'Room', 'Room:gate', ['coord:gate'], [(guard, False)], [],
                        ('movement', 'movePlayerRelative', True), [])
        unlock = ('flag', flag, True)
        producer = Rule('entry', 'Room', 'Room:pay', ['npc:1'], [], ['YES'], unlock, [])
        parent = ('flag', 'DISTANT_GYM_WIN', True)
        blockage = {'map': 'Room', 'position': [3, 3], 'destination': 'FarGym',
                    'goal': parent, 'detail': detail, 'blocking_trainers': [], 'blocking_npcs': []}
        if obstacle is not None:
            blockage['field_obstruction'] = obstacle
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = SimpleNamespace(rules=[movement], npc_toggles={},
            satisfied=lambda goal, facts: False, coordinates=lambda rule: [(3, 2)],
            frontier=lambda goal, facts: [producer] if goal == unlock else [])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_history, agent.navigation_blockage = {'actual': blockage}, None
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.client = Mock()
        agent.client.route.return_value = {'found': False}
        agent.remembered_goal_reachable = Mock(return_value=False)
        groups = {'parent': {'target': parent, 'rules': []}}
        return agent, groups, unlock, producer

    def test_legacy_water_history_cannot_reverse_unrelated_paid_flag(self):
        for detail in ('The target region requires crossing water',
                       'Water separates a reachable frontier from the final goal'):
            with self.subTest(detail=detail):
                agent, groups, _, _ = self.fixture(detail)
                before = deepcopy(agent.navigation_history)
                agent.add_navigation_groups(groups, {**fixtures.facts(), 'map': 'Town'})
                self.assertEqual(list(groups), ['parent'])
                self.assertEqual(agent.navigation_history, before)

    def test_structured_cut_and_surf_failures_keep_the_actual_field_requirement(self):
        for move in ('Cut', 'Surf'):
            with self.subTest(move=move):
                obstacle = {'move': move, 'map': 'Remote', 'stance': [7, 8],
                            'landing': ['Remote', 9, 8], 'destination': 'FarGym'}
                agent, groups, _, _ = self.fixture('A planning path needs terrain clearance', obstacle)
                agent.field_requirements[move] = deepcopy(obstacle)
                before = deepcopy(agent.field_requirements)
                agent.add_navigation_groups(groups, {**fixtures.facts(), 'map': 'Town'})
                self.assertEqual(list(groups), ['parent'])
                self.assertEqual(agent.field_requirements, before)

    def test_actual_scene_displacement_still_backchains_its_enabled_guard(self):
        agent, groups, unlock, producer = self.fixture(
            'repeated unchanged script displacement: Room (3, 2) -> (3, 3)')
        agent.add_navigation_groups(groups, {**fixtures.facts(), 'map': 'Town'})
        self.assertEqual(groups[json.dumps(unlock)]['rules'], [producer])
        self.assertEqual(groups[json.dumps(unlock)]['context']['observed_navigation_prerequisites'][0]
                         ['requested_goal'], groups['parent']['target'])

    def test_unknown_and_malformed_evidence_does_not_claim_a_field_failure(self):
        for blockage in ({}, {'detail': 'No tile route'}, {'detail': 'requires crossing water'},
                         {'field_obstruction': []}, {'field_obstruction': {'move': 'Unknown'}}):
            with self.subTest(blockage=blockage):
                self.assertFalse(navigation_failure_is_field_prerequisite(blockage))

    def test_legitimate_collection_and_existing_paid_producer_are_not_deleted(self):
        agent, groups, unlock, producer = self.fixture('The target region requires crossing water')
        collection = {'target': ('catch', 'safari:Room', True), 'rules': [],
                      'context': {'observed_new_species': ['Paras']}}
        paid = {'target': unlock, 'rules': [producer], 'objectives': ['Enter for actual collection']}
        groups.update(collection=collection, paid=paid)
        before = deepcopy(groups)
        agent.add_navigation_groups(groups, {**fixtures.facts(), 'map': 'Town'})
        self.assertEqual(groups, before)

    def test_remembered_obstacle_keeps_current_position_and_has_no_alias(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'Town', 'player_x': 3, 'player_y': 4}
        agent.active = {'target': ('heal', 'party', True)}
        agent.navigation_memory, agent.navigation_history, agent.observed_barrier_maps = {}, {}, set()
        result = {'result': 'blocked', 'detail': 'The target region requires crossing water',
                  'field_obstruction': {'move': 'Surf', 'map': 'Remote', 'stance': [7, 8]}}
        before = deepcopy(result)
        agent.remember_travel_result('Nurse', result)
        self.assertEqual(agent.navigation_blockage['map'], 'Town')
        self.assertEqual(agent.navigation_blockage['position'], [3, 4])
        self.assertEqual(agent.navigation_blockage['field_obstruction'], before['field_obstruction'])
        result['field_obstruction']['stance'][0] = 99
        self.assertEqual(agent.navigation_memory['Nurse']['field_obstruction'], before['field_obstruction'])
        self.assertEqual(agent.navigation_history[json.dumps(['Nurse', 'Town'])]['field_obstruction'],
                         before['field_obstruction'])


if __name__ == '__main__':
    unittest.main()
