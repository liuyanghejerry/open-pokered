"""Blackout evidence must not become an escape or an improved retry setup."""
import json
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import Mock

from openpokered import autonomous_story as policy
from openpokered.autonomous_story import AutonomousStoryAgent


def battle_pair(money=53, remaining=27):
    mon = {'species': 'Gloom', 'level': 46, 'hp': 129, 'max_hp': 129,
           'status': 'None', 'moves': ['SleepPowder'], 'pp': [15]}
    enemy = {'species': 'Zapdos', 'capture_species': 'Zapdos', 'level': 50,
             'hp': 165, 'max_hp': 165, 'status': 'None'}
    before = {'screen': 'battle', 'map_name': 'PowerPlant',
              'script_awaiting_battle': True, 'money': money, 'party': [mon],
              'pokedex': {'owned_species': ['Gloom']},
              'box_counts': [0], 'current_box_index': 0,
              'battle_inventory': [{'item': 'UltraBall', 'qty': 5},
                                   {'item': 'GreatBall', 'qty': 18}],
              'battle_live': {'is_wild': True, 'enemy': enemy,
                              'player_party': [mon]}}
    after = deepcopy(before)
    after.update(screen='overworld', script_awaiting_battle=False, money=remaining,
                 battle_phase='BattleOver { won: false, escaped: false, wait_frames: 48 }',
                 battle_inventory=[{'item': 'GreatBall', 'qty': 9}])
    after['party'] = deepcopy(before['party'])
    after['battle_live']['player_party'][0]['hp'] = 0
    # Native persistent party is already healed; live battle party is not.
    return before, after


def agent_fixture():
    agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
    agent.collects_dex = True
    agent.capture_retreats, agent.capture_retreat_totals = {}, {}
    agent.capture_blackouts, agent.capture_blackout_totals = {}, {}
    agent.battle_defeats, agent.defeat_preparation = [], 0
    agent.record, agent.dex_progress = Mock(), Mock(return_value={})
    agent.capture_source_evidence = Mock(return_value={'catchable_source_indexed': True})
    return agent


