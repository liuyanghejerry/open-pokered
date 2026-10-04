"""Training intent survives tactical selection/cache; self-KO is a cost, not a ban."""
import copy
import json
import unittest
from unittest.mock import Mock, patch

from openpokered import playthrough_judgments as judgments
from openpokered.playthrough_judgments import (JevGame, level_training_goal, move_question,
    training_move_question, training_threat_reference)
from openpokered.story_agent import StoryStopped


class TrainingAttackTests(unittest.TestCase):
    def active(self):
        return {'target': ['held_species', 'Graveler', True], 'context': {
            'acquisition_method': 'evolution', 'trigger': 'level',
            'from_species': 'Geodude', 'level': 25}}

    def state(self):
        trainee = {'species': 'Geodude', 'level': 21, 'hp': 55, 'max_hp': 55,
            'status': 'None', 'moves': ['Tackle', 'Selfdestruct', 'RockThrow', 'DefenseCurl'],
            'pp': [35, 5, 15, 40]}
        finisher = {'species': 'Charizard', 'level': 66, 'hp': 227, 'max_hp': 227,
            'status': 'None', 'moves': ['Slash', 'Flamethrower'], 'pp': [20, 15]}
        return {'screen': 'battle', 'battle_phase': 'MoveSelect', 'party': [trainee, finisher],
            'battle_inventory': [], 'battle_live': {'player': trainee,
                'enemy': {'species': 'Pidgey', 'level': 20, 'hp': 50, 'max_hp': 50, 'status': 'None'},
                'player_party': [trainee, finisher], 'is_wild': True},
            'battle_moves': {'cursor': 0, 'moves': [
                {'move': name, 'pp': pp, 'disabled': False}
                for name, pp in zip(trainee['moves'], trainee['pp'])]}}

    def game(self, state, active):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock(collects_dex=False, active=active)
        game.judgments.choose.return_value = '0'
        game.st = Mock(return_value=state)
        game.tap, game.step = Mock(), Mock()
        game.move_cache, game.move_cache_hits, game.active_milestone = {}, 0, None
        return game

    def test_self_knockout_effect_is_reported_without_removing_moves(self):
        state = self.state()
        menu = {'moves': [{'move': name, 'pp': 1, 'disabled': False}
            for name in ('Tackle', 'Selfdestruct', 'Explosion')]}
        compact, choices = move_question(state, menu)
        self.assertEqual(choices, {'0': 'Tackle', '1': 'Selfdestruct', '2': 'Explosion'})
        self.assertFalse(compact['moves']['0']['self_knockout_effect'])
        self.assertTrue(compact['moves']['1']['self_knockout_effect'])
        self.assertTrue(compact['moves']['2']['self_knockout_effect'])
        menu['moves'][1]['disabled'] = True
        menu['moves'][2]['pp'] = 0
        self.assertEqual(move_question(state, menu)[1], {'0': 'Tackle'})

    def test_training_intent_reports_observations_without_mutating_source(self):
        state, active = self.state(), self.active()
        before = copy.deepcopy((state, active))
        goal = level_training_goal(state, active)
        self.assertEqual(goal['selected_target'], ['held_species', 'Graveler', True])
        self.assertEqual(goal['target_level'], 25)
        self.assertTrue(goal['active_species_matches_trainee'])
        self.assertEqual(goal['observed_trainee_party_members'][0]['hp'], 55)
        self.assertIn('not individual identity', goal['scope'])
        self.assertEqual((state, active), before)
        goal['selected_target'][1] = 'changed'
        self.assertEqual(active['target'][1], 'Graveler')

    def test_capture_support_and_story_level_training_share_goal_reference(self):
        for active in ({'target': ('level', 'Geodude', 40), 'context': {
                'capture_support_training': True, 'trigger': 'level', 'from_species': 'Geodude', 'level': 40}},
                {'target': ('level', 'leader', 25), 'context': {
                    'training_battler': {'species': 'Geodude'}, 'target_level': 25}}):
            with self.subTest(active=active):
                self.assertEqual(level_training_goal(self.state(), active)['trainee_species'], 'Geodude')

    def test_unrelated_and_incomplete_goals_do_not_invent_training(self):
        for active in (None, Mock(), {}, {'target': ['held_species', 'Graveler', True]},
                {'target': ['item', 'MOON_STONE', True], 'context': {}},
                {'target': ['held_species', 'Golem', True], 'context': {
                    'acquisition_method': 'evolution', 'trigger': 'trade', 'from_species': 'Graveler'}},
                {'target': ['level', 'leader', 25], 'context': {'training_battler': None}}):
            with self.subTest(active=active):
                self.assertIsNone(level_training_goal(self.state(), active))

    def test_move_selector_receives_training_goal_and_preserves_selfdestruct_option(self):
        state = self.state()
        game = self.game(state, self.active())
        game._select_move()
        _layer, compact, choices, instruction = game.judgments.choose.call_args.args
        self.assertEqual(compact['level_training_goal']['trainee_species'], 'Geodude')
        self.assertEqual({key: value for key, value in choices.items() if key != 'back'},
                         {'0': 'Tackle', '1': 'Selfdestruct', '2': 'RockThrow'})
        self.assertIn('back', choices)
        self.assertIn('forfeits its experience', instruction)
        self.assertIn('No attack choice guarantees', instruction)
        game.tap.assert_called_once_with('a', 4)

    def test_training_goal_and_target_invalidate_nontraining_attack_cache(self):
        game = self.game(self.state(), {'target': ['flag', 'EVENT_BEAT_BROCK', True], 'context': {}})
        game._select_move()
        self.assertNotIn('level_training_goal', game.judgments.choose.call_args.args[1])
        game.judgments.active = self.active()
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 2)
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 2)
        self.assertEqual(game.move_cache_hits, 1)
        game.judgments.active['target'] = ['level', 'Geodude', 30]
        game.judgments.active['context']['level'] = 30
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 3)

    def test_finisher_does_not_invent_active_trainee_or_participation(self):
        state = self.state()
        state['battle_live']['player'] = state['party'][1]
        goal = level_training_goal(state, self.active())
        self.assertFalse(goal['active_species_matches_trainee'])
        self.assertIn('participation proof', goal['scope'])
        state['battle_live']['player_party'][0]['hp'] = 0
        self.assertNotEqual(goal, level_training_goal(state, self.active()))
        del state['battle_live']['player_party']
        self.assertIsNone(level_training_goal(state, self.active())['observed_trainee_party_members'])

    def test_main_menu_compares_self_ko_cost_with_offered_finisher(self):
        state = self.state()
        state['battle_phase'] = 'PlayerMenu'
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'switch:1'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        _layer, compact, choices, instruction = game.judgments.choose.call_args.args
        self.assertEqual(compact['level_training_goal']['trainee_species'], 'Geodude')
        fight = json.loads(choices['fight'])
        self.assertEqual(fight['self_knockout_moves_with_pp'], ['Selfdestruct'])
        self.assertIn('switch:1', choices)
        self.assertIn('Maximum damage is not training progress', instruction)

    def sleeping_finisher_state(self):
        state = self.state()
        state['battle_phase'] = 'PlayerMenu'
        state['battle_live']['player'] = state['party'][1]
        state['battle_live']['player']['status'] = 'Sleep(4)'
        snorlax = {'species': 'Snorlax', 'level': 31, 'hp': 142, 'max_hp': 142,
            'status': 'None', 'moves': ['Headbutt', 'Surf'], 'pp': [15, 15]}
        state['party'].append(snorlax)
        state['battle_live']['player_party'].append(snorlax)
        state['battle_inventory'] = [{'item': 'PokeBall', 'qty': 2}]
        return state

    def test_sleeping_finisher_can_relay_to_lower_level_awake_nontrainee(self):
        state = self.sleeping_finisher_state()
        original = copy.deepcopy(state)
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'switch:2'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 2))
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        self.assertIn('switch:2', choices)
        self.assertIn('fight', choices)
        self.assertIn('ball:PokeBall', choices)
        self.assertNotIn('switch:0', choices)  # Do not re-expose the switched-out trainee.
        self.assertNotIn('switch:1', choices)  # Current battler is not a switch target.
        relay = compact['training_finisher_continuation']
        self.assertEqual(relay['active_status'], 'Sleep(4)')
        self.assertEqual(relay['offered_finisher_switch_indices'], [2])
        self.assertIn('participation', relay['scope'])
        self.assertIn('wake-up tick', instructions)
        self.assertNotIn('allow_abstain', game.judgments.choose.call_args.kwargs)
        self.assertEqual(state, original)

    def test_training_relay_does_not_infer_lower_level_teammate_is_useless(self):
        state = self.sleeping_finisher_state()
        state['party'][2]['level'] = 15  # Even below the trainee; level alone is not legal availability.
        state['party'][1]['status'] = 'None'
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'switch:2'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 2))
        self.assertEqual(json.loads(game.judgments.choose.call_args.args[2]['switch:2'])[
            'switch_to']['level'], 15)

    def test_sleeping_training_move_remains_legal_with_turn_gate_explained(self):
        state = self.sleeping_finisher_state()
        state['battle_phase'] = 'MoveSelect'
        state['battle_moves'] = {'cursor': 0, 'moves': [
            {'move': 'Slash', 'pp': 20, 'disabled': False}]}
        game = self.game(state, self.active())
        game._select_move()
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        self.assertEqual(compact['training_battle_state']['active']['status'], 'Sleep(4)')
        self.assertEqual(choices['0'], 'Slash')
        self.assertIn('back', choices)
        self.assertIn('wake-up tick still forfeits the attack', instructions)
        self.assertIn('does not advance sleep', instructions)
        game.tap.assert_called_once_with('a', 4)

    def test_relay_excludes_fainted_or_pp_exhausted_alternatives(self):
        for changes in ({'hp': 0}, {'pp': [0, 0]}):
            with self.subTest(changes=changes):
                state = self.sleeping_finisher_state()
                state['party'][2].update(changes)
                game = self.game(state, self.active())
                game.judgments.choose.return_value = 'fight'
                self.assertIsNone(game.battle_recovery_plan(state))
                self.assertNotIn('switch:2', game.judgments.choose.call_args.args[2])

    def test_capture_support_training_can_relay_after_finisher_status_changes(self):
        active = {'target': ['level', 'Geodude', 40], 'context': {
            'capture_support_training': True, 'trigger': 'level', 'from_species': 'Geodude', 'level': 40}}
        state = self.sleeping_finisher_state()
        state['party'][1]['status'] = 'Paralysis'
        game = self.game(state, active)
        game.judgments.choose.return_value = 'switch:2'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 2))
        self.assertEqual(game.judgments.choose.call_args.args[1]['training_finisher_continuation'][
            'active_status'], 'Paralysis')

    def test_nontraining_sleep_does_not_receive_training_relay_semantics(self):
        state = self.sleeping_finisher_state()
        game = self.game(state, {'target': ['flag', 'EVENT_BEAT_BROCK', True], 'context': {}})
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        _, compact, choices, _ = game.judgments.choose.call_args.args
        self.assertNotIn('switch:2', choices)
        self.assertNotIn('training_finisher_continuation', compact)

    def test_capture_precedence_keeps_original_switch_choices_and_no_training_relay(self):
        state = self.sleeping_finisher_state()
        state['pokedex'] = {'owned_species': ['Geodude']}
        game = self.game(state, self.active())
        game.judgments.collects_dex = True
        game.judgments.choose.return_value = 'switch:0'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 0))
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        self.assertIn('switch:0', choices)  # Existing weak capture attacker, not training-only exclusion.
        self.assertIn('switch:2', choices)
        self.assertNotIn('training_finisher_continuation', compact)
        self.assertIn('defeating it spends the encounter', instructions)

    def test_selfdestruct_is_still_driven_when_the_judgment_selects_it(self):
        state = self.state()
        state['battle_moves']['cursor'] = 1
        game = self.game(state, self.active())
        game.judgments.choose.return_value = '1'
        game._select_move()
        game.tap.assert_called_once_with('a', 4)
        attack = next(call for call in game.judgments.record.call_args_list if call.args[0] == 'attack')
        self.assertEqual(attack.kwargs['move'], 'Selfdestruct')

    def test_finisher_receives_exact_hp_status_and_native_damage_without_crediting_participation(self):
        state = self.state()
        state['battle_live']['player'] = state['party'][1]
        state['battle_live']['player']['status'] = 'Poison'
        state['battle_moves']['moves'] = [{'move': 'Slash', 'pp': 20, 'disabled': False,
            'direct_hit_preview': {'normal_damage': [84, 99], 'critical_damage': [154, 181],
                                   'target_hp': 50, 'direct_hit_can_ko': True, 'critical_threshold': 15}}]
        original = copy.deepcopy(state)
        compact, choices = training_move_question(state, state['battle_moves'], self.active())
        self.assertEqual(choices, {'0': 'Slash'})
        self.assertEqual(compact['training_battle_state']['active_role'], 'other_species')
        self.assertEqual(compact['training_battle_state']['active']['status'], 'Poison')
        self.assertEqual(compact['training_battle_state']['enemy']['hp'], 50)
        self.assertEqual(compact['moves']['0']['direct_hit_preview']['normal_damage'], [84, 99])
        self.assertIn('NOT zero damage', compact['direct_hit_preview_scope'])
        self.assertNotIn('participated', compact['level_training_goal'])
        compact['moves']['0']['direct_hit_preview']['normal_damage'][0] = 0
        compact['training_battle_state']['active']['hp'] = 0
        self.assertEqual(state, original)

    def test_missing_preview_remains_unknown_and_nontraining_question_unchanged(self):
        state = self.state()
        compact, _ = training_move_question(state, state['battle_moves'], self.active())
        self.assertIsNone(compact['moves']['0']['direct_hit_preview'])
        self.assertEqual(training_move_question(state, state['battle_moves'], None),
                         move_question(state, state['battle_moves']))

    def test_training_abstention_cancels_menu_without_executing_an_attack(self):
        state = self.state()
        game = self.game(state, self.active())
        game.judgments.choose.side_effect = StoryStopped('action:no_selection')
        game._select_move()
        game.tap.assert_called_once_with('b', 4)
        game.step.assert_called_once_with(10)
        self.assertEqual(game.move_cache, {})
        kinds = [call.args[0] for call in game.judgments.record.call_args_list]
        self.assertIn('training_move_abstention', kinds)
        self.assertIn('training_menu_cancelled', kinds)
        self.assertNotIn('attack', kinds)

    def test_sleep_turn_reference_distinguishes_committing_from_menu_cancellation(self):
        state = self.sleeping_finisher_state()
        state['battle_live']['player']['status'] = 'Sleep(6)'
        original = copy.deepcopy(state)
        reference = judgments.training_turn_commitment_reference(state)
        gate = reference['commit_usable_move']['sleep_gate']
        self.assertEqual(gate['observed_counter'], 6)
        self.assertEqual(gate['counter_after_gate_if_reached'], 5)
        self.assertFalse(gate['move_executes_at_sleep_gate'])
        self.assertTrue(reference['commit_usable_move']['advances_battle_turn'])
        self.assertEqual(reference['open_or_cancel_menu']['sleep_counter_change'], 0)
        self.assertFalse(reference['open_or_cancel_menu']['advances_battle_turn'])
        self.assertIn('enemy can inflict sleep again', reference['scope'])
        self.assertEqual(state, original)

    def test_sleep_wake_tick_is_not_an_attack_but_zero_counter_is_defensively_awake(self):
        state = self.sleeping_finisher_state()
        for counter, after, executes in ((1, 0, False), (0, 0, True)):
            with self.subTest(counter=counter):
                state['battle_live']['player']['status'] = f'Sleep({counter})'
                gate = judgments.training_turn_commitment_reference(state)['commit_usable_move']['sleep_gate']
                self.assertEqual(gate['counter_after_gate_if_reached'], after)
                self.assertEqual(gate['move_executes_at_sleep_gate'], executes)

    def test_non_sleep_status_does_not_manufacture_a_sleep_counter(self):
        state = self.sleeping_finisher_state()
        for status in ('None', 'Paralysis', 'Freeze', 'Sleep(?)'):
            with self.subTest(status=status):
                state['battle_live']['player']['status'] = status
                reference = judgments.training_turn_commitment_reference(state)
                self.assertIsNone(reference['commit_usable_move']['sleep_gate'])
                self.assertEqual(reference['observed_active_status'], status)

    def test_finisher_fight_describes_turn_commitment_not_an_immediate_sleeping_attack(self):
        state = self.sleeping_finisher_state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'fight'
        self.assertIsNone(game.battle_recovery_plan(state))
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        fight = json.loads(choices['fight'])
        self.assertEqual(fight['active_pokemon']['status'], 'Sleep(4)')
        self.assertEqual(fight['training_turn_commitment_reference'], compact['training_turn_commitment_reference'])
        self.assertFalse(fight['opening_menu_spends_turn'])
        self.assertIn('switch:2', choices)
        self.assertIn('ball:PokeBall', choices)
        self.assertIn('counter_after_gate_if_reached', instructions)
        self.assertNotIn('allow_abstain', game.judgments.choose.call_args.kwargs)

    def test_attack_judgment_receives_actual_same_state_main_menu_comparison(self):
        state = self.sleeping_finisher_state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        main_options = copy.deepcopy(game.judgments.choose.call_args.args[2])
        state['battle_phase'] = 'MoveSelect'
        state['battle_moves'] = {'cursor': 0, 'moves': [
            {'move': 'Slash', 'pp': 20, 'disabled': False}]}
        game.judgments.choose.return_value = '0'
        game._select_move()
        _, compact, choices, _ = game.judgments.choose.call_args.args
        comparison = compact['training_main_menu_comparison']
        self.assertEqual(comparison['selected_operation'], 'fight')
        self.assertEqual(comparison['offered_operations'], main_options)
        self.assertIn('not an instruction to override', comparison['scope'])
        self.assertEqual(choices['0'], 'Slash')
        self.assertIn('back', choices)
        game.tap.assert_called_once_with('a', 4)

    def test_changed_battle_facts_invalidate_saved_main_menu_comparison(self):
        state = self.sleeping_finisher_state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        state['battle_live']['enemy']['hp'] -= 1
        state['battle_phase'] = 'MoveSelect'
        state['battle_moves'] = {'cursor': 0, 'moves': [
            {'move': 'Slash', 'pp': 20, 'disabled': False}]}
        game.judgments.choose.return_value = '0'
        game._select_move()
        self.assertNotIn('training_main_menu_comparison', game.judgments.choose.call_args.args[1])

    def test_repeated_same_state_menu_cancellations_are_counted_without_banning_fight(self):
        state = self.state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'back'
        game._select_move()
        game._select_move()
        state['battle_phase'] = 'PlayerMenu'
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        _, compact, choices, _ = game.judgments.choose.call_args.args
        feedback = compact['training_move_menu_feedback']
        self.assertEqual(feedback['same_state_menu_cancellations'], 2)
        self.assertEqual(feedback['battle_turns_spent_by_cancellation'], 0)
        self.assertIn('fight', choices)
        self.assertIn('switch:1', choices)
        self.assertEqual(game.move_cache, {})
        self.assertEqual([call.args[0] for call in game.tap.call_args_list], ['b', 'b'])

    def test_changed_state_restarts_cancellation_count_and_does_not_credit_a_turn(self):
        state = self.state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'back'
        game._select_move()
        state['battle_live']['enemy']['hp'] -= 1
        game._select_move()
        state['battle_phase'] = 'PlayerMenu'
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        feedback = game.judgments.choose.call_args.args[1]['training_move_menu_feedback']
        self.assertEqual(feedback['same_state_menu_cancellations'], 1)

    def test_capture_precedence_does_not_add_training_turn_commitment_or_saved_comparison(self):
        state = self.sleeping_finisher_state()
        state['pokedex'] = {'owned_species': ['Geodude']}
        game = self.game(state, self.active())
        game.judgments.collects_dex = True
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        self.assertNotIn('training_turn_commitment_reference', game.judgments.choose.call_args.args[1])
        self.assertFalse(hasattr(game, '_training_main_menu_comparison'))

    def test_native_preview_changes_invalidate_menu_feedback_even_if_hp_and_status_match(self):
        state = self.state()
        state['battle_live']['player_move_previews'] = [{'slot': 0, 'direct_hit_preview': None}]
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'back'
        game._select_move()
        state['battle_live']['player_move_previews'][0]['direct_hit_preview'] = {'normal_damage': [1, 2]}
        state['battle_phase'] = 'PlayerMenu'
        game.judgments.choose.return_value = 'fight'
        game.battle_recovery_plan(state)
        self.assertNotIn('training_move_menu_feedback', game.judgments.choose.call_args.args[1])

    def test_explicit_back_is_legal_and_next_question_receives_feedback(self):
        state = self.state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'back'
        game._select_move()
        self.assertEqual(game.move_cache, {})
        game.judgments.choose.return_value = '0'
        game._select_move()
        self.assertIn('prior_menu_abstention', game.judgments.choose.call_args.args[1])
        self.assertEqual([call.args[0] for call in game.tap.call_args_list], ['b', 'a'])

    def test_cancel_feedback_reaches_main_menu_without_removing_fight_or_switches(self):
        state = self.state()
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'back'
        game._select_move()
        state['battle_phase'] = 'PlayerMenu'
        game.judgments.choose.return_value = 'switch:1'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        _, compact, choices, _ = game.judgments.choose.call_args.args
        self.assertIn('training_move_menu_feedback', compact)
        self.assertIn('fight', choices)
        self.assertIn('switch:1', choices)
        state['battle_live']['enemy']['hp'] -= 1
        game.battle_recovery_plan(state)
        self.assertNotIn('training_move_menu_feedback', game.judgments.choose.call_args.args[1])

    def test_training_service_errors_and_nontraining_abstention_remain_fail_closed(self):
        for active, reason in ((self.active(), 'action:service_unavailable'),
                               (None, 'action:no_selection')):
            with self.subTest(reason=reason):
                game = self.game(self.state(), active)
                game.judgments.choose.side_effect = StoryStopped(reason)
                with self.assertRaisesRegex(StoryStopped, reason):
                    game._select_move()
                game.tap.assert_not_called()

    def test_exact_training_hp_and_preview_changes_invalidate_attack_cache(self):
        state = self.state()
        game = self.game(state, self.active())
        game._select_move()
        state['battle_live']['enemy']['hp'] -= 1  # Still in the same healthy band.
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 2)
        state['battle_moves']['moves'][0]['direct_hit_preview'] = {'normal_damage': [1, 2]}
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 3)

    def test_wild_training_threat_exposes_fourfold_grass_risk_and_resistant_finisher(self):
        state = self.state()
        state['battle_live']['player']['hp'] = 43
        state['battle_live']['enemy'] = {'species': 'Oddish', 'level': 19,
            'hp': 47, 'max_hp': 47, 'status': 'None'}
        before = copy.deepcopy(state)
        reference = training_threat_reference(state)
        self.assertEqual(reference['active_defensive_matchup']['hp'], 43)
        self.assertEqual(reference['active_defensive_matchup']['inferred_powered_move_type_multipliers'],
                         {'Absorb': 4})
        self.assertEqual(reference['party_defensive_matchups'][1]['inferred_powered_move_type_multipliers'],
                         {'Absorb': .25})
        names = [row['move'] for row in reference['enemy_natural_move_reference']['inferred_natural_moves']]
        self.assertIn('Absorb', names)
        self.assertIn('SleepPowder', names)  # Status is retained, not given a damage multiplier.
        self.assertIn('Not observed live enemy moves', reference['scope'])
        self.assertIn('No survival', reference['scope'])
        self.assertEqual(state, before)
        reference['active_defensive_matchup']['hp'] = 0
        reference['enemy_natural_move_reference']['inferred_natural_moves'][0]['power'] = 0
        self.assertEqual(state, before)

    def test_defensive_chart_deduplicates_monotype_species(self):
        state = self.state()
        state['battle_live']['enemy'].update(species='Oddish', level=19)
        state['battle_live']['player'] = {**state['battle_live']['player'], 'species': 'Psyduck'}
        reference = training_threat_reference(state)
        self.assertEqual(reference['active_defensive_matchup']['species_types'], ['Water'])
        self.assertEqual(reference['active_defensive_matchup']['inferred_powered_move_type_multipliers'],
                         {'Absorb': 2})

    def test_unobserved_wild_rules_and_changed_combat_form_do_not_infer_moves(self):
        for update in ({'is_wild': False}, {'is_wild': None}, {'is_ghost': True},
                       {'enemy': {'species': 'Pidgey', 'capture_species': 'Ditto', 'level': 20,
                                  'hp': 50, 'max_hp': 50, 'status': 'None'}}):
            with self.subTest(update=update):
                state = self.state()
                state['battle_live'].update(update)
                reference = training_threat_reference(state)
                self.assertIsNone(reference['enemy_natural_move_reference'])
                self.assertIsNone(reference['active_defensive_matchup']['inferred_powered_move_type_multipliers'])
                self.assertIn('null, not harmless', reference['scope'])

    def test_status_fixed_and_ohko_moves_do_not_inherit_power_damage_multipliers(self):
        from playthrough_late import move_data
        moves = ['Absorb', 'Poisonpowder', 'SeismicToss', 'SuperFang', 'Fissure']
        inferred = {'inferred_natural_moves': [{'move': move, **move_data(move)} for move in moves]}
        with patch('openpokered.playthrough_judgments.capture_threat', return_value=inferred):
            reference = training_threat_reference(self.state())
        self.assertEqual(reference['active_defensive_matchup']['inferred_powered_move_type_multipliers'],
                         {'Absorb': 4})
        self.assertEqual([row['move'] for row in reference['enemy_natural_move_reference']['inferred_natural_moves']], moves)
        self.assertIn('Struggle remain unpredicted', reference['scope'])

    def test_missing_roster_does_not_invent_switch_safety_or_individual_identity(self):
        state = self.state()
        del state['battle_live']['player_party']
        reference = training_threat_reference(state)
        self.assertIsNone(reference['party_defensive_matchups'])
        self.assertNotIn('party_index', reference['active_defensive_matchup'])
        self.assertNotIn('active_party_index', reference)

    def test_training_menu_and_attack_both_receive_threat_without_removing_choices(self):
        state = self.state()
        state['battle_live']['enemy'].update(species='Oddish', level=19)
        game = self.game(state, self.active())
        game.judgments.choose.return_value = 'switch:1'
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        self.assertIn('training_threat_reference', compact)
        self.assertIn('direct_hit_preview_scope', compact)
        self.assertIn('fight', choices)
        self.assertIn('switch:1', choices)
        self.assertIn('ordinary enemy response', instructions)
        game.judgments.choose.return_value = '0'
        game._select_move()
        _, compact, choices, instructions = game.judgments.choose.call_args.args
        self.assertIn('training_threat_reference', compact)
        self.assertEqual(choices, {'0': 'Tackle', '1': 'Selfdestruct', '2': 'RockThrow',
            'back': 'Cancel this move menu without spending a turn; return to compare recovery, switches and FIGHT if no listed attack is suitable for the training turn.'})
        self.assertIn('not actual damage or speed', instructions)

    def test_incoming_finisher_condition_changes_invalidate_training_attack_cache(self):
        state = self.state()
        game = self.game(state, self.active())
        game._select_move()
        state['battle_live']['player_party'][1]['hp'] -= 1
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 2)
        game._select_move()
        self.assertEqual(game.judgments.choose.call_count, 2)


if __name__ == '__main__':
    unittest.main()
