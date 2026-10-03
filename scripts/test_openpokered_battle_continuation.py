"""Required battle turns remain Jev choices, not optional strategic abstentions."""
import copy
import unittest
from unittest.mock import Mock, patch

from openpokered.playthrough_judgments import JevGame
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.typesafe import ChoiceAnswer, SystemOneResult


class BattleContinuationTests(unittest.TestCase):
    def fixture(self):
        party = [
            {'species': 'Exeggcute', 'level': 38, 'hp': 75, 'max_hp': 103,
             'moves': ['Barrage', 'Hypnosis', 'Reflect', 'LeechSeed'],
             'pp': [19, 20, 20, 10], 'status': 'None'},
            {'species': 'Muk', 'level': 38, 'hp': 138, 'max_hp': 138,
             'moves': ['Pound', 'Sludge', 'PoisonGas', 'Minimize'],
             'pp': [35, 20, 40, 20], 'status': 'None'},
            {'species': 'Gloom', 'level': 46, 'hp': 121, 'max_hp': 129,
             'moves': ['MegaDrain', 'Poisonpowder', 'StunSpore', 'SleepPowder'],
             'pp': [0, 35, 30, 15], 'status': 'None'},
            {'species': 'Charizard', 'level': 73, 'hp': 213, 'max_hp': 262,
             'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
             'pp': [20, 30, 15, 0], 'status': 'Poison'},
            {'species': 'Snorlax', 'level': 30, 'hp': 139, 'max_hp': 139,
             'moves': ['Headbutt', 'Strength', 'Rest', 'Surf'],
             'pp': [15, 15, 10, 15], 'status': 'None'},
            {'species': 'Abra', 'level': 10, 'hp': 25, 'max_hp': 25,
             'moves': ['Teleport', 'None', 'None', 'None'],
             'pp': [20, 0, 0, 0], 'status': 'None'},
        ]
        state = {'screen': 'battle', 'battle_phase': 'PlayerMenu',
                 'party': party, 'battle_inventory': [],
                 'battle_live': {'player': party[2], 'player_party': party,
                    'is_wild': True, 'is_safari': False,
                    'enemy': {'species': 'Koffing', 'hp': 39, 'max_hp': 65,
                              'level': 30, 'status': 'None'}},
                 'pokedex': {'owned_species': ['Koffing']}}
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.collects_dex = True
        game.judgments.active = {'target': ('level', 'Exeggcute', 38),
            'context': {'capture_support_training': True, 'trigger': 'level',
                        'from_species': 'Exeggcute'}}
        game.judgments.choose.return_value = 'switch:1'
        return game, state

    def test_actual_exhausted_active_turn_requires_a_model_selected_legal_switch(self):
        game, state = self.fixture()
        before = copy.deepcopy(state)
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        decision = game.judgments.choose.call_args
        self.assertEqual(set(decision.args[2]), {'switch:0', 'switch:1', 'switch:3', 'switch:4'})
        self.assertEqual(decision.kwargs, {'allow_abstain': False})
        required = decision.args[1]['required_battle_continuation']
        self.assertEqual(required['active_party_index'], 2)
        self.assertEqual(required['legal_switch_indices'], [0, 1, 3, 4])
        self.assertEqual(decision.args[1]['current_subgoal']['target'], ('level', 'Exeggcute', 38))
        self.assertIn('does not guarantee', required['scope'])
        self.assertIn('already registered', decision.args[3])
        self.assertEqual(state, before)

    def test_no_hardwired_preferred_finisher(self):
        for index in (0, 1, 3, 4):
            with self.subTest(index=index):
                game, state = self.fixture()
                game.judgments.choose.return_value = f'switch:{index}'
                self.assertEqual(game.battle_recovery_plan(state), ('switch', index))
                game.judgments.choose.assert_called_once()

    def test_capture_abstention_is_not_disabled(self):
        game, state = self.fixture()
        with patch('openpokered.playthrough_judgments.capture_intent', return_value=True):
            game.battle_recovery_plan(state)
        decision = game.judgments.choose.call_args
        self.assertNotIn('allow_abstain', decision.kwargs)
        self.assertNotIn('required_battle_continuation', decision.args[1])

    def test_unobserved_phase_does_not_certify_a_required_switch(self):
        game, state = self.fixture()
        state.pop('battle_phase')
        game.battle_recovery_plan(state)
        self.assertNotIn('allow_abstain', game.judgments.choose.call_args.kwargs)

    def test_fainted_or_ineffective_members_are_not_mandatory_options(self):
        game, state = self.fixture()
        state['party'][0]['hp'] = 0
        state['party'][3]['pp'] = [0, 0, 0, 0]
        game.battle_recovery_plan(state)
        decision = game.judgments.choose.call_args
        self.assertEqual(set(decision.args[2]), {'switch:1', 'switch:4'})
        self.assertEqual(decision.args[1]['required_battle_continuation']['legal_switch_indices'], [1, 4])

    def real_judge(self, answer):
        game, state = self.fixture()
        client, model = Mock(), Mock()
        client.state.return_value = {'frame_count': 90186}
        model.system_one.return_value = SystemOneResult('fixture-jev', {'action': answer}, 1, 1)
        actor = DualStoryAgent(client, model, [{'id': 'collect-dex', 'agent_verified': True}],
                               frame_budget=100000)
        actor.active = game.judgments.active
        game.judgments = actor
        return game, state, model

    def test_real_choice_schema_excludes_none_without_overriding_the_model(self):
        game, state, model = self.real_judge(ChoiceAnswer('switch:3', {'switch:3': 1.0}, 1.0))
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 3))
        question = model.system_one.call_args.args[1]['action'].to_json()
        self.assertNotIn('none', question['criteria'])
        self.assertEqual(set(question['criteria']), {'switch:0', 'switch:1', 'switch:3', 'switch:4'})
        model.system_one.assert_called_once()

    def test_invalid_none_answer_is_not_silently_converted_to_a_switch(self):
        game, state, model = self.real_judge(ChoiceAnswer('none', {'none': .55, 'switch:1': .24}, .44))
        with self.assertRaisesRegex(StoryStopped, 'action:no_selection'):
            game.battle_recovery_plan(state)
        model.system_one.assert_called_once()


if __name__ == '__main__':
    unittest.main()
