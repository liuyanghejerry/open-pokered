"""Filtered-item choices and source-bound, multi-visit gift preparation."""
import io
import unittest
from unittest.mock import Mock

from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.story_agent import DualStoryAgent, StoryStopped
from openpokered.story_rules import StoryIndex, Rule, compile_story, evaluate, literal
from test_openpokered_story import call, command, conditional, story, facts, FakeModel, Client


def filtered_assignment(options):
    return {'Assign': {'name': 'selected', 'value': {'Call': {
        'callee': 'game.filterBag', 'args': [{'ArrayLit': [literal(v) for v in options]}]}}}}


def selected(value):
    return {'BinaryOp': {'op': 'Eq', 'left': {'Variable': 'selected'}, 'right': literal(value)}}


def fossil_fixture():
    entries = [('DOME_FOSSIL', 'KABUTO'), ('OLD_AMBER', 'AERODACTYL')]
    handovers = [conditional(selected(item), [
        {'Choice': {'options': [{'label': literal('YES'), 'body': [
            command('takeItem', item, 1), command('setFlag', 'REVIVING_' + species),
            command('setFlag', 'GAVE'), command('setFlag', 'WAITING')]},
            {'label': literal('NO'), 'body': []}]}}]) for item, species in entries]
    program = [conditional(call('getFlag', 'GAVE'), [
        conditional(call('getFlag', 'WAITING'), [], [
            *[conditional(call('getFlag', 'REVIVING_' + species),
                          [command('givePokemon', species, 30)]) for _, species in entries],
            command('resetFlag', 'GAVE'), command('resetFlag', 'WAITING')])],
        [filtered_assignment([item for item, _ in entries]), *handovers])]
    source = {**story(program), 'id': 'Lab:doctor', 'map': 'Lab', 'triggers': ['npc:1']}
    reset = {**story([command('resetFlag', 'WAITING')]), 'id': 'Island:@load',
             'map': 'Island', 'triggers': ['load']}
    rules = compile_story(source) + compile_story(reset)
    index = StoryIndex.__new__(StoryIndex)
    index.rules, index.by_effect, index.visibility_defaults = rules, {}, {}
    for rule in rules:
        index.by_effect.setdefault(rule.effect, []).append(rule)
    agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
    agent.collects_dex, agent.index = True, index
    agent._complete_collection_graph = {
        species.title(): [{'method': 'gift', 'map': 'Lab', 'storyline': 'doctor',
                           'level': 30, 'item': item}]
        for item, species in entries}
    observed = {**facts(), 'bag': {'DOMEFOSSIL': 1}, 'coins': 0, 'money': 553,
                'map': 'Center', 'x': 3, 'y': 3, 'dex': {'owned_species': []},
                'stored_pokemon': [], 'box_counts': [0] * 12, 'current_box_index': 0}
    return agent, observed


class FilterRulesTests(unittest.TestCase):
    def test_filter_bag_binds_real_item_guards_and_choice_to_each_branch(self):
        rules = compile_story(story([
            filtered_assignment(['DOME_FOSSIL', 'OLD_AMBER']),
            conditional(selected('DOME_FOSSIL'), [command('setFlag', 'KABUTO')]),
            conditional(selected('OLD_AMBER'), [command('setFlag', 'AERO')])]))
        self.assertEqual(len(rules), 2)
        for rule, item in zip(rules, ['DOME_FOSSIL', 'OLD_AMBER']):
            self.assertEqual(rule.choices, [item])
            self.assertEqual(rule.missing(facts()), [('item', item, True)])
            carried = {**facts(), 'bag': {item.replace('_', ''): 1}}
            self.assertEqual(rule.missing(carried), [])
        self.assertIsNone(evaluate(call('filterBag', 'DOME_FOSSIL'),
                                   {**facts(), 'bag': {'DOMEFOSSIL': 1}}))

    def test_filter_bag_cancel_is_a_real_empty_result_not_an_item_row(self):
        rules = compile_story(story([filtered_assignment(['DOME_FOSSIL']),
            conditional(selected(''), [command('setFlag', 'CANCELLED')])]))
        self.assertEqual(len(rules), 1)
        self.assertEqual(rules[0].choices, ['CANCEL'])
        self.assertEqual(rules[0].missing(facts()), [])

    def test_dynamic_or_malformed_filter_bag_remains_unknown(self):
        for expr in ({'Variable': 'unknown'}, {'ArrayLit': [literal(7)]}):
            with self.subTest(expr=expr):
                assignment = filtered_assignment([])
                assignment['Assign']['value']['Call']['args'] = [expr]
                rule = compile_story(story([assignment,
                    conditional(selected('DOME_FOSSIL'), [command('setFlag', 'KABUTO')])]))[0]
                self.assertEqual(rule.missing(facts())[0][0], 'unknown')