class CaptureBlackoutTests(unittest.TestCase):
    def test_observed_blackout_is_not_counted_as_a_successful_escape(self):
        agent = agent_fixture()
        before, after = battle_pair()
        agent.observe_battle_result(before, after)
        row = agent.capture_blackouts['PowerPlant:Zapdos']
        self.assertEqual(row['reason'], 'native_blackout_without_registration')
        self.assertEqual(row['preparation']['party'][0]['hp'], 129)
        self.assertEqual(row['blackout_observation']['party'][0]['hp'], 0)
        self.assertEqual(row['money_before'], 53)
        self.assertEqual(row['money_after'], 27)
        self.assertEqual(agent.capture_retreats, {})
        self.assertEqual(agent.capture_retreat_totals, {})
        self.assertEqual(agent.record.call_args_list[0].args, ('capture_blackout',))

    def test_automatic_blackout_healing_does_not_reopen_identical_setup(self):
        agent = agent_fixture()
        before, after = battle_pair()
        agent.observe_battle_result(before, after)
        facts = {'party': after['party'], 'bag': {'GREATBALL': 9},
                 'box_counts': [0], 'current_box_index': 0}
        self.assertTrue(agent.static_capture_deferred('ZAPDOS', 'PowerPlant', facts))
        facts['party'] = deepcopy(facts['party'])
        facts['party'][0]['level'] += 1
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        facts['party'][0]['level'] -= 1
        facts['bag']['ULTRABALL'] = 6
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))

    def test_actual_costs_have_observation_counts_and_unknown_is_not_zero(self):
        before, after = battle_pair()
        totals = {}
        row = policy.capture_blackout_evidence(before, after)
        policy.accumulate_capture_blackout(totals, row)
        before2, after2 = battle_pair(27, 14)
        before2['battle_inventory'] = [{'item': 'GreatBall', 'qty': 9}]
        after2['battle_inventory'] = []
        policy.accumulate_capture_blackout(totals, policy.capture_blackout_evidence(before2, after2))
        result = totals['PowerPlant:Zapdos']
        self.assertEqual(result, {'recorded_blackouts': 2, 'inventory_observed_blackouts': 2,
                                 'balls_spent': {'ULTRABALL': 5, 'GREATBALL': 18},
                                 'cash_observed_blackouts': 2, 'cash_decrease': 39})
        before.pop('money')
        after.pop('money')
        before.pop('battle_inventory')
        after.pop('battle_inventory')
        unknown = policy.capture_blackout_evidence(before, after)
        self.assertNotIn('money_before', unknown)
        policy.accumulate_capture_blackout(totals, unknown)
        self.assertEqual(result['recorded_blackouts'], 3)
        self.assertEqual(result['inventory_observed_blackouts'], 2)
        self.assertEqual(result['cash_observed_blackouts'], 2)
        self.assertEqual(result['cash_decrease'], 39)

    def test_zero_is_observed_but_bool_negative_and_float_money_are_not(self):
        for money in (0, True, -1, 1.0, '1', None):
            with self.subTest(money=money):
                before, after = battle_pair(money, money)
                before['battle_inventory'] = after['battle_inventory'] = []
                totals = {}
                policy.accumulate_capture_blackout(totals, policy.capture_blackout_evidence(before, after))
                row = totals['PowerPlant:Zapdos']
                self.assertEqual(row['inventory_observed_blackouts'], 1)
                self.assertEqual(row['cash_observed_blackouts'], int(type(money) is int and money >= 0))
                self.assertEqual(row['cash_decrease'], 0)

    def test_unrelated_or_unproven_results_cannot_create_blackout_evidence(self):
        variants = [
            ('trainer', lambda b, a: b['battle_live'].update(is_wild=False)),
            ('random', lambda b, a: b.update(script_awaiting_battle=False)),
            ('safari', lambda b, a: b['battle_live'].update(is_safari=True)),
            ('ghost', lambda b, a: b['battle_live'].update(is_ghost=True)),
            ('blocked', lambda b, a: b['battle_live'].update(capture_blocked_reason='restless_soul')),
            ('caught', lambda b, a: a['pokedex']['owned_species'].append('Zapdos')),
            ('unknown_dex', lambda b, a: a.pop('pokedex')),
            ('escape', lambda b, a: a.update(battle_phase='BattleOver { won: false, escaped: true }')),
            ('win', lambda b, a: a.update(battle_phase='BattleOver { won: true, escaped: false }')),
            ('nonterminal', lambda b, a: a.update(battle_phase='SelectAction')),
            ('survivor', lambda b, a: a['battle_live']['player_party'][0].update(hp=1)),
            ('unknown_party', lambda b, a: a['battle_live'].pop('player_party')),
            ('other_map', lambda b, a: a.update(map_name='Route10')),
            ('other_enemy', lambda b, a: a['battle_live']['enemy'].update(capture_species='Moltres')),
        ]
        for name, mutate in variants:
            with self.subTest(name=name):
                before, after = battle_pair()
                mutate(before, after)
                self.assertIsNone(policy.capture_blackout_evidence(before, after))

    def test_malformed_inventory_never_certifies_a_full_observation(self):
        for inventory in (None, {}, [{'item': 'GreatBall', 'qty': True}],
                          [{'item': 'GreatBall', 'qty': -1}], [{'qty': 1}]):
            with self.subTest(inventory=inventory):
                before, after = battle_pair()
                before['battle_inventory'] = inventory
                totals = {}
                policy.accumulate_capture_blackout(totals, policy.capture_blackout_evidence(before, after))
                self.assertEqual(totals['PowerPlant:Zapdos']['inventory_observed_blackouts'], 0)

    def test_latest_blackout_supersedes_old_escape_and_later_escape_supersedes_it(self):
        agent = agent_fixture()
        before, after = battle_pair()
        escaped = deepcopy(after)
        escaped['battle_phase'] = 'BattleOver { won: false, escaped: true }'
        agent.observe_battle_result(before, escaped)
        stronger = deepcopy(before)
        stronger['party'][0]['level'] = stronger['battle_live']['player_party'][0]['level'] = 48
        agent.observe_battle_result(stronger, after)
        facts = {'party': stronger['party'], 'bag': {'GREATBALL': 18, 'ULTRABALL': 5}}
        self.assertTrue(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        agent.observe_battle_result(before, escaped)
        self.assertEqual(agent.capture_blackouts, {})
        self.assertFalse(agent.static_capture_deferred('Zapdos', 'PowerPlant', facts))
        self.assertEqual(agent.capture_blackout_totals['PowerPlant:Zapdos']['recorded_blackouts'], 1)

    def test_blackout_remains_visible_when_preparation_reopens_retry(self):
        agent = agent_fixture()
        before, after = battle_pair()
        agent.observe_battle_result(before, after)
        facts = {'party': before['party'], 'bag': {'ULTRABALL': 6}}
        state = {}
        agent.augment_strategy_state(state, facts)
        self.assertEqual(state['capture_blackouts_requiring_preparation'], [])
        self.assertEqual(state['capture_retreats_requiring_preparation'], [])
        row = state['capture_blackout_retry_evidence'][0]
        self.assertEqual(row['recorded_history']['cash_decrease'], 26)
        self.assertEqual(row['blackout_observation']['party'][0]['hp'], 0)
        self.assertEqual(row['preparation_changes_since_attempt'], ['more_ball_stock:ULTRABALL'])
        self.assertNotIn('preparation_changes_since_attempt', agent.capture_blackouts['PowerPlant:Zapdos'])

    def test_blackout_offers_real_bounded_support_training(self):
        from test_openpokered_autonomous import AutonomousTests
        # Reuse the established earned-party/training-site fixture only.
        owner = AutonomousTests()
        agent, facts = owner.support_training_agent()
        retreat = agent.capture_retreats.pop('PowerPlant:Zapdos')
        observation = retreat.pop('retreat_observation')
        retreat['blackout_observation'] = observation
        retreat['reason'] = 'native_blackout_without_registration'
        agent.capture_blackouts = {'PowerPlant:Zapdos': retreat}
        groups = {}
        agent.add_capture_support_training(groups, facts)
        self.assertEqual(groups['prepare:capture-support:Gloom']['target'], ('level', 'Gloom', 25))
        failure = groups['prepare:capture-support:Gloom']['context']['observed_failed_capture_setups'][0]
        self.assertEqual(failure['result_kind'], 'native_blackout_without_registration')


class BlackoutCheckpointTests(unittest.TestCase):
    def write_run(self, path, before, after, parent=None, extra=None):
        path.mkdir()
        path.joinpath('summary.json').write_text(json.dumps({'resumed_from': str(parent) if parent else None,
                                                           **(extra or {})}))
        events = [{'kind': 'battle_started', 'elapsed_s': 1, 'state': before},
                  {'kind': 'battle_resolved', 'elapsed_s': 2, 'state': after}]
        path.joinpath('trace.jsonl').write_text(''.join(json.dumps(event) + '\n' for event in events))

    def test_replays_matching_real_pairs_only_in_selected_lineage(self):
        from openpokered.run_autonomous import checkpoint_capture_blackouts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            before, after = battle_pair()
            self.write_run(root / 'parent', before, after)
            before2, after2 = battle_pair(27, 14)
            before2['battle_inventory'] = [{'item': 'GreatBall', 'qty': 9}]
            after2['battle_inventory'] = []
            self.write_run(root / 'child', before2, after2, root / 'parent')
            self.write_run(root / 'sibling', before, after, root / 'parent')
            rows, totals = checkpoint_capture_blackouts(root / 'child')
            self.assertEqual(totals['PowerPlant:Zapdos']['recorded_blackouts'], 2)
            self.assertEqual(totals['PowerPlant:Zapdos']['cash_decrease'], 39)
            self.assertEqual(totals['PowerPlant:Zapdos']['balls_spent'], {'ULTRABALL': 5, 'GREATBALL': 18})
            self.assertEqual(rows['PowerPlant:Zapdos']['money_before'], 27)
            self.assertEqual(rows['PowerPlant:Zapdos']['recorded_battle_pair']['trace'], str((root / 'child' / 'trace.jsonl').resolve()))

    def test_explicit_schema_is_authoritative_including_empty_and_does_not_double_count(self):
        from openpokered.run_autonomous import checkpoint_capture_blackouts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            before, after = battle_pair()
            self.write_run(root / 'parent', before, after)
            rows, totals = checkpoint_capture_blackouts(root / 'parent')
            schema = {'capture_blackouts_schema': 1, 'capture_blackouts': rows,
                      'capture_blackout_totals': totals}
            self.write_run(root / 'child', before, after, root / 'parent', schema)
            self.assertEqual(checkpoint_capture_blackouts(root / 'child'), (rows, totals))
            schema.update(capture_blackouts={}, capture_blackout_totals={})
            root.joinpath('child/summary.json').write_text(json.dumps(schema))
            self.assertEqual(checkpoint_capture_blackouts(root / 'child'), ({}, {}))

    def test_later_escape_clears_latest_blackout_but_not_observed_cost_history(self):
        from openpokered.run_autonomous import checkpoint_capture_blackouts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            before, after = battle_pair()
            self.write_run(root / 'parent', before, after)
            self.write_run(root / 'child', before, before, root / 'parent')
            escape = {'kind': 'capture_retreat', 'map': 'PowerPlant', 'species': 'Zapdos'}
            root.joinpath('child/trace.jsonl').write_text(json.dumps(escape) + '\n')
            rows, totals = checkpoint_capture_blackouts(root / 'child')
            self.assertEqual(rows, {})
            self.assertEqual(totals['PowerPlant:Zapdos']['recorded_blackouts'], 1)

    def test_missing_or_mismatched_or_duplicate_result_never_creates_extra_attempt(self):
        from openpokered.run_autonomous import checkpoint_capture_blackouts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            before, after = battle_pair()
            self.write_run(root / 'run', before, after)
            trace = root / 'run/trace.jsonl'
            original = trace.read_text()
            result = json.dumps({'kind': 'battle_resolved', 'state': after}) + '\n'
            trace.write_text(original + result)
            self.assertEqual(checkpoint_capture_blackouts(root / 'run')[1]['PowerPlant:Zapdos']['recorded_blackouts'], 1)
            trace.write_text(result)
            self.assertEqual(checkpoint_capture_blackouts(root / 'run'), ({}, {}))
            mismatch = deepcopy(after)
            mismatch['map_name'] = 'Route10'
            trace.write_text(json.dumps({'kind': 'battle_started', 'state': before}) + '\n' +
                             json.dumps({'kind': 'battle_resolved', 'state': mismatch}) + '\n')
            self.assertEqual(checkpoint_capture_blackouts(root / 'run'), ({}, {}))

    def test_cycles_and_malformed_schema_fail_closed(self):
        from openpokered.run_autonomous import checkpoint_capture_blackouts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            before, after = battle_pair()
            self.write_run(root / 'run', before, after, root / 'run')
            with self.assertRaisesRegex(ValueError, 'cycle'):
                checkpoint_capture_blackouts(root / 'run')
            root.joinpath('run/summary.json').write_text(json.dumps({'capture_blackouts_schema': 1,
                                                                   'capture_blackouts': []}))
            with self.assertRaisesRegex(ValueError, 'schema'):
                checkpoint_capture_blackouts(root / 'run')


if __name__ == '__main__':
    unittest.main()
