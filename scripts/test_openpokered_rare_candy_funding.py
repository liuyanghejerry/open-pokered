"""Optional finite level-item funding must expose the real retention tradeoff."""
import copy
import unittest
from unittest.mock import Mock, patch

import test_openpokered_autonomous as fixtures
from openpokered.autonomous_story import AutonomousStoryAgent, ITEM_CATALOG, level_experience


class RareCandyFundingTests(unittest.TestCase):
    def fixture(self, quantity=1):
        agent, facts = fixtures.AutonomousTests().collection_funding_agent()
        facts['bag'].update(NUGGET=0, RARECANDY=quantity)
        facts['money'] = 14
        agent._complete_collection_graph.update({
            'Magikarp': [{'method': 'fishing', 'map': 'PalletTown'}],
            'Gyarados': [{'method': 'evolution', 'trigger': 'level',
                         'from_species': 'Magikarp', 'level': 20}],
            'Geodude': [{'method': 'grass', 'map': 'MtMoon1F'}],
            'Graveler': [{'method': 'evolution', 'trigger': 'level',
                         'from_species': 'Geodude', 'level': 25}],
            'Golem': [{'method': 'evolution', 'trigger': 'trade',
                      'from_species': 'Graveler', 'external_trade': True}],
        })
        facts['party'] = [{'species': 'Magikarp', 'level': 19,
                           'experience': level_experience('Magikarp', 19) + 100}]
        facts['stored_pokemon'] = [{'species': 'Geodude', 'level': 26,
                                   'experience': level_experience('Geodude', 26) + 50,
                                   'box': 2, 'index': 4}]
        facts['dex']['owned_species'] = ['Magikarp', 'Geodude']
        return agent, facts

    def sale(self, agent, facts):
        groups = {}
        agent.add_collection_funding(groups, facts)
        return next(group for group in groups.values() if group['target'] == ('sale', 'RareCandy', False))

    def test_owned_candy_sale_is_optional_and_never_credits_preview_money(self):
        agent, facts = self.fixture(2)
        original = copy.deepcopy(facts)
        group = self.sale(agent, facts)
        context = group['context']
        self.assertTrue(context['optional_preparation'])
        self.assertTrue(context['rare_candy_sale'])
        self.assertFalse(context['treasure_sale'])
        self.assertEqual(context['quantity'], 2)
        self.assertEqual(context['expected_proceeds'], 4800)
        reference = context['collection_funding_reference']
        self.assertEqual(reference['money_after_sale_reference'], 4814)
        self.assertEqual(reference['ball_purchase_options_here'][0]['max_quantity_affordable_after_sale'], 24)
        self.assertEqual(context['sale_opportunity_cost']['quantity_relinquished'], 2)
        self.assertEqual(context['sale_opportunity_cost']['retained_item_effect']['type'], 'RareCandy')
        self.assertIn('not a surplus certificate', context['sale_opportunity_cost']['scope'])
        self.assertEqual(facts, original)

    def test_retention_uses_exact_observed_xp_and_overlevel_evolution_requires_new_gain(self):
        agent, facts = self.fixture()
        reference = self.sale(agent, facts)['context']['sale_opportunity_cost']['level_up_reference']
        options = reference['held_source_level_options']
        self.assertEqual(len(options), 2)
        party, stored = options
        self.assertEqual((party['origin'], party['index'], party['level_after_one_candy']), ('party', 0, 20))
        self.assertEqual(party['potential_unregistered_level_evolutions'], ['Gyarados'])
        cost = party['normal_training_cost_to_next_level']
        remaining = level_experience('Magikarp', 20) - facts['party'][0]['experience']
        self.assertEqual(cost['remaining_experience_min'], remaining)
        self.assertEqual(cost['remaining_experience_max'], remaining)
        self.assertEqual((stored['origin'], stored['box'], stored['index'], stored['level_after_one_candy']), ('pc', 2, 4, 27))
        self.assertEqual(stored['potential_unregistered_level_evolutions'], ['Graveler'])
        self.assertIn('PC withdrawal', reference['scope'])
        self.assertIn('not a joint registration yield', reference['scope'])

    def test_retention_does_not_claim_stone_trade_registered_or_multiple_level_yields(self):
        agent, facts = self.fixture(3)
        facts['dex']['owned_species'].append('Gyarados')
        facts['party'].append({'species': 'Growlithe', 'level': 20})
        facts['stored_pokemon'].append({'species': 'Graveler', 'level': 40, 'box': 1, 'index': 2})
        reference = self.sale(agent, facts)['context']['sale_opportunity_cost']['level_up_reference']
        options = reference['held_source_level_options']
        self.assertEqual(options[0]['potential_unregistered_level_evolutions'], [])
        self.assertEqual(next(row for row in options if row['species'] == 'Growlithe')['potential_unregistered_level_evolutions'], [])
        self.assertEqual(next(row for row in options if row['species'] == 'Graveler')['potential_unregistered_level_evolutions'], [])
        self.assertTrue(all(row['level_after_one_candy'] == row['level_before'] + 1 for row in options))
        self.assertEqual(reference['candies_held'], 3)

    def test_unknown_xp_stays_a_range_not_zero_or_synthetic_observed_experience(self):
        agent, facts = self.fixture()
        facts['party'][0].pop('experience')
        cost = self.sale(agent, facts)['context']['sale_opportunity_cost']['level_up_reference']['held_source_level_options'][0]['normal_training_cost_to_next_level']
        self.assertEqual(cost['remaining_experience_min'], 1)
        self.assertGreater(cost['remaining_experience_max'], 1)
        self.assertNotIn('observed_experience', cost)

    def test_invalid_or_maximum_levels_do_not_certify_a_usable_recipient(self):
        for level in (None, True, '19', 0, 100, 101):
            with self.subTest(level=level):
                agent, facts = self.fixture()
                facts['party'][0]['level'] = level
                reference = self.sale(agent, facts)['context']['sale_opportunity_cost']['level_up_reference']
                self.assertEqual([row['species'] for row in reference['held_source_level_options']], ['Geodude'])

    def test_native_sale_guards_effect_identity_and_known_shop_remain_required(self):
        for change in ({'sellable': False}, {'key_item': True}, {'price': 0},
                       {'tags': []}, {'effect': {'type': 'HealHP'}}):
            with self.subTest(change=change):
                agent, facts = self.fixture()
                with patch.dict(ITEM_CATALOG, {'RareCandy': {**ITEM_CATALOG['RareCandy'], **change}}):
                    groups = {}
                    agent.add_collection_funding(groups, facts)
                self.assertEqual(groups, {})
        for case in ('not_collecting', 'unvisited', 'unreachable', 'guard_missing', 'none_held'):
            with self.subTest(case=case):
                agent, facts = self.fixture()
                if case == 'not_collecting': agent.collects_dex = False
                if case == 'unvisited': agent.visited.clear()
                if case == 'unreachable': agent.client.route.return_value = {'found': False}
                if case == 'guard_missing': agent.index.rules[0].missing = Mock(return_value=['ACCESS'])
                if case == 'none_held': facts['bag']['RARECANDY'] = 0
                groups = {}
                agent.add_collection_funding(groups, facts)
                self.assertEqual(groups, {})

    def test_other_evolution_and_quest_items_stay_protected_and_existing_candidates_unchanged(self):
        agent, facts = self.fixture()
        facts['bag'].update(NUGGET=1, CARBOS=1, MOONSTONE=1, FIRESTONE=1, HM03=1, TM34=1, DOMEFOSSIL=1, POKEBALL=5)
        without = copy.deepcopy(facts)
        without['bag'].pop('RARECANDY')
        old, new = {}, {}
        agent.add_collection_funding(old, without)
        agent.add_collection_funding(new, facts)
        self.assertTrue(all(new[key] == value for key, value in old.items()))
        self.assertEqual({g['target'][1] for g in new.values()}, {'Nugget', 'Carbos', 'RareCandy', 'Tm34'})
        tm = next(g for g in new.values() if g['target'][1] == 'Tm34')
        self.assertTrue(tm['context']['tm_sale'])
        self.assertFalse(tm['context']['treasure_sale'])
        self.assertIn('move_teaching_reference', tm['context']['sale_opportunity_cost'])
        self.assertEqual(len(new), len(old) + 1)

    def test_actual_receipt_is_labeled_level_item_not_treasure(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('sale', 'RareCandy', False), 'context': {'rare_candy_sale': True}}
        agent.client, agent.game, agent.record = Mock(), Mock(), Mock()
        agent.client.state.return_value = {'money': 2414}
        with patch('openpokered.autonomous_story.data.sell') as sell:
            self.assertTrue(agent.settle_special({'shop_phase': 'MainMenu', 'money': 14}))
        sell.assert_called_once_with(agent.game, 'RareCandy')
        agent.record.assert_called_once_with('sold_level_item', item='RareCandy', money_after=2414)

    def test_retention_is_computed_once_per_item_not_once_per_shop(self):
        agent, facts = self.fixture()
        first = agent.index.rules[0]
        second = Mock(id='second-shop', map=first.map, effect=first.effect)
        second.missing.return_value = []
        agent.index.rules.append(second)
        with patch.object(agent, 'rare_candy_retention_reference', wraps=agent.rare_candy_retention_reference) as retention:
            groups = {}
            agent.add_collection_funding(groups, facts)
        self.assertEqual(len(groups), 2)
        retention.assert_called_once_with(facts, 1)


if __name__ == '__main__':
    unittest.main()