class FilterMenuTests(unittest.TestCase):
    def exercise_menu(self, selection, cursor=0):
        menu = {'frame_count': 0, 'map_name': 'Lab', 'screen': 'filter-bag',
                'field_menu': {'kind': 'filter_bag', 'cursor': cursor,
                               'items': ['DOME_FOSSIL', 'OLD_AMBER']}}
        ready = {'frame_count': 0, 'map_name': 'Lab', 'screen': 'overworld'}
        agent = DualStoryAgent(Client(), FakeModel(),
            [{'id': 'dex', 'agent_verified': True}], trace=io.StringIO())
        agent.client = Mock()
        current = dict(menu)
        agent.client.state.side_effect = lambda: current
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.choose = Mock(return_value=selection)
        taps = []
        def tap(button):
            taps.append(button)
            if button in ('a', 'b'):
                current.clear()
                current.update(ready)
            else:
                current['field_menu'] = {**current['field_menu'], 'cursor': 1}
        agent.tap = tap
        agent.client.step.side_effect = lambda _: None
        agent.settle(('flag', 'REVIVING_AERODACTYL', True))
        return agent, taps

    def test_filter_menu_moves_by_observed_cursor_then_confirms(self):
        agent, taps = self.exercise_menu('1')
        self.assertEqual(taps, ['down', 'a'])
        self.assertEqual(agent.choose.call_count, 1)
        candidates = agent.choose.call_args.args[2]
        self.assertEqual(candidates['1'], 'OLD_AMBER')
        self.assertIn('cancel', candidates)

    def test_filter_menu_cancels_with_b_not_cursor_or_a(self):
        _, taps = self.exercise_menu('cancel', cursor=1)
        self.assertEqual(taps, ['b'])

    def test_invalid_observed_cursor_stops_before_input(self):
        with self.assertRaisesRegex(StoryStopped, 'invalid_filter_bag_menu'):
            self.exercise_menu('1', cursor=True)


class GiftPreparationTests(unittest.TestCase):
    def test_held_dome_creates_preparation_not_premature_registration(self):
        agent, observed = fossil_fixture()
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        self.assertNotIn('register:Kabuto:gift:Lab', groups)
        candidates = [g for g in groups.values() if g.get('context', {}).get('species') == 'Kabuto']
        self.assertTrue(candidates)
        self.assertTrue(all(g['target'][0] != 'register' for g in candidates))
        self.assertTrue(all(not r.missing(observed) for g in candidates for r in g['rules']))
        self.assertTrue(all(g['context']['registration_requires_receipt'] for g in candidates))

    def test_dome_handover_is_never_preparation_for_old_amber(self):
        agent, observed = fossil_fixture()
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        self.assertFalse(any(g.get('context', {}).get('species') == 'Aerodactyl' for g in groups.values()))

    def test_pending_revival_offers_actual_map_load_reset_then_receipt(self):
        agent, observed = fossil_fixture()
        observed['bag'] = {}
        observed['flags'] = {'GAVE': True, 'WAITING': True, 'REVIVING_KABUTO': True}
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        prepares = [g for g in groups.values() if g.get('context', {}).get('species') == 'Kabuto']
        self.assertTrue(prepares)
        self.assertEqual({r.map for g in prepares for r in g['rules']}, {'Island'})
        self.assertEqual({g['target'] for g in prepares}, {('flag', 'WAITING', False)})
        observed['flags']['WAITING'] = False
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        self.assertIn('register:Kabuto:gift:Lab', groups)
        self.assertFalse(any(k.startswith('acquisition-prepare:Kabuto:') for k in groups))

    def test_unknown_source_guard_never_becomes_certified_preparation(self):
        agent, observed = fossil_fixture()
        for rule in agent.acquisition_story_rules('Kabuto', agent._complete_collection_graph['Kabuto'][0]):
            rule.guards.append(({'Unsupported': 'unknown'}, True))
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        self.assertFalse(any(g.get('context', {}).get('species') == 'Kabuto' for g in groups.values()))

    def test_delivery_capacity_is_required_before_consuming_a_fossil(self):
        agent, observed = fossil_fixture()
        observed['party'] = [{'species': 'Pidgey', 'hp': 10, 'level': 10}] * 6
        observed['box_counts'] = [20] * 12
        agent.add_box_capacity_group = Mock()
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        self.assertFalse(any(k.startswith('acquisition-prepare:') for k in groups))
        self.assertTrue(agent.add_box_capacity_group.called)

    def test_multiple_reward_paths_share_one_item_preparation_candidate(self):
        agent, observed = fossil_fixture()
        amber = Rule('amber', 'Museum', 'Museum:scientist', ['npc:1'], [], [],
                     ('item', 'OLD_AMBER', True), [])
        agent.index.rules.append(amber)
        agent.index.by_effect[amber.effect] = [amber]
        groups = {}
        agent.add_nonwild_collection_groups(groups, observed)
        prepares = [g for g in groups.values() if g.get('context', {}).get('species') == 'Aerodactyl']
        self.assertEqual(len(prepares), 1)
        self.assertEqual(prepares[0]['target'], amber.effect)
        self.assertGreater(len(prepares[0]['context']['source_preparation_paths']), 1)


if __name__ == '__main__':
    unittest.main()
