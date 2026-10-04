"""A real goal observed during travel must retire only that stale walk."""
from copy import deepcopy
import unittest
from unittest.mock import Mock

from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.playthrough_judgments import JevGame, NavigationGoalObserved, NavigationPause
from openpokered.story_rules import Rule, StoryIndex


class NavigationGoalHandoffTests(unittest.TestCase):
    def game(self):
        game = JevGame.__new__(JevGame)
        target = ('flag', 'EVENT_IN_SAFARI_ZONE', True)
        game.navigation_active = True
        game.navigation_goal_target = target
        agent = game.judgments = Mock()
        agent.active = {'target': target}
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.facts.return_value = {'flags': {'EVENT_IN_SAFARI_ZONE': True}}
        game.st = Mock(return_value={
            'screen': 'overworld', 'map_name': 'SafariZoneCenter',
            'player_x': 14, 'player_y': 25, 'frame_count': 3142,
            'warp_fade': 'Idle', 'player_movement_state': 'Idle',
            'script_running': False, 'script_awaiting_battle': False,
            'door_exit_pending': False, 'fishing_active': False,
            'active_script_effect': None, 'dialogue': None, 'choice': None,
            'field_menu': None})
        return game

    def test_fresh_observed_goal_stops_before_returning_to_temporary_gate_destination(self):
        game = self.game()
        before = deepcopy(game.st.return_value)
        with self.assertRaises(NavigationGoalObserved) as raised:
            game.cutscene()
        result = raised.exception.result('SafariZoneGate')
        self.assertEqual(result['result'], 'paused_after_goal')
        self.assertEqual(result['destination'], 'SafariZoneGate')
        self.assertEqual(result['position'], ['SafariZoneCenter', 14, 25])
        self.assertEqual(result['frame'], 3142)
        self.assertEqual(result['observed_target'], ['flag', 'EVENT_IN_SAFARI_ZONE', True])
        self.assertNotIsInstance(raised.exception, NavigationPause)
        game.judgments.settle.assert_called_once_with(game.judgments.active['target'])
        game.judgments.facts.assert_called_once_with()
        self.assertEqual(game.st.return_value, before)
        game.judgments.choose.assert_not_called()

    def test_unsatisfied_goal_does_not_retire_navigation(self):
        game = self.game()
        game.judgments.facts.return_value['flags']['EVENT_IN_SAFARI_ZONE'] = False
        self.assertTrue(game.cutscene())
        game.judgments.record.assert_not_called()

    def test_navigation_binding_does_not_leak_to_other_skills_or_changed_subgoals(self):
        for change in ('no_navigation', 'no_binding', 'no_active', 'changed_target', 'malformed_binding'):
            with self.subTest(change=change):
                game = self.game()
                if change == 'no_navigation':
                    game.navigation_active = False
                elif change == 'no_binding':
                    game.navigation_goal_target = None
                elif change == 'no_active':
                    game.judgments.active = None
                elif change == 'changed_target':
                    game.judgments.active['target'] = ('item', 'HM03', True)
                else:
                    game.navigation_goal_target = ['flag', 'EVENT_IN_SAFARI_ZONE']
                self.assertTrue(game.cutscene())
                game.judgments.facts.assert_not_called()
                game.judgments.record.assert_not_called()

    def test_busy_or_unknown_control_never_certifies_goal_handoff(self):
        state = self.game().st.return_value
        changes = {
            'screen': 'battle', 'warp_fade': 'FadeOut', 'player_movement_state': 'Walking',
            'script_running': True, 'script_awaiting_battle': True,
            'door_exit_pending': True, 'fishing_active': True,
            'active_script_effect': 'Warp', 'dialogue': 'unfinished',
            'choice': {'options': ['YES', 'NO']}, 'field_menu': {'kind': 'pc'},
            'frame_count': True, 'player_x': None, 'player_y': '25', 'map_name': None}
        for key, value in changes.items():
            for missing in (False, True):
                with self.subTest(key=key, missing=missing):
                    game = self.game()
                    game.st.return_value = deepcopy(state)
                    if missing:
                        game.st.return_value.pop(key)
                    else:
                        game.st.return_value[key] = value
                    self.assertTrue(game.cutscene())
                    game.judgments.facts.assert_not_called()

    def test_registration_handoff_uses_existing_pending_source_audit_gate(self):
        game = self.game()
        target = ('register', 'Snorlax', True)
        game.navigation_goal_target = target
        game.judgments.active['target'] = target
        game.judgments.facts.return_value = {
            'dex': {'owned_species': ['Snorlax']}, 'collection_audit_pending': ['Snorlax']}
        self.assertTrue(game.cutscene())
        game.judgments.facts.return_value['collection_audit_pending'] = []
        with self.assertRaises(NavigationGoalObserved):
            game.cutscene()

    def agent(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ['flag', 'EVENT_IN_SAFARI_ZONE', True]}
        agent.game = self.game()
        agent.game.navigation_goal_target = ['previous', ['nested'], False]
        return agent

    def test_travel_binds_only_current_goal_and_reports_actual_not_claimed_arrival(self):
        agent = self.agent()
        previous = agent.game.navigation_goal_target
        target = deepcopy(agent.active['target'])
        rule = Rule('entry', 'SafariZoneGate', 'gate', [], [], [], target, [])
        def walk(*args, **options):
            self.assertEqual(agent.game.navigation_goal_target, target)
            self.assertIsNot(agent.game.navigation_goal_target, agent.active['target'])
            agent.game.cutscene()
            self.fail('Old direction stream resumed after native goal satisfaction')
        agent._travel = Mock(side_effect=walk)
        result = agent.travel('SafariZoneGate', rule, [(3, 2)], avoid_encounters=True)
        self.assertEqual(result['result'], 'paused_after_goal')
        self.assertEqual(result['position'], ['SafariZoneCenter', 14, 25])
        self.assertIs(agent.game.navigation_goal_target, previous)
        self.assertEqual(agent.active['target'], target)
        agent._travel.assert_called_once_with('SafariZoneGate', rule, [(3, 2)], True)

    def test_travel_restores_binding_after_success_and_unrelated_failure(self):
        for outcome in ({'result': 'reached', 'position': [3, 2]}, NavigationPause('battle')):
            with self.subTest(outcome=outcome):
                agent = self.agent()
                previous = agent.game.navigation_goal_target
                rule = Rule('entry', 'SafariZoneGate', 'gate', [], [], [], agent.active['target'], [])
                if isinstance(outcome, Exception):
                    agent._travel = Mock(side_effect=outcome)
                    with self.assertRaises(NavigationPause):
                        agent.travel('SafariZoneGate', rule)
                else:
                    agent._travel = Mock(return_value=outcome)
                    self.assertIs(agent.travel('SafariZoneGate', rule), outcome)
                self.assertIs(agent.game.navigation_goal_target, previous)

    def test_observation_result_does_not_alias_or_mutate_nested_target(self):
        target = ('location', ['SomeMap', 4, 5], True)
        state = self.game().st.return_value
        pause = NavigationGoalObserved(target, state)
        result = pause.result('DifferentMap')
        result['observed_target'][1][0] = 'ChangedByConsumer'
        self.assertEqual(target, ('location', ['SomeMap', 4, 5], True))
        self.assertEqual(pause.result('DifferentMap')['observed_target'], list(target))


if __name__ == '__main__':
    unittest.main()
