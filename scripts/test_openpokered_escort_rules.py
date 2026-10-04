"""Escort movement and terminal visibility, not cinematic transient removals."""
import json
import unittest
from types import SimpleNamespace
from unittest.mock import Mock

import test_openpokered_story as fixtures
from openpokered.story_rules import compile_story, requirements, Rule
from openpokered.autonomous_story import AutonomousStoryAgent


class EscortRulesTests(unittest.TestCase):
    def test_guarded_follow_npc_exposes_real_player_movement_prerequisite(self):
        guard = fixtures.call('getFlag', 'GATE_OPEN')
        rules = compile_story(fixtures.story([fixtures.conditional(guard, [], [
            fixtures.command('game.followNpc', 'GUIDE', 12, 18)])]))
        movement = next(rule for rule in rules if rule.effect == ('movement', 'followNpc', True))
        self.assertEqual(movement.missing(fixtures.facts(GATE_OPEN=False)), [])
        self.assertEqual(movement.guards, [(guard, False)])
        self.assertEqual(requirements(guard, True, fixtures.facts(GATE_OPEN=False)),
                         [[('flag', 'GATE_OPEN', True)]])
        self.assertEqual(movement.missing(fixtures.facts(GATE_OPEN=True)), [('flag', 'GATE_OPEN', False)])

    def test_temporary_hide_then_show_is_not_a_permanent_removal_candidate(self):
        rules = compile_story(fixtures.story([
            fixtures.command('followNpc', 'GUIDE', 12, 18),
            fixtures.command('hideObjectByName', 'GUIDE'),
            fixtures.command('showObjectByName', 'GUIDE')]))
        self.assertNotIn(('visibility', 'GUIDE', False), [rule.effect for rule in rules])
        show = next(rule for rule in rules if rule.effect == ('visibility', 'GUIDE', True))
        # The actual cinematic order is still evidence for later operations.
        self.assertIn(('visibility', 'GUIDE', False), show.preceding)
        self.assertIn(('movement', 'followNpc', True), show.preceding)

    def test_show_then_hide_keeps_only_terminal_hidden_state(self):
        rules = compile_story(fixtures.story([fixtures.command('showObject', 'GUIDE'),
                                             fixtures.command('hideObject', 'GUIDE')]))
        self.assertEqual([rule.effect for rule in rules], [('visibility', 'GUIDE', False)])

    def test_different_objects_and_guarded_arms_do_not_cancel_each_other(self):
        guard = fixtures.call('getFlag', 'GO')
        rules = compile_story(fixtures.story([
            fixtures.command('hideObjectByName', 'OTHER'),
            fixtures.conditional(guard, [fixtures.command('hideObjectByName', 'GUIDE')],
                                  [fixtures.command('showObjectByName', 'GUIDE')])]))
        self.assertEqual({rule.effect for rule in rules}, {
            ('visibility', 'OTHER', False), ('visibility', 'GUIDE', False), ('visibility', 'GUIDE', True)})
        hidden = next(rule for rule in rules if rule.effect == ('visibility', 'GUIDE', False))
        shown = next(rule for rule in rules if rule.effect == ('visibility', 'GUIDE', True))
        self.assertEqual(hidden.guards, [(guard, True)])
        self.assertEqual(shown.guards, [(guard, False)])

    def test_unreachable_reset_after_return_does_not_remove_a_real_hide(self):
        rules = compile_story(fixtures.story([fixtures.command('hideObjectByName', 'GUIDE'),
            {'Return': None}, fixtures.command('showObjectByName', 'GUIDE')]))
        self.assertEqual([rule.effect for rule in rules], [('visibility', 'GUIDE', False)])

    def test_final_reset_on_one_choice_does_not_cancel_another_choice(self):
        choice = {'Choice': {'options': [
            {'label': fixtures.literal('YES'), 'body': [fixtures.command('hideObjectByName', 'GUIDE')]},
            {'label': fixtures.literal('NO'), 'body': [fixtures.command('hideObjectByName', 'GUIDE'),
                                                    fixtures.command('showObjectByName', 'GUIDE')]},
        ]}}
        rules = compile_story(fixtures.story([choice]))
        self.assertEqual([(rule.effect, rule.choices) for rule in rules], [
            (('visibility', 'GUIDE', False), ['YES']), (('visibility', 'GUIDE', True), ['NO'])])

    def test_invalid_or_unknown_escort_arguments_do_not_certify_movement(self):
        for args in (('GUIDE',), ('GUIDE', 12), ('GUIDE', 'unknown', 18), (None, 12, 18)):
            with self.subTest(args=args):
                self.assertEqual(compile_story(fixtures.story([fixtures.command('followNpc', *args)])), [])

    def test_existing_strategy_candidate_keeps_the_observed_navigation_reason(self):
        guard = fixtures.call('getFlag', 'GATE_OPEN')
        escort = Rule('escort', 'Room', 'Room:escort', ['coord:exit'], [(guard, False)], [],
                      ('movement', 'followNpc', True), [])
        unlock = ('flag', 'GATE_OPEN', True)
        producer = Rule('win', 'Gym', 'Gym:challenge', ['npc:1'], [], [], unlock,
                        [('battle', 'actual_opponent', True)])
        requested = ('flag', 'LATER_WIN', True)
        blockage = {'map': 'Room', 'destination': 'FarGym', 'goal': requested,
                    'position': [4, 10], 'detail': 'Actual repeated unchanged displacement'}
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = SimpleNamespace(rules=[escort], npc_toggles={},
            satisfied=lambda target, facts: False, coordinates=lambda rule: [(4, 10)],
            frontier=lambda target, facts: [producer] if target == unlock else [])
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.navigation_history = {'actual': blockage}
        agent.game = SimpleNamespace(stationary_npcs={})
        agent.client = Mock()
        agent.client.route.return_value = {'found': False}
        agent.remembered_goal_reachable = Mock(return_value=False)
        key = json.dumps(unlock)
        context = {'opponent_parties': [{'species': 'Onix', 'level': 14}]}
        groups = {key: {'target': unlock, 'rules': [producer], 'context': context},
                  'parent': {'target': requested, 'rules': []}}
        for _ in range(2):
            agent.add_navigation_groups(groups, {'map': 'Town', 'flags': {'GATE_OPEN': False}})
        self.assertEqual(context['opponent_parties'], [{'species': 'Onix', 'level': 14}])
        references = context['observed_navigation_prerequisites']
        self.assertEqual(len(references), 1)
        self.assertEqual(references[0]['requested_goal'], requested)
        self.assertEqual(references[0]['destination'], 'FarGym')
        self.assertEqual(references[0]['enabling_guards'], [{'expression': guard,
            'required_value': False, 'observed_value': False}])
        self.assertIn('not a whole-route', references[0]['scope'])
        self.assertEqual(agent.navigation_history, {'actual': blockage})


if __name__ == '__main__':
    unittest.main()
