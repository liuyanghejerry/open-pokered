"""Finite TM funding must expose native prices and irrecoverable teaching costs."""
from copy import deepcopy
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from openpokered.autonomous_story import (AutonomousStoryAgent, native_tm_sale_catalog,
                                         parse_native_tm_sale_catalog)
from openpokered.story_rules import Rule


class TmFundingTests(unittest.TestCase):
    def setup_funding(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        agent.visited = {'FuchsiaMart'}
        rule = Rule('mart', 'FuchsiaMart', 'mart:clerk', ['npc:1'], [], [],
                    ('shop', ('POKE_BALL', 'GREAT_BALL'), True), [])
        agent.index = Mock(rules=[rule])
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'FuchsiaMart'}]}
        agent.item_evolution_spending_reference = Mock(return_value={'held_source_options': [
            {'species': 'Arcanine', 'from_species': 'Growlithe',
             'affordable_before_purchase': False, 'affordable_after_purchase': True,
             'purchase_removes_affordability': False}]})
        facts = {'map': 'FuchsiaPokecenter', 'money': 93,
                 'bag': {'TM34': 1, 'TM11': 1, 'TM24': 1, 'TM06': 1, 'TM21': 1,
                         'HM01': 1, 'HM03': 1, 'POKEBALL': 36, 'MASTERBALL': 1, 'MOONSTONE': 1},
                 'flags': {}, 'party': [
                     {'species': 'Charizard', 'level': 59, 'hp': 201,
                      'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig']},
                     {'species': 'Hypno', 'level': 38, 'hp': 129,
                      'moves': ['Headbutt', 'Hypnosis', 'Disable', 'PsychicM']}],
                 'stored_pokemon': [{'species': 'Pikachu', 'box': 0, 'index': 4, 'level': 22,
                                     'hp': 40, 'moves': ['Thunderbolt', 'Growl', 'None', 'None']}]}
        return agent, facts, rule

    def test_catalog_reads_all_fifty_native_tm_prices_and_exact_enum_menu_identities(self):
        catalog = native_tm_sale_catalog()
        self.assertEqual(set(catalog), {f'Tm{i:02d}' for i in range(1, 51)})
        self.assertFalse(any(name.startswith('Hm') for name in catalog))
        for name, price, move in [('Tm34', 2000, 'Bide'), ('Tm11', 2000, 'Bubblebeam'),
                                  ('Tm24', 2000, 'Thunderbolt'), ('Tm06', 4000, 'Toxic'),
                                  ('Tm21', 5000, 'MegaDrain')]:
            with self.subTest(name=name):
                self.assertEqual(catalog[name]['price'], price)
                self.assertEqual(catalog[name]['effect']['params']['move'], move)
                self.assertEqual(catalog[name]['name'], name.upper())
                self.assertFalse(catalog[name]['key_item'])

    def test_unknown_key_item_nonpositive_or_mismatched_definitions_do_not_authorize_sale(self):
        header = 'pub const TMHM_DATA: [ItemData; 1] = [\n'
        for row in ('ItemData { id: ItemId::Tm00, name: "TM00", price: 2000, is_key_item: false }',
                    'ItemData { id: ItemId::Tm51, name: "TM51", price: 2000, is_key_item: false }',
                    'ItemData { id: ItemId::Tm34, name: "TM35", price: 2000, is_key_item: false }',
                    'ItemData { id: ItemId::Tm34, name: "TM34", price: 0, is_key_item: false }',
                    'ItemData { id: ItemId::Tm34, name: "TM34", price: 2000, is_key_item: true }',
                    'ItemData { id: ItemId::Hm01, name: "HM01", price: 2000, is_key_item: false }'):
            with self.subTest(row=row):
                self.assertEqual(parse_native_tm_sale_catalog(header + row + '\n];'), {})
        self.assertEqual(parse_native_tm_sale_catalog('unknown metadata'), {})

    def test_actual_five_finite_tms_are_optional_sales_not_hms_balls_or_stones(self):
        agent, facts, _ = self.setup_funding()
        original = deepcopy(facts)
        existing = {'target': ('level', 'Hypno', 39), 'context': {'retained': True}}
        groups = {'existing': existing}
        agent.add_collection_funding(groups, facts)
        sales = {g['target'][1]: g for g in groups.values() if g['target'][0] == 'sale'}
        self.assertEqual(set(sales), {'Tm34', 'Tm11', 'Tm24', 'Tm06', 'Tm21'})
        for name, proceeds in [('Tm34', 1000), ('Tm11', 1000), ('Tm24', 1000), ('Tm06', 2000), ('Tm21', 2500)]:
            context = sales[name]['context']
            self.assertTrue(context['optional_preparation'] and context['tm_sale'])
            self.assertFalse(context['treasure_sale'])
            self.assertEqual(context['expected_proceeds'], proceeds)
            reference = context['collection_funding_reference']
            self.assertEqual(reference['money_before_sale'], 93)
            self.assertEqual(reference['money_after_sale_reference'], 93 + proceeds)
            self.assertIn('not a joint', reference['scope'])
            self.assertIn('not a surplus certificate', context['sale_opportunity_cost']['scope'])
            self.assertEqual(context['sale_opportunity_cost']['quantity_relinquished'], 1)
        self.assertIs(groups['existing'], existing)
        self.assertEqual(facts, original)

    def test_tm_retention_names_actual_move_compatibility_pc_identity_and_shared_copy_limit(self):
        agent, facts, _ = self.setup_funding()
        reference = agent.tm_retention_reference(facts, native_tm_sale_catalog()['Tm24'], 2)
        self.assertEqual(reference['learned_move'], 'Thunderbolt')
        self.assertEqual(reference['move_data']['power'], 95)
        self.assertEqual(reference['copies_held'], 2)
        self.assertFalse(any(row['species'] == 'Charizard' for row in reference['compatible_held_recipients']))
        pc = next(row for row in reference['compatible_held_recipients'] if row['origin'] == 'pc')
        self.assertEqual((pc['box'], pc['index']), (0, 4))
        self.assertTrue(pc['already_knows_move'] and pc['withdrawal_required'])
        self.assertIn('not a joint moveset yield', reference['scope'])
        self.assertIn('does not rule out future', reference['scope'])

    def test_sparse_recipient_observations_remain_unknown_not_healthy_or_unlearned(self):
        agent, facts, _ = self.setup_funding()
        facts['party'] = [{'species': 'Charizard'}]
        facts['stored_pokemon'] = []
        reference = agent.tm_retention_reference(facts, native_tm_sale_catalog()['Tm34'], 1)
        recipient, = reference['compatible_held_recipients']
        self.assertIsNone(recipient['conscious'])
        self.assertIsNone(recipient['level'])
        self.assertIsNone(recipient['observed_moves'])
        self.assertIsNone(recipient['already_knows_move'])

    def test_funding_requires_actual_positive_integer_stack_and_known_reachable_guarded_shop(self):
        for case in ('not_collecting', 'unvisited', 'unreachable', 'guard_missing',
                     'missing', 'zero', 'negative', 'bool', 'float', 'string'):
            with self.subTest(case=case):
                agent, facts, rule = self.setup_funding()
                facts['bag'] = {'TM34': 1}
                if case == 'not_collecting': agent.collects_dex = False
                if case == 'unvisited': agent.visited.clear()
                if case == 'unreachable': agent.client.route.return_value = {'found': False}
                if case == 'guard_missing': rule.missing = Mock(return_value=['ACCESS'])
                if case == 'missing': facts['bag'] = {}
                for name, value in [('zero', 0), ('negative', -1), ('bool', True), ('float', 1.5), ('string', '1')]:
                    if case == name: facts['bag']['TM34'] = value
                groups = {}
                agent.add_collection_funding(groups, facts)
                self.assertEqual(groups, {})

    def test_sale_uses_the_held_stack_without_removing_any_other_strategy(self):
        agent, facts, _ = self.setup_funding()
        facts['bag'] = {'TM34': 3}
        groups = {'other': {'target': ('badge', 6, True)}}
        agent.add_collection_funding(groups, facts)
        sale, = [value for value in groups.values() if value['target'][0] == 'sale']
        self.assertEqual(sale['context']['quantity'], 3)
        self.assertEqual(sale['context']['expected_proceeds'], 3000)
        self.assertEqual(sale['context']['sale_opportunity_cost']['move_teaching_reference']['copies_held'], 3)
        self.assertEqual(groups['other']['target'], ('badge', 6, True))
        self.assertEqual(facts['money'], 93)

    def test_normal_shop_sale_uses_exact_bag_enum_name_and_distinct_tm_receipt(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('sale', 'Tm34', False), 'context': {'tm_sale': True}}
        agent.client, agent.game, agent.record = Mock(), Mock(), Mock()
        agent.client.state.return_value = {'money': 1093}
        with patch('openpokered.autonomous_story.data.sell') as sell:
            self.assertTrue(agent.settle_special({'shop_phase': 'MainMenu', 'money': 93}))
        sell.assert_called_once_with(agent.game, 'Tm34')
        agent.record.assert_called_once_with('sold_tm', item='Tm34', money_after=1093)


if __name__ == '__main__':
    unittest.main()
