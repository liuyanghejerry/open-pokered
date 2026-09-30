"""Autonomous skill contracts: real-input boundary and grounded preparation."""
import json
import sys
import time
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.playthrough_judgments import ObservedProtocol, move_question, JevGame, replacement_options, medicine_options
from openpokered.autonomous_story import (AutonomousStoryAgent, counter_approaches, reachable_grass,
                                          training_tile, battle_readiness, encounter_value)
from openpokered.story_agent import StoryStopped
from openpokered.navigation_skills import cut_requirement, surf_requirement, water_tile, hm_compatible, water_planning
from openpokered.story_rules import Rule
from openpokered.run_autonomous import observations_valid


class AutonomousTests(unittest.TestCase):
    def test_surf_finds_an_executable_shore_when_shortest_route_enters_water_across_map_border(self):
        import playthrough as pt
        from openpokered.navigation_skills import surf_requirement, water_tile
        state = {'map_name': 'CinnabarIsland', 'player_x': 19, 'player_y': 4,
                 'player_transport': 'Walking'}
        original = pt.cross_step
        crossing = surf_requirement(state, 'PalletTown', [(5, 6)], 'Route20')
        self.assertIsNotNone(crossing)
        self.assertIs(pt.cross_step, original)
        name, stance = crossing['map'], crossing['stance']
        self.assertTrue(pt.bfs_cross('CinnabarIsland', (19, 4), name, tuple(stance),
                                     last_map='Route20', allow_ledges=True))
        dx, dy = pt.DELTA[crossing['direction']]
        self.assertTrue(water_tile(name, stance[0]+dx, stance[1]+dy))

    def test_branched_fall_uses_only_positions_matching_its_landing(self):
        from openpokered.story_rules import literal
        from openpokered.autonomous_story import trigger_position_matches
        condition = {'BinaryOp': {'op': 'Eq', 'left': {'Call': {'callee': 'getPlayerX', 'args': []}},
                                  'right': literal(19)}}
        branch = Rule('fall', 'Room', 'Room:fall', [], [(condition, True)], [],
                      ('transport', ('LowerRoom', 18, 14), True), [])
        self.assertFalse(trigger_position_matches(branch, (16, 14)))
        self.assertFalse(trigger_position_matches(branch, (17, 14)))
        self.assertTrue(trigger_position_matches(branch, (19, 14)))

    def test_mechanism_parent_survives_a_frontier_change_after_switching(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        target = ('item', 'KEY', True)
        pickup = Rule('key', 'Room', 'Room:key', [], [], [], target, [])
        jump = Rule('jump', 'Room', 'Room:jump', [], [], [], ('transport', ('Room', 8, 8), True), [])
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        agent.index.frontier.return_value = [pickup]
        agent.destination_points = Mock(return_value=[(9, 8)])
        agent.mechanism_plan = Mock(return_value={'steps': [jump], 'flags': ['SWITCH'], 'maps': {'Room'}})
        groups = {'key': {'target': target, 'rules': [pickup]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, target)
        groups = {'switch': {'target': ('flag', 'SWITCH', False), 'rules': []}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual([g['target'] for g in groups.values()], [jump.effect])
        agent.mechanism_plan.return_value = {'steps': [], 'flags': ['SWITCH'], 'maps': {'Room'}}
        groups = {'switch': {'target': ('flag', 'SWITCH', False), 'rules': []}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual([g['target'] for g in groups.values()], [target])
        agent.index.satisfied.return_value = True
        agent.add_mechanism_groups({}, {})
        self.assertIsNone(agent.mechanism_goal)

    def test_switch_approach_respects_its_script_facing_guard(self):
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        pose = {'BinaryOp': {'op': 'Eq', 'left': {'Call': {'callee': 'getPlayerFacing', 'args': []}},
                             'right': literal('up')}}
        rule = Rule('switch', 'Room', 'Room:switch', [], [(pose, True)], [], ('flag', 'ON', True), [])
        self.assertEqual(list(agent.interaction_approaches(rule, 10, 5)), [((10, 6), 'up')])

    def test_local_mechanism_prerequisite_precedes_exit_for_outer_goal(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        outer = Rule('battle', 'Gym', 'Gym:battle', [], [], [], ('flag', 'BADGE', True), [])
        pickup = Rule('key', 'Room', 'Room:key', [], [], [], ('item', 'KEY', True), [])
        switch = Rule('switch', 'Room', 'Room:switch', [], [], [], ('flag', 'SWITCH', True), [])
        agent.index = Mock()
        agent.index.satisfied.return_value = False
        agent.mechanism_goal = outer.effect
        agent.destination_points = Mock(return_value=[(2, 3)])
        agent.mechanism_plan = lambda name, points, facts: {
            'steps': [switch] if name == 'Room' else [], 'flags': ['SWITCH'], 'maps': {'Room'},
            'exit': None if name == 'Room' else ('Outside', 1, 1)}
        groups = {'outer': {'target': outer.effect, 'rules': [outer]},
                  'key': {'target': pickup.effect, 'rules': [pickup]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, pickup.effect)
        self.assertIn(switch.effect, [g['target'] for g in groups.values()])
        self.assertFalse(any(g['rules'][0].storyline == 'skill:leave_mechanism' for g in groups.values()))

        # Once the local prerequisite is actually acquired, leaving becomes
        # useful again; the remembered item goal must not trap the agent.
        agent.index.satisfied.side_effect = lambda target, facts: target == pickup.effect
        groups = {'outer': {'target': outer.effect, 'rules': [outer]}}
        agent.add_mechanism_groups(groups, {})
        self.assertEqual(agent.mechanism_goal, outer.effect)
        self.assertTrue(any(g['rules'][0].storyline == 'skill:leave_mechanism' for g in groups.values()))

    def test_action_receives_selected_mechanism_context_and_live_reachability(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        context = {'towards': ('item', 'KEY', True),
                   'planned_effects': [('flag', 'SWITCH', False), ('flag', 'SWITCH', True)],
                   'trigger_navigation': [{'tile_route_found': True}]}
        agent.active = {'context': context}
        state = {'subgoal': ('flag', 'SWITCH', False), 'local_state': {}}
        with patch.object(DualStoryAgent, 'choose', return_value='step') as choose:
            agent.choose('action', state, {'step': '{}'}, 'Choose the next operation.')
            self.assertEqual(choose.call_args.args[1]['strategy_context'], context)
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            self.assertNotIn('strategy_context', state)
            # An unverified route still allows rejection. The instruction
            # is not a blanket requirement to execute every proposed plan.
            context['trigger_navigation'][0]['tile_route_found'] = False
            agent.choose('action', state, {'step': '{}'}, 'Choose the next operation.')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_healing_route_reports_only_live_entry_resets_of_won_battles(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [literal('STARTED')]}}
        win = Rule('win', 'Arena', 'Arena:trainer', [], [], [], ('flag', 'WON', True), [('battle', 'TRAINER', True)])
        reset = Rule('reset', 'Lobby', 'Lobby:@load', ['load'], [(guard, True)], [], ('flag', 'WON', False), [])
        lever = Rule('lever', 'Arena', 'Arena:lever', [], [], [], ('flag', 'LEVER', True), [])
        reset_lever = Rule('lever_reset', 'Lobby', 'Lobby:@load', ['load'], [], [], ('flag', 'LEVER', False), [])
        heal = Rule('heal', 'Lobby', 'Lobby:nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.index = SimpleNamespace(rules=[win, reset, lever, reset_lever])
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Hall'}, {'to_map': 'Lobby'}]}
        facts = {'map': 'Arena', 'flags': {'STARTED': True, 'WON': True, 'LEVER': True}}
        self.assertEqual(agent.healing_route_costs([heal], facts), {'Lobby': ['WON']})
        self.assertEqual(agent.healing_route_costs([heal], {**facts, 'flags': {'WON': True}}), {})
        self.assertTrue(facts['flags']['WON'])
        agent.client.route.return_value = {'found': True, 'legs': []}
        self.assertEqual(agent.healing_route_costs([heal], {**facts, 'map': 'Lobby'}), {})

    def test_coupled_doors_require_transport_before_toggling_back(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        guard = {'Call': {'callee': 'getFlag', 'args': [literal('SWITCH')]}}
        on = Rule('on', 'A', 'A:lever', ['sign:1'], [(guard, False)], [], ('flag', 'SWITCH', True), [])
        off = Rule('off', 'B', 'B:lever', ['sign:1'], [(guard, True)], [], ('flag', 'SWITCH', False), [])
        jump = Rule('jump', 'A', 'A:jump', ['coord:(3,0)'], [], [], ('transport', ('B', 1, 0), True), [])
        rules = [on, off, jump]
        for name in ['A', 'B']:
            for enabled in [False, True]:
                rules.append(Rule(name + str(enabled), name, name + ':@load', ['load'],
                    [(guard, enabled)], [], ('block', name + ',0,0', int(enabled)), []))
        agent.index = SimpleNamespace(rules=rules, by_effect={r.effect: [r] for r in rules})
        agent.game = Mock(last_map='A')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        agent.destination_points = lambda name, rule: [(3 if rule.id == 'jump' else 2, 0)]
        maps = {name: {'blocks': [0], 'width': 1} for name in ['A', 'B']}
        original = maps['A']['blocks']
        def route(name, start, dest, end, **kwargs):
            enabled = maps[name]['blocks'][0]
            if name != dest or end[0] == 3 and ((name == 'A' and not enabled) or (name == 'B' and enabled)):
                return None
            return [(name, *start)] if tuple(start) == tuple(end) else [(name, *start), ((dest, *end), 'right')]
        facts = {'map': 'A', 'x': 1, 'y': 0, 'flags': {}}
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=route):
            result = agent.mechanism_plan('B', [(3, 0)], facts)
        self.assertEqual([r.id for r in result['steps']], ['on', 'jump', 'off'])
        self.assertIs(maps['A']['blocks'], original)
        self.assertEqual(maps['B']['blocks'], [0])
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=RuntimeError('cancelled')):
            with self.assertRaises(RuntimeError):
                agent.mechanism_plan('B', [(3, 0)], facts)
        self.assertIs(maps['A']['blocks'], original)
        maps['A']['warps'] = []
        maps['B']['warps'] = [{'x': 4, 'y': 0}]
        def exit_route(name, start, dest, end, **kwargs):
            if dest == 'C':
                return [(name, *start), (('C', *end), 'right')] if name == 'B' and not maps['B']['blocks'][0] else None
            return route(name, start, dest, end, **kwargs)
        with patch.object(pt, 'MAPS', maps), patch.object(pt, 'bfs_cross', side_effect=exit_route), \
                patch.object(pt, 'warp_edges_from', return_value=[('C', 4, 0)]), \
                patch.object(pt, 'walkable', side_effect=lambda name, x, y: (x, y) == (5, 0)), \
                patch.object(pt, 'warp_tiles', return_value=set()):
            result = agent.mechanism_plan('C', [(8, 0)], {**facts, 'map': 'B', 'flags': {'SWITCH': True}})
        self.assertEqual([r.id for r in result['steps']], ['off'])
        self.assertEqual(result['exit'], ('C', 5, 0))
        self.assertIs(maps['A']['blocks'], original)

    def test_same_map_boulder_in_another_region_requires_travel(self):
        from openpokered.story_rules import MAPS_DIR
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'SeafoamIslandsB2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = Mock(maps_dir=MAPS_DIR)
        agent.index.coordinates.return_value = []
        flag = 'EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE'
        rule = Rule('boulder:' + flag, name, name + ':engine_boulder', [], [], [], ('flag', flag, True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'npc_index': 1, 'text_id': 2, 'x': 23, 'y': 6, 'visible': True}]
        agent.client.route.return_value = {'legs': []}
        agent.visited = {name}
        facts = {'map': name, 'x': 4, 'y': 3, 'party': [{'moves': ['Strength']}], 'flags': {}}
        with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
            _, bindings = agent.action_candidates(facts)
        self.assertEqual([v[0] for v in bindings.values()], ['travel_to:' + name])
        with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
            _, bindings = agent.action_candidates({**facts, 'x': 24, 'y': 6})
        self.assertEqual([v[0] for v in bindings.values()], ['push_puzzle:0,' + flag])

    def test_observed_pushback_backchains_key_without_removing_real_barriers(self):
        from types import SimpleNamespace
        from openpokered.story_rules import literal
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        condition = {'Call': {'callee': 'hasItem', 'args': [literal('KEY')]}}
        target = ('item', 'KEY', True)
        rule = Rule('guard', 'Gate', 'Gate:guard', [], [(condition, False)], [], ('movement', 'down', True), [])
        agent.index = SimpleNamespace(rules=[rule], coordinates=lambda r: [(2, 3)],
            frontier=lambda target, facts: [rule] if target == ('item', 'KEY', True) else [])
        original = {'Gate': {(2, 3), (8, 8)}}
        agent.game = SimpleNamespace(last_map='Road', script_navigation_barriers=original,
            navigation_excluded_maps=lambda: ())
        agent.navigation_facts = {'flags': {}, 'bag': {}}
        def relaxed_path(*args, **kwargs):
            if kwargs['blocked_maps'] == original:
                return None
            self.assertEqual(kwargs['blocked_maps'], {'Gate': {(8, 8)}})
            self.assertEqual(original, {'Gate': {(2, 3), (8, 8)}})
            return [(('Road', 0, 0), ''), (('Gate', 2, 3), 'up')]
        with patch.object(pt, 'bfs_cross', side_effect=relaxed_path):
            result = agent.discover_route_prerequisites(
                {'map_name': 'Road', 'player_x': 0, 'player_y': 0}, 'Room', [(1, 1)])
        self.assertEqual(result, [target])
        self.assertIs(agent.game.script_navigation_barriers, original)
        # If a route preserving the gate exists, its incidental shortcut
        # through the guard must not introduce a spurious key dependency.
        agent.maps = {'PalletTown': {'npcs': []}}
        with patch.object(pt, 'bfs_cross', return_value=[(('Road', 0, 0), ''),
                (('PalletTown', 5, 5), 'up')]) as search:
            self.assertEqual(agent.discover_route_prerequisites(
                {'map_name': 'Road', 'player_x': 0, 'player_y': 0}, 'Room', [(1, 1)]), [])
            self.assertEqual(search.call_count, 1)
            self.assertEqual(search.call_args.kwargs['blocked_maps'], original)

    def test_puzzle_walk_avoids_fall_holes_and_restores_navigation_barriers(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(script_navigation_barriers={'OtherRoom': {(1, 1)}})
        original = agent.game.script_navigation_barriers
        holes = {(3, 16), (6, 16)}
        rocks = {(3, 15), (4, 14), (9, 12), (6, 15)}
        def walk(x, y, name, **kwargs):
            blocked = {name: rocks | agent.game.script_navigation_barriers.get(name, set())}
            path = pt.bfs_cross(name, (7, 15), name, (x, y), blocked_maps=blocked, last_map='Route20')
            self.assertTrue(path)
            self.assertFalse(any(node[0] == name and tuple(node[1:]) in holes for node, _ in path[1:]))
        agent.game.nav_to_map.side_effect = walk
        agent.navigate_point('SeafoamIslandsB3F', (6, 14), avoid_tiles=holes)
        self.assertIs(agent.game.script_navigation_barriers, original)
        agent.game.nav_to_map.side_effect = pt.NavError('interrupted')
        with self.assertRaises(pt.NavError):
            agent.navigate_point('SeafoamIslandsB3F', (6, 14), avoid_tiles=holes)
        self.assertIs(agent.game.script_navigation_barriers, original)
        self.assertFalse(agent.game.navigation_active)

    def test_puzzle_approach_uses_the_stone_that_can_reach_its_hole(self):
        from openpokered.story_rules import MAPS_DIR
        from openpokered.boulder_skills import plan_pushes
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'SeafoamIslandsB2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = Mock(maps_dir=MAPS_DIR)
        agent.index.coordinates.return_value = []
        flag = 'EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE'
        rule = Rule('boulder:' + flag, name, name + ':engine_boulder', [], [], [], ('flag', flag, True), [])
        points = agent.destination_points(name, rule)
        self.assertIn((17, 6), points)
        self.assertNotIn((24, 6), points)
        self.assertNotIn((19, 6), points)  # A hole is not a safe approach tile.
        self.assertIsNone(plan_pushes(name, (24, 6), [(18, 6)], [], (19, 6)))
        self.assertTrue(plan_pushes(name, (17, 6), [(18, 6)], [], (19, 6)))

    def test_travel_chooses_reachable_side_of_key_with_live_trainer_collision(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='SaffronCity')
        agent.game.st.return_value = {'map_name': 'SilphCo5F', 'player_x': 26, 'player_y': 1}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = {(28, 4), (21, 16), (13, 9), (8, 16), (8, 3),
            (18, 10), (2, 13), (4, 6), (22, 12), (25, 10), (24, 6)}
        agent.index = object()
        agent.facts = Mock(return_value={})
        agent.observed_navigation_barriers = Mock(return_value={})
        agent.navigate_point = Mock()
        rule = Rule('key', 'SilphCo5F', 'SilphCo5F:itemCardKey', [], [], [], ('item', 'CARD_KEY', True), [])
        result = agent.travel('SilphCo5F', rule, [(22, 16), (20, 16)])
        self.assertEqual(result['position'], (20, 16))
        agent.navigate_point.assert_called_once_with('SilphCo5F', (20, 16))

    def test_navigation_dependencies_expand_recursively_but_ignore_unrelated_memories(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        main = ('flag', 'BOSS', True)
        prerequisite = ('flag', 'RESCUE', True)
        pickup = ('visibility', 'BALL', False)
        rescue = Rule('rescue', 'Tower', 'Tower:rescue', [], [], [], prerequisite, [])
        clear = Rule('clear', 'Tower', 'Tower:pickup', [], [], [], pickup, [])
        agent.index = SimpleNamespace(rules=[], npc_toggles={('Gate', 1): ('SLEEPER', False),
            ('Tower', 2): ('BALL', True), ('OldRoom', 3): ('OLD', True)},
            satisfied=lambda target, facts: target[1] in facts.get('completed', []),
            frontier=lambda target, facts: {('visibility', 'SLEEPER', False): [rescue],
                pickup: [clear]}.get(target, []))
        agent.maps, agent.field_requirements, agent.navigation_memory = {}, {}, {}
        agent.navigation_blockage = None
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'Road'}, {'to_map': 'End'}]}
        # The dependent memory comes first, before its parent is discovered.
        agent.navigation_history = {
            'child': {'map': 'Tower', 'destination': 'Tower', 'goal': prerequisite, 'blocking_npcs': [2]},
            'parent': {'map': 'Gate', 'destination': 'End', 'goal': main, 'blocking_npcs': [1]},
            'stale': {'map': 'OldRoom', 'destination': 'OldRoom', 'goal': ('flag', 'OLD_DETOUR', True), 'blocking_npcs': [3]}}
        groups = {'main': {'target': main, 'rules': [], 'objectives': []}}
        # Topology omits the gatehouse; the actual map warp includes it.
        with patch.object(pt, 'MAPS', {'Road': {'warps': [{'dest_map_name': 'Gate'}]}}):
            agent.add_navigation_groups(groups, {'map': 'City'})
        self.assertEqual({tuple(g['target']) for g in groups.values()}, {main, prerequisite, pickup})
        completed = {'main': {'target': main, 'rules': [], 'objectives': []}}
        agent.add_navigation_groups(completed, {'map': 'City', 'completed': ['BOSS']})
        self.assertEqual(list(completed), ['main'])

    def test_reachable_frontier_precedes_a_remote_npc_detour(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='City')
        agent.game.st.return_value = {'map_name': 'City', 'player_x': 3, 'player_y': 3}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.index = None
        agent.client = Mock()
        agent.client.route.return_value = {'legs': [{'to_map': 'Frontier'}, {'to_map': 'Tower'}]}
        agent.destination_points = Mock(return_value=[(3, 4)])
        agent.remembered_route_blocker = Mock(return_value={'result': 'blocked',
            'blockage_map': 'Detour', 'blocking_npcs': [1]})
        agent.discover_route_prerequisites = Mock(return_value=[])
        agent.navigate_point = Mock()
        rule = Rule('goal', 'Tower', 'Tower:goal', [], [], [], ('flag', 'DONE', True), [])
        def path(source, position, destination, target, **kwargs):
            return [(('Frontier', 3, 4), 'up')] if destination == 'Frontier' else None
        with patch.object(pt, 'bfs_cross', side_effect=path), patch(
                'openpokered.autonomous_story.cut_requirement', return_value=None), patch(
                'openpokered.autonomous_story.surf_requirement', return_value=None):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['stage'], 'Frontier')
        agent.navigate_point.assert_called_once_with('Frontier', (3, 4), tries=50)

    def test_remote_npc_does_not_hide_an_alternative_cut_route(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock(last_map='City')
        agent.game.st.return_value = {'map_name': 'City', 'player_x': 3, 'player_y': 3}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.index = None
        agent.field_requirements = {}
        agent.remembered_route_blocker = Mock(return_value={'result': 'blocked',
            'blockage_map': 'Road', 'blocking_npcs': [1]})
        tree = {'move': 'Cut', 'map': 'Detour', 'tree': [3, 4], 'stance': [3, 3]}
        rule = Rule('goal', 'Tower', 'Tower:goal', [], [], [], ('flag', 'DONE', True), [])
        with patch.object(pt, 'bfs_cross', return_value=None), patch(
                'openpokered.autonomous_story.cut_requirement', return_value=tree):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['field_obstruction'], tree)
        self.assertEqual(result['blocking_npcs'], [1])
        self.assertEqual(agent.field_requirements['Cut'], tree)
        # A later NPC can also prevent the complete relaxed-tree route.
        # The same tree must still be discoverable toward an earlier region.
        agent.client = Mock()
        agent.client.route.return_value = {'legs': [{'to_map': 'Frontier'}, {'to_map': 'Tower'}]}
        agent.destination_points = Mock(return_value=[(3, 4)])
        with patch.object(pt, 'bfs_cross', return_value=None), patch(
                'openpokered.autonomous_story.cut_requirement', side_effect=[None, tree]):
            result = agent.travel('Tower', rule, [(5, 5)])
        self.assertEqual(result['field_obstruction']['frontier'], 'Frontier')
        self.assertEqual(result['field_obstruction']['tree'], [3, 4])

    def test_remote_trainer_positions_expire_while_story_guards_remain(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.script_navigation_barriers = {}
        game._prev_map = 'Room'
        game.stationary_npcs = {'Hall': {1: (2, 3), 2: (4, 5)}}
        game.judgments = SimpleNamespace(
            index=SimpleNamespace(npc_toggles={('Hall', 2): ('GUARD', False)}),
            navigation_facts={'flags': {}},
            maps={'Hall': {'npcs': [{'textId': 1, 'isTrainer': True}, {'textId': 2}]}})
        self.assertEqual(game.navigation_barriers(), {'Hall': {(4, 5)}})

    def test_field_move_approach_reports_its_travel_blockage_before_using_move(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        agent.active = {'context': {'map': 'Route14', 'stance': [4, 42]}}
        agent.travel = Mock(return_value={'result': 'blocked', 'detail': 'A guard blocks the approach'})
        agent.remember_travel_result, agent.record, agent.game = Mock(), Mock(), Mock()
        rule = Rule('cut', 'Route14', 'skill:field', [], [], [], (), [])
        result = agent.execute('cut:0', rule)
        self.assertEqual(result['result'], 'blocked')
        agent.remember_travel_result.assert_called_once_with('Route14', result)
        agent.game.face.assert_not_called()

    def test_remote_observed_npc_blockage_keeps_its_map_in_recovery_memory(self):
        from types import SimpleNamespace
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = SimpleNamespace(last_map='CeruleanCity', script_navigation_barriers={},
            stationary_npcs={'CeruleanCity': {11: (27, 12)}})
        agent.maps = {'CeruleanCity': {'npcs': [{'textId': 11, 'isTrainer': False}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = []
        agent.client.route.return_value = {'found': True, 'legs': []}
        state = {'map_name': 'CeruleanPokecenter', 'player_x': 3, 'player_y': 3}
        agent.client.state.return_value = state
        def path(*args, **kwargs):
            if (27, 12) in kwargs['blocked_maps']['CeruleanCity']:
                return None
            return [(('CeruleanPokecenter', 3, 3), None),
                    (('CeruleanCity', 27, 12), 'up'), (('Route5', 10, 13), 'down')]
        with patch.object(pt, 'bfs_cross', side_effect=path):
            result = agent.remembered_route_blocker(state, 'Route5', [(10, 13)],
                {'CeruleanCity': {(27, 12)}}, ())
        self.assertEqual(result['blocking_npcs'], [11])
        self.assertEqual(result['blockage_map'], 'CeruleanCity')
        agent.active = {'target': ('item', 'HM01', True)}
        agent.navigation_memory, agent.navigation_history = {}, {}
        agent.observed_barrier_maps = set()
        agent.remember_travel_result('Route5', result)
        self.assertEqual(agent.navigation_memory['Route5']['map'], 'CeruleanCity')
        self.assertEqual(agent.navigation_memory['Route5']['position'], [27, 12])
        with patch.object(pt, 'bfs_cross', return_value=[(('Route5', 10, 13), None)]):
            self.assertIsNone(agent.remembered_route_blocker(state, 'Route5', [(10, 13)],
                {'CeruleanCity': {(27, 12)}}, ()))

    def test_training_action_ranks_grounded_sites_without_redeciding_strategy(self):
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        state = {'subgoal': ['level', 'leader', 26], 'local_state': {'party': [
            {'hp': 70, 'max_hp': 70, 'status': 'None', 'moves': ['Scratch'], 'pp': [35]}]}}
        candidates = {'a': json.dumps({'navigation': {'tile_route_found': True}})}
        with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
            self.assertEqual(agent.choose('action', state, candidates, 'choose'), 'a')
            self.assertFalse(choose.call_args.kwargs['allow_abstain'])
            agent.choose('strategy', state, candidates, 'choose')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])
            state['local_state']['party'][0]['pp'] = [0]
            agent.choose('action', state, candidates, 'choose')
            self.assertTrue(choose.call_args.kwargs['allow_abstain'])

    def test_training_is_not_offered_when_the_skill_would_stop_for_recovery(self):
        from openpokered.story_agent import DualStoryAgent
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [{'id': 'brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 17
        healer = Rule('heal', 'PewterPokecenter', 'nurse', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.nearby_healers = Mock(return_value=[healer])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_training_sites = Mock(return_value={'Route2': (7, 7)})
        agent.find_catch_areas = Mock(return_value={})
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={}):
            groups = agent.strategy_groups(facts)
            self.assertIn('prepare:heal', groups)
            self.assertNotIn('prepare:train', groups)
            mon['pp'][0] = 35
            self.assertIn('prepare:train', agent.strategy_groups(facts))

    def test_real_object_approach_propagates_battle_pause_without_retrying(self):
        import playthrough as pt
        from openpokered.playthrough_judgments import NavigationPause
        game = JevGame.__new__(JevGame)
        game.pos = Mock(return_value=('CeruleanGym', 7, 10))
        game.live_npcs = Mock(return_value=set())
        game.nav_to = Mock(side_effect=NavigationPause('blackout moved the player'))
        game.face = Mock()
        with patch.object(pt, 'bfs', return_value=[(7, 10), (7, 9)]):
            with self.assertRaises(NavigationPause):
                game.approach_object(4, 2, 'CeruleanGym')
        self.assertEqual(game.nav_to.call_count, 1)
        game.face.assert_not_called()

    def test_pp_recovery_goal_matches_useful_owned_medicine(self):
        from openpokered.story_rules import StoryIndex
        index = StoryIndex.__new__(StoryIndex)
        mon = {'hp': 70, 'max_hp': 70, 'moves': ['Dig', 'Growl'], 'pp': [5, 0]}
        facts = {'party': [mon]}
        target = ('pp_reserve', 'party', True)
        self.assertFalse(index.satisfied(target, facts))
        self.assertEqual([item for item, _, _ in medicine_options([mon], {'Elixer': 1, 'Ether': 1})], ['Elixer'])
        mon['pp'][0] = 10
        self.assertTrue(index.satisfied(target, facts))
        self.assertEqual(list(medicine_options([mon], {'Elixer': 1})), [])

    def test_route_relaxation_discovers_boulders_without_leaving_doors_open(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'VictoryRoad2F'
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        rule = Rule('door', name, name + ':@load', ['load'], [], [], ('block', name + ',3,4', 21), [])
        agent.index = SimpleNamespace(rules=[rule], frontier=Mock(return_value=[rule]))
        agent.game = SimpleNamespace(last_map='Route23', script_navigation_barriers={}, navigation_excluded_maps=lambda: ())
        agent.navigation_facts = {'flags': {}}
        state = {'map_name': name, 'player_x': 0, 'player_y': 8}
        original = list(pt.MAPS[name]['blocks'])
        targets = agent.discover_route_prerequisites(state, name, [(10, 5)])
        self.assertIn(('flag', 'EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1', True), targets)
        self.assertEqual(pt.MAPS[name]['blocks'], original)
        with patch.object(pt, 'bfs_cross', side_effect=RuntimeError('interrupted search')):
            with self.assertRaises(RuntimeError):
                agent.discover_route_prerequisites(state, name, [(10, 5)])
        self.assertEqual(pt.MAPS[name]['blocks'], original)

    def test_training_can_consider_unvisited_grass_on_the_explored_frontier(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        agent.visited = {'PewterCity', 'Route1', 'Route2', 'ViridianCity', 'ViridianForest'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='PewterCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        sites = agent.find_training_sites({'map': 'PewterCity', 'x': 18, 'y': 18, 'party': [{'level': 16}]})
        self.assertIn('Route3', sites)
        self.assertNotIn('Route3', agent.visited)
        self.assertNotIn('VictoryRoad1F', sites)

    def test_training_route_checks_current_npcs_and_keeps_reachable_alternative(self):
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        agent.visited = {'CeruleanCity'}
        agent.navigation_memory = {'Route5': {}, 'Route9': {}}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='CeruleanCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = {(27, 12)}
        agent.game.navigation_excluded_maps.return_value = ()
        def path(start, position, destination, target, **kwargs):
            self.assertIn((27, 12), kwargs['blocked_maps']['CeruleanCity'])
            if destination == 'Route24':
                return [(('CeruleanCity', *position), None), (('Route24', 5, 18), 'up')]
            return None
        with patch.object(pt, 'bfs_cross', side_effect=path):
            sites = agent.find_training_sites({'map': 'CeruleanCity', 'x': 19, 'y': 18, 'party': [{'level': 23}]})
        self.assertNotIn('Route5', sites)
        self.assertNotIn('Route9', sites)
        self.assertEqual(sites['Route24'], (5, 18))
        agent.active = {'target': ('level', 'leader', 25), 'rules': [
            Rule('train', 'Route24', 'skill:train_encounter', [], [], [], ('level', 'leader', 25), [])]}
        candidates, _ = agent.action_candidates({})
        self.assertTrue(json.loads(candidates['action:0'])['navigation']['tile_route_found'])

    def test_training_site_can_be_the_current_cave_floor(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'VictoryRoad2F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.visited = {name}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': []}
        agent.game = Mock(last_map='Route23')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        sites = agent.find_training_sites({'map': name, 'x': 0, 'y': 7, 'party': [{'level': 61}]})
        self.assertEqual(sites[name], (0, 7))
        self.assertEqual(agent.training_navigation[name]['steps'], 0)

    def test_catch_areas_skip_registered_tables_and_record_their_navigation(self):
        from openpokered.story_rules import MAPS_DIR
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('ViridianCity', 'Route1', 'Route2')}
        agent.visited = {'ViridianCity'}
        agent.navigation_memory = {}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='ViridianCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        owned = {mon['species'] for mon in agent.maps['Route1']['wild']['red']['grass']['mons']}
        self.assertEqual(owned, {'Pidgey', 'Rattata'})
        path = [(('ViridianCity', 19, 18), None), (('Route2', 5, 17), 'up'), (('Route2', 5, 18), 'up')]
        with patch.object(pt, 'bfs_cross', return_value=path) as search:
            areas = agent.find_catch_areas({'map': 'ViridianCity', 'x': 19, 'y': 18,
                                            'dex': {'owned_species': sorted(owned)}})
        # Route1 holds no unregistered species, so it is never even searched.
        self.assertEqual(search.call_args.args[2], 'Route2')
        self.assertEqual(list(areas), ['Route2'])
        self.assertEqual(areas['Route2']['species'], ['Weedle'])
        self.assertEqual(areas['Route2']['spots'], [(5, 18)])
        self.assertTrue(areas['Route2']['reachable'])
        self.assertEqual(list(agent.catch_navigation), ['Route2'])
        self.assertTrue(agent.catch_navigation['Route2']['tile_route_found'])
        self.assertEqual(agent.catch_navigation['Route2']['steps'], 2)

    def ranked_catch_agent(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3')}
        agent.visited = {'PewterCity'}
        agent.navigation_memory = {}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [0]}
        agent.game = Mock(last_map='PewterCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        return agent

    def test_catch_areas_expand_past_an_exhausted_neighbourhood(self):
        import playthrough as pt
        from openpokered.story_rules import MAPS_DIR
        # Everything one hop from PewterCity is Route2/Route3 grass; owning
        # those species leaves nothing there, and only a wider ring helps.
        exhausted = ['Pidgey', 'Rattata', 'Weedle', 'Jigglypuff', 'Spearow']
        facts = {'map': 'PewterCity', 'x': 14, 'y': 7, 'dex': {'owned_species': exhausted}}
        with patch.object(pt, 'bfs_cross', return_value=[(('PewterCity', 14, 7), None)]):
            near_only = self.ranked_catch_agent().find_catch_areas(dict(facts))
        self.assertEqual(near_only, {})  # Control: the old one-hop search found nothing.
        agent = self.ranked_catch_agent()
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3', 'Route4')}
        with patch.object(pt, 'bfs_cross', return_value=[(('PewterCity', 14, 7), None)]):
            expanded = agent.find_catch_areas(dict(facts))
        self.assertIn('Route4', expanded)
        self.assertEqual(expanded['Route4']['species'], ['Ekans'])

    def test_forest_and_safari_interiors_are_both_huntable(self):
        import playthrough as pt
        from openpokered.story_rules import MAPS_DIR
        agent = self.ranked_catch_agent()
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())
                      for name in ('PewterCity', 'Route2', 'Route3', 'ViridianForest',
                                   'SafariZoneCenter')}
        agent.visited = {'ViridianForest', 'SafariZoneCenter'}
        facts = {'map': 'ViridianForest', 'x': 16, 'y': 43, 'dex': {'owned_species': []}}
        with patch.object(pt, 'bfs_cross', return_value=[(('ViridianForest', 16, 43), None)]):
            areas = agent.find_catch_areas(dict(facts))
        self.assertIn('ViridianForest', areas)
        self.assertEqual(areas['ViridianForest']['species'],
                         ['Caterpie', 'Kakuna', 'Metapod', 'Pikachu', 'Weedle'])
        safari = areas['safari:SafariZoneCenter']
        self.assertEqual(safari['method'], 'safari')
        self.assertIn('Scyther', safari['species'])

    def test_catch_ranking_prefers_more_unregistered_species_at_equal_distance(self):
        import playthrough as pt
        agent = self.ranked_catch_agent()
        equal_paths = [(('PewterCity', 14, 7), None), (('Route2', 5, 18), 'up')]
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            areas = agent.find_catch_areas({'map': 'PewterCity', 'x': 14, 'y': 7,
                                            'dex': {'owned_species': ['Pidgey', 'Rattata']}})
        # Route3 still holds two unregistered species where Route2 holds one,
        # so the richer table outranks the closer name at the same distance.
        self.assertEqual(list(areas), ['Route3', 'Route2'])
        self.assertEqual(areas['Route2']['species'], ['Weedle'])

    def test_catch_ranking_demotes_an_area_with_unproductive_recent_hunts(self):
        import playthrough as pt
        agent = self.ranked_catch_agent()
        agent.catch_attempts = [{'map': 'Route2', 'registered': False},
                                {'map': 'Route2', 'registered': False},
                                {'map': 'Route3', 'registered': True}]
        equal_paths = [(('PewterCity', 14, 7), None), (('Route2', 5, 18), 'up')]
        facts = {'map': 'PewterCity', 'x': 14, 'y': 7, 'dex': {'owned_species': []}}
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            self.assertEqual(list(agent.find_catch_areas(facts)), ['Route3', 'Route2'])
        # With no history at all the tie falls back to the map name, matching
        # the old distance-only order.
        with patch.object(pt, 'bfs_cross', return_value=equal_paths):
            self.assertEqual(list(self.ranked_catch_agent().find_catch_areas(facts)),
                             ['Route2', 'Route3'])

    def test_encounter_value_uses_real_slot_weights_and_step_rate(self):
        table = {'wild': {'red': {'grass': {'encounterRate': 25, 'mons': [
            {'level': 3, 'species': 'Common'}, {'level': 4, 'species': 'Common'},
            {'level': 5, 'species': 'Common'}, {'level': 6, 'species': 'Common'},
            {'level': 7, 'species': 'Common'}, {'level': 8, 'species': 'Common'},
            {'level': 9, 'species': 'Common'}, {'level': 10, 'species': 'Common'},
            {'level': 11, 'species': 'Common'}, {'level': 12, 'species': 'Pidgey'},
        ]}}}}
        value = encounter_value(table, {'Common'})
        target = value['targets'][0]
        self.assertEqual(target['species'], 'Pidgey')
        self.assertEqual(target['encounter_share_pct'], 1.2)  # threshold 253..255 = 3/256
        self.assertEqual(target['levels'], [12, 12])
        self.assertEqual(value['new_species_per_step_pct'], 0.11)
        self.assertEqual(value['expected_steps_to_any_new_species'], 873.8)

    def test_unified_red_acquisition_graph_contains_86_wild_species(self):
        from openpokered.collection_planner import acquisition_graph, SUPER_ROD_MAP_GROUP
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        self.assertEqual(len(graph), 86)
        self.assertEqual({row['method'] for row in graph['Scyther']}, {'safari'})
        self.assertTrue(any(row['method'] == 'water' for row in graph['Tentacool']))
        self.assertTrue(any(row.get('rod') == 'SuperRod' for row in graph['Dratini']))
        self.assertEqual({row['method'] for row in graph['Magikarp']}, {'fishing'})

    def test_complete_red_solo_graph_proves_the_124_species_ceiling(self):
        from openpokered.collection_planner import (complete_acquisition_graph,
            solo_plan, SUPER_ROD_MAP_GROUP)
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        plan = solo_plan(graph)
        self.assertEqual(len(graph), 151)
        self.assertEqual(plan['ceiling'], 124)
        self.assertEqual(len(plan['unreachable_species']), 27)
        self.assertEqual({'Alakazam', 'Gengar', 'Golem', 'Machamp'} &
                         set(plan['unreachable_species']),
                         {'Alakazam', 'Gengar', 'Golem', 'Machamp'})
        self.assertIn('Mew', plan['unreachable_species'])
        self.assertEqual(len({'Bulbasaur', 'Charmander', 'Squirtle'} &
                             set(plan['reachable_species'])), 1)
        self.assertEqual(plan['optimal_assignment_count'], 36)
        self.assertEqual(plan['optimal_choices']['starter'],
                         ['Bulbasaur', 'Charmander', 'Squirtle'])
        self.assertEqual(plan['optimal_choices']['fossil'], ['Kabuto', 'Omanyte'])
        self.assertEqual(plan['optimal_choices']['dojo'], ['Hitmonchan', 'Hitmonlee'])
        self.assertEqual(plan['optimal_choices']['eevee_evolution'],
                         ['Flareon', 'Jolteon', 'Vaporeon'])
        self.assertEqual(len(plan['choice_reachable_species']), 135)
        self.assertEqual(len(plan['always_unreachable_species']), 16)

    def test_complete_graph_keeps_exact_solo_choice_and_external_trade_reasons(self):
        from openpokered.collection_planner import (complete_acquisition_graph,
            solo_plan, SUPER_ROD_MAP_GROUP)
        from openpokered.story_rules import MAPS_DIR
        maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
        forced = {'starter': 'Bulbasaur', 'fossil': 'Kabuto',
                  'dojo': 'Hitmonchan', 'eevee_evolution': 'Jolteon'}
        plan = solo_plan(graph, forced_choices=forced)
        self.assertEqual(plan['ceiling'], 124)
        self.assertEqual(plan['choices'], forced)
        self.assertEqual(plan['optimal_assignment_count'], 1)
        self.assertEqual(plan['optimal_choices'], {
            'dojo': ['Hitmonchan'], 'eevee_evolution': ['Jolteon'],
            'fossil': ['Kabuto'], 'starter': ['Bulbasaur'],
        })
        self.assertTrue(any(row.get('external_trade') for row in graph['Alakazam']))
        self.assertTrue(any(row.get('source_version') == 'blue' for row in graph['Vulpix']))
        self.assertEqual(next(row['coins'] for row in graph['Porygon']
                              if row['method'] == 'prize'), 9999)
        self.assertFalse(any(row.get('storyline') == 'coordGhostMarowak'
                             for row in graph['Marowak']))

    def test_acquisition_contract_exposes_code_owned_conditions_and_costs(self):
        from openpokered.collection_planner import acquisition_contract
        prize = acquisition_contract('Porygon', {
            'method': 'prize', 'map': 'GameCornerPrizeRoom', 'coins': 9999})
        self.assertIn({'kind': 'visit_map', 'value': 'GameCornerPrizeRoom'},
                      prize['requirements'])
        self.assertIn({'kind': 'coin_case', 'value': True}, prize['requirements'])
        self.assertEqual(prize['direct_cost']['coins'], 9999)

        evolution = acquisition_contract('Arcanine', {
            'method': 'evolution', 'from_species': 'Growlithe',
            'trigger': 'item', 'item': 'FireStone'})
        self.assertIn({'kind': 'owned_species', 'value': 'Growlithe'},
                      evolution['requirements'])
        self.assertEqual(evolution['direct_cost']['consumed_items'], {'FireStone': 1})

        choice = acquisition_contract('Bulbasaur', {
            'method': 'gift', 'map': 'OaksLab', 'exclusive_group': 'starter',
            'choice': 'Bulbasaur'})
        self.assertEqual(choice['direct_cost']['irreversible_choice'],
                         {'group': 'starter', 'choice': 'Bulbasaur'})

    def test_nonwild_collection_groups_offer_gift_and_item_evolution(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        gift = Rule('gift', 'House', 'House:talkGift', [], [], [],
                    ('pokemon', 'LAPRAS', 15), [])
        agent.index = Mock(rules=[gift], by_effect={})
        agent._complete_collection_graph = {
            'Growlithe': [{'method': 'grass', 'map': 'Route8'}],
            'Arcanine': [{'method': 'evolution', 'from_species': 'Growlithe',
                          'trigger': 'item', 'item': 'FireStone'}],
            'Lapras': [{'method': 'gift', 'map': 'House', 'storyline': 'talkGift',
                        'level': 15}],
        }
        facts = {'party': [{'species': 'Growlithe', 'level': 20}], 'stored_pokemon': [],
                 'bag': {'FIRESTONE': 1}, 'flags': {}, 'coins': 0, 'money': 0,
                 'map': 'Route8', 'dex': {'owned_species': ['Growlithe']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual(groups['register:Arcanine:evolution:Growlithe']['target'],
                         ('register', 'Arcanine', True))
        self.assertEqual(groups['register:Lapras:gift:House']['rules'], [gift])

    def test_uncommitted_solo_choices_offer_every_ceiling_preserving_branch(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        gifts = [
            Rule(name, 'Lab', f'Lab:{name}', [], [], [], ('pokemon', name, 5), [])
            for name in ('Bulbasaur', 'Charmander', 'Squirtle')
        ]
        agent.index = Mock(rules=gifts, by_effect={})
        agent._complete_collection_graph = {
            name: [{'method': 'gift', 'map': 'Lab', 'storyline': name, 'level': 5,
                    'exclusive_group': 'starter', 'choice': name}]
            for name in ('Bulbasaur', 'Charmander', 'Squirtle')
        }
        facts = {'party': [], 'stored_pokemon': [], 'bag': {}, 'flags': {},
                 'coins': 0, 'money': 0, 'map': 'Lab', 'dex': {'owned_species': []}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual({group['context']['species'] for group in groups.values()},
                         {'Bulbasaur', 'Charmander', 'Squirtle'})

    def test_boxed_evolution_source_backchains_through_real_pc_rule(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = True
        pc = Rule('pc', 'Center', 'Center:pcStorage', ['sign:1'], [], [],
                  ('pc', 'storage', True), [])
        agent.index = Mock(rules=[pc], by_effect={('pc', 'storage', True): [pc]})
        agent._complete_collection_graph = {
            'Charmander': [{'method': 'gift', 'map': 'Lab', 'storyline': 'starter'}],
            'Charmeleon': [{'method': 'evolution', 'from_species': 'Charmander',
                            'trigger': 'level', 'level': 16}],
        }
        facts = {'party': [{'species': 'Pidgey', 'level': 8}],
                 'stored_pokemon': [{'box': 2, 'index': 4, 'species': 'Charmander', 'level': 15}],
                 'bag': {}, 'flags': {}, 'coins': 0, 'money': 0, 'map': 'Route1',
                 'dex': {'owned_species': ['Charmander']}}
        groups = {}
        agent.add_nonwild_collection_groups(groups, facts)
        self.assertEqual(groups['retrieve:Charmander']['target'], ('pokemon', 'Charmander', None))
        self.assertEqual(groups['retrieve:Charmander']['context']['stored_pokemon']['box'], 2)

    def test_story_semantics_compile_pc_and_coin_sources(self):
        from openpokered.story_rules import compile_story
        story = {'id': 'Room:test', 'map': 'Room', 'triggers': ['sign:1'], 'program': [
            {'Command': {'name': 'openPC', 'args': []}},
            {'Command': {'name': 'giveCoins', 'args': [{'NumberLit': 50}]}}
        ]}
        effects = {rule.effect for rule in compile_story(story)}
        self.assertIn(('pc', 'storage', True), effects)
        self.assertIn(('coins', 50, True), effects)

    def test_fishing_profiles_include_no_bite_and_uniform_group_odds(self):
        from openpokered.collection_planner import fishing_profile
        old = fishing_profile('OldRod', 'PalletTown')
        self.assertEqual((old['bite_probability_pct'], old['targets'][0]['per_attempt_pct']),
                         (100.0, 100.0))
        super_rod = fishing_profile('SuperRod', 'SafariZoneCenter')
        self.assertEqual(super_rod['bite_probability_pct'], 50.0)
        self.assertEqual({row['per_attempt_pct'] for row in super_rod['targets']}, {12.5})

    def test_collection_resources_expose_ball_quality_and_safe_status_support(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        resources = agent.collection_resources({
            'bag': {'POKEBALL': 3, 'ULTRABALL': 2, 'POTION': 4},
            'party': [{'species': 'Butterfree', 'moves': ['SleepPowder', 'Poisonpowder'],
                       'pp': [7, 12]}],
        })
        self.assertEqual(resources['total_balls'], 5)
        self.assertEqual(resources['ball_inventory'], [
            {'ball': 'PokeBall', 'quantity': 3, 'quality': 'basic'},
            {'ball': 'UltraBall', 'quantity': 2, 'quality': 'strong'},
        ])
        self.assertTrue(resources['can_apply_safe_capture_status'])
        self.assertEqual([(row['move'], row['capture_bonus'], row['residual_damage_risk'])
                          for row in resources['capture_status_moves']],
                         [('SleepPowder', 'strong', False),
                          ('Poisonpowder', 'moderate', True)])

    def test_catch_target_offers_encounter_terrain_without_training_sites(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('catch:Route24', 'Route24', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route24', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.catch_areas = {'Route24': {'species': ['Abra'], 'spots': [(5, 18)], 'reachable': True}}
        agent.catch_navigation = {'Route24': {'map': 'Route24', 'tile_route_found': True, 'steps': 2,
                                             'scope': 'path to actual encounter terrain'}}
        agent.maps = {'Route24': {'wild': {'red': {'grass': {'mons': [{'level': 8, 'species': 'Abra'}]}}}}}
        candidates, bindings = agent.action_candidates({})
        self.assertEqual(json.loads(candidates['action:0'])['operation'], 'catch_encounter:Route24,5,18')
        self.assertEqual(json.loads(candidates['action:0'])['unregistered_species_here'], ['Abra'])
        self.assertEqual(bindings['action:0'], ('catch_encounter:Route24,5,18', rule))
        # The level fallthrough would raise here; a catch target has no training site.
        self.assertFalse(hasattr(agent, 'training_sites'))

    def test_catch_encounter_reports_the_dex_delta_of_whatever_battle_started(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('catch:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.actions, agent.max_actions = 0, 100
        before = {'map_name': 'Route2', 'player_x': 5, 'player_y': 18, 'screen': 'overworld',
                  'pokedex': {'owned': 2}, 'party': [{'level': 12}]}
        battle = {**before, 'screen': 'battle', 'pokedex': {'owned': 3}}
        agent.client = Mock()
        agent.client.state.side_effect = [before, before, before] + [battle] * 8
        agent.client.cmd.return_value = []
        agent.client.move_to.return_value = {'result': 'reached'}
        agent.game = Mock()
        agent.game.st.return_value = {'player_x': 5, 'player_y': 18}
        agent.check_budget, agent.settle, agent.record = Mock(), Mock(), Mock()
        agent.record_travel = Mock()
        with patch('openpokered.autonomous_story.reachable_grass', return_value=(5, 18)):
            result = agent.execute('catch_encounter:Route2,5,18', rule)
        self.assertEqual(result, {'result': 'hunted', 'map': 'Route2', 'owned_before': 2, 'owned_after': 3})
        self.assertEqual(agent.record.call_args.kwargs['operation'], 'catch_encounter:Route2,5,18')
        self.assertEqual(agent.record.call_args.kwargs['result'], result)
        self.assertEqual(agent.catch_attempts, [{'map': 'Route2', 'registered': True}])

    def travel_goal_agent(self, legs, origin='ViridianCity'):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = None
        agent.game = Mock(last_map=origin)
        agent.game.st.return_value = {'map_name': origin, 'player_x': 1, 'player_y': 1}
        agent.game.navigation_excluded_maps.return_value = ()
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': legs}
        agent.navigate_point = Mock()
        return agent

    def test_catch_trip_blocks_grass_on_the_maps_it_only_crosses(self):
        import playthrough as pt
        agent = self.travel_goal_agent([{'to_map': 'Route2'}, {'to_map': 'Route22'}])
        rule = Rule('catch:Route22', 'Route22', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route22', True), [])
        searches = []
        def search(source, start, goal_map, goal, **kwargs):
            searches.append(kwargs['blocked_maps'])
            return [('ViridianCity', 1, 1), ('Route22', *goal)] if len(searches) > 1 else None
        grass = {'ViridianCity': {(0, 0)}, 'Route2': {(5, 5)}, 'Route22': {(9, 9)}}
        with patch.object(pt, 'bfs_cross', side_effect=search), \
                patch.object(pt, 'grass_tiles', side_effect=lambda name: grass[name]) as tiles:
            result = agent.travel('Route22', rule, [(3, 3)], avoid_encounters=True)
        self.assertEqual(result['result'], 'reached')
        self.assertEqual(searches[0], {'ViridianCity': {(0, 0)}, 'Route2': {(5, 5)}})
        # Extra blocked tiles can make a route infeasible: the trip still plans.
        self.assertEqual(searches[1], {})
        # The destination's own grass is the point of the trip; never queried.
        self.assertEqual([row.args[0] for row in tiles.call_args_list], ['Route2', 'ViridianCity'])
        # The walk re-plans from every observation, so it is handed the transit
        # maps as navigation barriers — without the map it starts on.
        self.assertEqual(agent.navigate_point.call_args.args, ('Route22', (3, 3)))
        self.assertEqual(agent.navigate_point.call_args.kwargs['avoid_maps'], {'Route2': {(5, 5)}})

    def test_ordinary_travel_never_blocks_encounter_terrain(self):
        import playthrough as pt
        agent = self.travel_goal_agent([{'to_map': 'Route22'}])
        rule = Rule('stop', 'Route22', 'Route22:trigger', [], [], [], ('flag', 'DONE', True), [])
        with patch.object(pt, 'bfs_cross', return_value=[('ViridianCity', 1, 1), ('Route22', 2, 2)]) as search, \
                patch.object(pt, 'grass_tiles') as tiles:
            self.assertEqual(agent.travel('Route22', rule, [(2, 2)])['result'], 'reached')
        self.assertEqual(search.call_count, 1)
        self.assertEqual(search.call_args.kwargs['blocked_maps'], {})
        tiles.assert_not_called()
        agent.navigate_point.assert_called_once_with('Route22', (2, 2))

    def test_only_a_catch_trip_avoids_encounters_on_the_way(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 100
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'Route2', 'player_x': 5, 'player_y': 18}
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'PewterCity', 'party': [{'level': 12}],
                                           'pokedex': {'owned': 2}}
        agent.travel = Mock(return_value={'result': 'blocked'})
        agent.record, agent.record_travel, agent.settle, agent.remember_travel_result = (
            Mock(), Mock(), Mock(), Mock())
        rule = Rule('trip:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        agent.active = {'target': ('level', 'leader', 14), 'rules': [rule]}
        agent.execute('train_encounter:Route2,5,18', rule)
        self.assertFalse(agent.travel.call_args.kwargs['avoid_encounters'])
        agent.active = {'target': ('catch', 'Route2', True), 'rules': [rule]}
        agent.execute('catch_encounter:Route2,5,18', rule)
        self.assertTrue(agent.travel.call_args.kwargs['avoid_encounters'])

    def test_catch_area_context_states_quality_registration_and_attempts(self):
        from openpokered.story_agent import DualStoryAgent
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        agent.find_catch_areas = Mock(return_value={'Route2': {'species': ['Chansey', 'Zubat'],
                                                              'spots': [(5, 18)], 'reachable': True}})
        agent.catch_attempts = [{'map': 'Route2', 'registered': False},
                                {'map': 'Route2', 'registered': True},
                                {'map': 'Route3', 'registered': True}]
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {'POKEBALL': 5}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18,
                 'dex': {'owned_species': ['Pidgey', 'Rattata']}}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            context = agent.strategy_groups(facts)['collect:Route2']['context']
        self.assertEqual(context['unregistered_species'],
                         [{'species': 'Chansey', 'catch_rate': 30, 'band': 'hard'},
                          {'species': 'Zubat', 'catch_rate': 255, 'band': 'easy'}])
        # A table that is mostly duplicates is visible rather than inferred.
        self.assertEqual(context['already_registered_here'], ['Pidgey', 'Rattata'])
        self.assertEqual(context['recent_attempts'], {'hunts': 2, 'registered': 1})
        self.assertEqual(context['balls_held'], 5)
        self.assertEqual(context['prerequisite'], 'Catching spends balls; 5 carried')
        self.assertIn('expected_steps_to_any_new_species', context['encounter_value'])
        self.assertEqual(context['collection_resources']['ball_inventory'],
                         [{'ball': 'PokeBall', 'quantity': 5, 'quality': 'basic'}])
        self.assertIn('Chansey', context['species_scarcity'])
        self.assertEqual(context['encounters'],
                         ((agent.maps['Route2'].get('wild') or {}).get('red') or {}).get('grass'))

    def test_dex_panel_reports_progress_rungs_and_remaining_areas(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        def panel(owned, seen=()):
            return agent.dex_progress({'bag': {'POKEBALL': 3, 'POTION': 1},
                                       'dex': {'owned': len(owned), 'seen': len(seen),
                                               'owned_species': list(owned), 'seen_species': list(seen)}})
        start = panel(set())
        self.assertEqual((start['owned'], start['seen'], start['total']), (0, 0, 151))
        self.assertEqual(start['next_rung'], {'rung': 2, 'needs': 2, 'remaining': 2})
        self.assertEqual(panel({'Pidgey', 'Rattata'})['next_rung'],
                         {'rung': 10, 'needs': 10, 'remaining': 8})
        self.assertIsNone(panel({f'Species{index}' for index in range(150)})['next_rung'])
        # Species already met are known-reachable: the strongest collection lead.
        self.assertEqual(panel({'Pidgey'}, {'Zubat', 'Pidgey', 'Rattata'})['seen_not_owned'],
                         ['Rattata', 'Zubat'])
        self.assertEqual(panel({'Pidgey'})['unregistered_by_area'], {'Route2': 2, 'Route3': 2})
        self.assertEqual(panel({'Pidgey'})['expected_yield_by_area']['Route2']
                     ['unregistered_encounter_share_pct'], 55.1)
        self.assertEqual(panel({'Pidgey'})['balls_held'], 3)

    def test_map_hops_counts_map_crossings_on_real_map_data(self):
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {p.parent.name: json.loads(p.read_text()) for p in MAPS_DIR.glob('*/map.json')}
        self.assertEqual(agent.map_hops('MtMoon1F', 'ViridianMart'), 6)
        self.assertEqual(agent.map_hops('Route22', 'ViridianMart'), 2)
        # The diagnosis quoted two hops here, but the map graph cannot be
        # shorter: CeruleanMart opens only off CeruleanCity, and MtMoon1F's
        # street neighbours are Route4 and MtMoonB1F, so three is the minimum.
        self.assertEqual(agent.map_hops('MtMoon1F', 'CeruleanMart'), 3)
        # The cable-club rooms sit outside the walking world entirely.
        self.assertIsNone(agent.map_hops('PalletTown', 'Colosseum'))

    def test_dex_panel_points_at_the_nearest_reachable_ball_shop(self):
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}])
        viridian = Rule('shop:ViridianMart', 'ViridianMart', 'ViridianMart:shop', [], [], [],
                        ('shop', ('POTION', 'POKE_BALL'), True), [])
        cerulean = Rule('shop:CeruleanMart', 'CeruleanMart', 'CeruleanMart:shop', [], [], [],
                        ('shop', ('POKE_BALL', 'GREAT_BALL'), True), [])
        agent.index = Mock(rules=[viridian, cerulean])
        def panel(map_name):
            return agent.dex_progress({'map': map_name, 'bag': {},
                                       'dex': {'owned': 0, 'seen': 0,
                                               'owned_species': [], 'seen_species': []}})
        self.assertEqual(panel('MtMoon1F')['nearest_ball_source'],
                         {'map': 'CeruleanMart', 'hops': 3, 'stock': ['GreatBall', 'PokeBall']})
        self.assertEqual(panel('Route22')['nearest_ball_source'],
                         {'map': 'ViridianMart', 'hops': 2, 'stock': ['PokeBall']})
        agent.index = Mock(rules=[])
        self.assertIsNone(panel('MtMoon1F')['nearest_ball_source'])

    def test_dex_panel_is_only_assembled_for_the_collecting_goal(self):
        facts = {'bag': {}, 'dex': {'owned': 0, 'seen': 0, 'owned_species': [], 'seen_species': []}}
        collected = {}
        self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True}]).augment_strategy_state(
            collected, facts)
        self.assertIn('dex_progress', collected)
        story = {}
        self.catch_goal_agent([{'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}]
                              ).augment_strategy_state(story, facts)
        self.assertEqual(story, {})

    def test_strategy_assembly_asks_for_the_panel_before_judging(self):
        from openpokered.story_agent import DualStoryAgent
        agent = self.catch_goal_agent([{'id': 'collect-dex', 'agent_verified': True,
                                        'name': 'Register every wild species'}])
        agent.client.route.return_value = {'found': True, 'legs': []}
        agent.index.wild_species.return_value = {'Zubat'}  # collection still in progress
        rule = Rule('catch:Route2', 'Route2', 'skill:catch_encounter', [], [], [],
                    ('catch', 'Route2', True), [])
        group = {'target': ('catch', 'Route2', True), 'rules': [rule],
                 'objectives': ['Register wild species that are not in the Pokédex yet'], 'context': {}}
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'badges': 0, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18,
                 'dex': {'owned': 1, 'seen': 3, 'owned_species': ['Pidgey'],
                         'seen_species': ['Pidgey', 'Rattata', 'Zubat']}}
        agent.augment_strategy_state = Mock(wraps=agent.augment_strategy_state)
        agent.choose = Mock(return_value='subgoal:0')
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={'collect:Route2': group}):
            agent.select_strategy(facts)
        state, passed = agent.augment_strategy_state.call_args.args
        self.assertIs(state, agent.choose.call_args.args[1])
        self.assertEqual(passed, facts)
        self.assertEqual(state['dex_progress']['seen_not_owned'], ['Rattata', 'Zubat'])

    def ball_supply_agent(self, collecting=True):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.collects_dex = collecting
        rule = Rule('shop:ViridianMart', 'ViridianMart', 'ViridianMart:shop', [], [], [],
                    ('shop', ('POTION', 'POKE_BALL'), True), [])
        agent.index = Mock(rules=[rule])
        agent.visited = {'ViridianMart'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'ViridianMart'}]}
        return agent

    def test_ball_supply_is_bought_through_the_shop_path_only_when_collecting(self):
        facts = {'bag': {}, 'money': 3000, 'map': 'ViridianCity'}
        groups = {}
        self.ball_supply_agent(collecting=False).add_ball_supply(groups, facts)
        self.assertEqual(groups, {})
        self.ball_supply_agent().add_ball_supply(groups, facts)
        self.assertEqual(list(groups), ['ball:shop:ViridianMart:PokeBall'])
        group = groups['ball:shop:ViridianMart:PokeBall']
        self.assertEqual(group['target'], ('supply', 'PokeBall', 9))
        self.assertEqual(group['context']['stock_index'], 1)

    def test_ball_supply_stops_once_the_reserve_is_held(self):
        groups = {}
        self.ball_supply_agent().add_ball_supply(
            groups, {'bag': {'POKEBALL': 12}, 'money': 3000, 'map': 'ViridianCity'})
        self.assertEqual(groups, {})

    def test_ball_supply_offers_an_unvisited_mart_with_its_distance(self):
        agent = self.ball_supply_agent()
        agent.visited = set()  # A mart is a restock target before it is entered.
        agent.client.route.return_value = {'found': True, 'legs': [{}] * 6}
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        self.assertEqual(list(groups), ['ball:shop:ViridianMart:PokeBall'])
        context = groups['ball:shop:ViridianMart:PokeBall']['context']
        self.assertEqual((context['map'], context['map_hops']), ('ViridianMart', 6))

    def test_ball_supply_keeps_the_two_nearest_shops_per_ball_kind(self):
        agent = self.ball_supply_agent()
        agent.visited = set()
        cerulean = Rule('shop:CeruleanMart', 'CeruleanMart', 'CeruleanMart:shop', [], [], [],
                        ('shop', ('POKE_BALL',), True), [])
        saffron = Rule('shop:SaffronMart', 'SaffronMart', 'SaffronMart:shop', [], [], [],
                       ('shop', ('POKE_BALL',), True), [])
        agent.index = Mock(rules=[*agent.index.rules, cerulean, saffron])
        def route(origin, destination):
            legs = {'CeruleanMart': 2, 'SaffronMart': 4}.get(destination, 6)
            return {'found': True, 'legs': [{}] * legs}
        agent.client.route.side_effect = route
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        # The two nearest survive — a blocked route to the nearest must not
        # strike the whole supply line out — and a farther one is dropped.
        self.assertEqual(sorted(groups), ['ball:shop:CeruleanMart:PokeBall',
                                          'ball:shop:SaffronMart:PokeBall'])
        self.assertEqual(groups['ball:shop:CeruleanMart:PokeBall']['context']['map_hops'], 2)
        self.assertEqual(groups['ball:shop:SaffronMart:PokeBall']['context']['map_hops'], 4)

    def test_ball_supply_offers_nothing_when_no_shop_is_reachable(self):
        agent = self.ball_supply_agent()
        agent.client.route.return_value = {'found': False}
        groups = {}
        agent.add_ball_supply(groups, {'bag': {}, 'money': 3000, 'map': 'MtMoon1F'})
        self.assertEqual(groups, {})

    def catch_goal_agent(self, objectives):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), objectives, game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 0
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_catch_areas = Mock(return_value={'Route2': {'species': ['Weedle'],
                                                              'spots': [(5, 18)], 'reachable': True}})
        return agent

    def test_catch_targets_are_offered_only_for_the_collect_dex_goal(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            collecting = self.catch_goal_agent([{
                'id': 'collect-dex', 'agent_verified': True,
                'name': 'Register every wild species; clear the first playthrough to open the areas '
                            'that hold the rest'}]).strategy_groups(facts)
            story = self.catch_goal_agent([{
                'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}]).strategy_groups(facts)
        self.assertEqual(list(collecting), ['collect:Route2'])
        self.assertEqual(collecting['collect:Route2']['target'], ('catch', 'Route2', True))
        self.assertEqual(collecting['collect:Route2']['rules'][0].storyline, 'skill:catch_encounter')
        self.assertEqual(story, {})

    def test_catch_targets_state_the_ball_prerequisite(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        def group(bag):
            facts = {'party': [mon], 'bag': bag, 'flags': {}, 'fully_recovered': False,
                     'map': 'PewterCity', 'x': 12, 'y': 18}
            with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
                return self.catch_goal_agent([{'id': 'collect-dex',
                                               'agent_verified': True}]).strategy_groups(facts)['collect:Route2']
        empty = group({})
        self.assertEqual(empty['context']['balls_held'], 0)
        self.assertIn('No balls are carried', empty['context']['prerequisite'])
        stocked = group({'POKEBALL': 7})
        self.assertEqual(stocked['context']['balls_held'], 7)
        self.assertIn('7 carried', stocked['context']['prerequisite'])

    def test_dex_completion_requires_every_supported_acquisition_and_is_never_vacuous(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.index._wild_cache = {'Route1': {'Pidgey', 'Rattata'}, 'ViridianCity': set()}
        agent.maps = {'Route1': {}, 'ViridianCity': {}}
        agent._collection_graph = {
            'Pidgey': [{'method': 'grass', 'map': 'Route1'}],
            'Rattata': [{'method': 'grass', 'map': 'Route1'}],
            'Magikarp': [{'method': 'fishing', 'map': 'ViridianCity', 'rod': 'OldRod'}],
        }
        agent._complete_collection_graph = agent._collection_graph
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Pidgey']}}))
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Rattata', 'Pidgey']}}))
        self.assertTrue(agent.dex_complete({'dex': {'owned_species': ['Rattata', 'Pidgey', 'Magikarp']}}))
        # The spawn room has no encounter table. "Nothing unregistered here"
        # must not read as a finished collection before the run has moved.
        agent.index._wild_cache = {'RedsHouse2F': set(), 'Route1': {'Pidgey'}}
        agent.maps = {'RedsHouse2F': {}, 'Route1': {}}
        agent._collection_graph = {'Pidgey': [{'method': 'grass', 'map': 'Route1'}]}
        agent._complete_collection_graph = agent._collection_graph
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': []}}))
        agent.index = None  # No planning index yet: nothing is proven registered.
        self.assertFalse(agent.dex_complete({'dex': {'owned_species': ['Pidgey']}}))

    def test_collect_dex_objective_is_decided_by_registered_species(self):
        from openpokered.story_rules import StoryIndex
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = StoryIndex.__new__(StoryIndex)
        agent.index._wild_cache = {'Route1': {'Pidgey'}}
        agent.maps = {'Route1': {}}
        agent._complete_collection_graph = {
            'Pidgey': [{'method': 'grass', 'map': 'Route1'}],
        }
        objective = {'id': 'collect-dex', 'agent_verified': True,
                     'name': 'Register every wild species; clear the first playthrough to open the areas '
                            'that hold the rest'}
        self.assertFalse(agent.objective_satisfied(objective, {'dex': {'owned_species': []}}))
        self.assertTrue(agent.objective_satisfied(objective, {'dex': {'owned_species': ['Pidgey']}}))

    def test_runner_collect_dex_goal_enables_the_catch_targets(self):
        from openpokered.run_autonomous import DEX_OBJECTIVE
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [DEX_OBJECTIVE], game=Mock())
        self.assertNotIn('satisfied_when', DEX_OBJECTIVE)  # The goal waits for no game flag.
        self.assertTrue(agent.collects_dex)

    def test_runner_goal_registry_reuses_the_collect_dex_entry_and_the_champion_flag(self):
        from openpokered import run_autonomous
        from openpokered.judgment_agent import load_objectives
        self.assertEqual(list(run_autonomous.GOAL_OBJECTIVES),
                         ['collect-dex', 'max-coverage', 'fast-clear'])
        self.assertIs(run_autonomous.DEX_OBJECTIVE, run_autonomous.GOAL_OBJECTIVES['collect-dex'])
        self.assertTrue(all(key == goal['id'] for key, goal in run_autonomous.GOAL_OBJECTIVES.items()))
        flagged = {key: goal for key, goal in run_autonomous.GOAL_OBJECTIVES.items()
                   if not goal.get('agent_verified')}
        self.assertEqual(list(flagged), ['fast-clear'])
        champion = next(o for o in load_objectives() if o['id'] == 'become-champion')
        self.assertEqual(flagged['fast-clear']['satisfied_when'], champion['satisfied_when'])

    def test_runner_records_the_preference_and_threads_it_into_the_agent(self):
        import io
        import tempfile
        from openpokered import run_autonomous
        options = {}
        add_argument = run_autonomous.argparse.ArgumentParser.add_argument
        def record(parser, *names, **kwargs):
            for name in names:
                options[name] = kwargs
            return add_argument(parser, *names, **kwargs)
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / 'pokered-app').write_bytes(b'binary')
            logs = path / 'logs'
            logs.mkdir()
            (logs / 'game.log').write_text('')
            game, agent = Mock(), Mock()
            game.run_dir, game.d.counts = logs, {}
            game.battles_driven, game.move_cache_hits, game.stationary_npcs = 0, 0, {}
            game.d.raw.cmd.return_value = {'ok': True, 'data': {}}
            agent.run.return_value = {}
            agent.calls, agent.tokens, agent.models = {}, {}, set()
            agent.completed, agent.actions, agent.resolved_battles = [], 0, 0
            agent.visited, agent.observed_barrier_maps = set(), set()
            agent.navigation_memory, agent.navigation_history = {}, {}
            agent.field_requirements, agent.battle_requirements = {}, {}
            agent.battle_defeats, agent.defeat_preparation = [], 0
            agent.first_clear_verification, agent.mechanism_goal = None, None
            with patch.object(run_autonomous.argparse.ArgumentParser, 'add_argument', record), \
                    patch.object(run_autonomous, 'TypeSafeClient'), \
                    patch.object(run_autonomous, 'JevGame', return_value=game), \
                    patch.object(run_autonomous, 'boot_new_game', return_value={'screen': 'overworld'}), \
                    patch.object(run_autonomous, 'AutonomousStoryAgent', return_value=agent) as factory, \
                    patch('sys.stdout', new_callable=io.StringIO):
                code = run_autonomous.main(['--preference', 'level',
                                            '--binary', str(path / 'pokered-app'),
                                            '--output', str(path / 'out')])
            self.assertEqual(options['--preference']['choices'], ['none', 'level', 'type', 'tactic'])
            self.assertEqual(options['--preference']['default'], 'none')
            self.assertEqual(factory.call_args.kwargs['preference'], 'level')
            self.assertEqual(code, 1)
            folder = next((path / 'out').iterdir())
            self.assertFalse((folder / 'failure.txt').exists())  # The stubbed run reached the end.
            self.assertEqual(json.loads((folder / 'summary.json').read_text())['preference'], 'level')

    def test_every_goal_entry_appends_a_terminal_objective_to_the_story_prefix(self):
        from openpokered import run_autonomous
        from openpokered.judgment_agent import load_objectives
        for key, goal in run_autonomous.GOAL_OBJECTIVES.items():
            # The constructor rejects an unverified objective without a flag,
            # so building the agent proves the appended entry is complete.
            agent = self.goal_agent(load_objectives() + [goal])
            self.assertEqual(agent.objectives[-1]['id'], key)

    def goal_agent(self, objectives):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), objectives, game=Mock())
        agent.index = Mock(rules=[], by_effect={})
        agent.defeat_preparation = 0
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        return agent

    def goal_flags(self, ids):
        agent = self.goal_agent([{'id': name, 'agent_verified': True} for name in ids])
        return agent.collects_dex, agent.maximizes_coverage, agent.avoids_optional_preparation

    def test_goal_flags_are_derived_from_the_objective_ids(self):
        self.assertEqual(self.goal_flags(['get-starter']), (False, False, False))
        self.assertEqual(self.goal_flags(['collect-dex']), (True, False, False))
        self.assertEqual(self.goal_flags(['max-coverage']), (False, True, False))
        self.assertEqual(self.goal_flags(['fast-clear']), (False, False, True))

    def test_goal_flags_add_their_instruction_to_the_strategy_call(self):
        from openpokered.story_agent import DualStoryAgent
        def instruction(objectives):
            agent = self.goal_agent(objectives)
            with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
                agent.choose('strategy', {}, {'a': 'x'}, 'pick')
            return choose.call_args.args[3]
        story = instruction([{'id': 'beat-brock', 'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}])
        coverage = instruction([{'id': 'max-coverage', 'agent_verified': True}])
        speed = instruction([{'id': 'fast-clear', 'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}])
        dex = instruction([{'id': 'collect-dex', 'agent_verified': True}])
        self.assertEqual(story, 'pick')  # A story run keeps its instruction untouched.
        self.assertIn('terminal goal is coverage', coverage)
        self.assertIn('progress in itself', coverage)
        self.assertIn('terminal goal is speed', speed)
        self.assertIn('shortest route', speed)
        # Collection must be told that clearing is the channel to more species,
        # or it would treat the story objectives as the finish line.
        self.assertIn('terminal goal is the Pokédex', dex)
        self.assertIn('channel to more species', dex)
        self.assertIn('never stop at the Champion', dex)

    def preference_agent(self, preference):
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        return AutonomousStoryAgent(client, Mock(), [{'id': 'beat-brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock(), preference=preference)

    def test_preference_bias_reaches_the_strategy_and_action_questions(self):
        from openpokered.story_agent import DualStoryAgent
        from openpokered.autonomous_story import PREFERENCE_INSTRUCTIONS
        self.assertEqual(sorted(PREFERENCE_INSTRUCTIONS), ['level', 'tactic', 'type'])
        for preference, bias in PREFERENCE_INSTRUCTIONS.items():
            agent = self.preference_agent(preference)
            for layer in ('strategy', 'action'):
                with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
                    self.assertEqual(agent.choose(layer, {}, {'a': 'x'}, 'pick'), 'a')
                self.assertEqual(choose.call_args.args[3], f'pick {bias}')
        with patch.object(DualStoryAgent, 'choose', return_value='a') as choose:
            self.preference_agent('none').choose('strategy', {}, {'a': 'x'}, 'pick')
        self.assertEqual(choose.call_args.args[3], 'pick')

    def preparation_agent(self, preference, moves=('Scratch', 'Ember', 'Growl')):
        """An agent whose only offered battle is one stubbed level 21 trainer."""
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        agent = AutonomousStoryAgent(client, Mock(), [{'id': 'beat-brock',
            'satisfied_when': {'flag': 'EVENT_BEAT_BROCK'}}], game=Mock(), preference=preference)
        agent.index = Mock(rules=[], by_effect={})
        agent.destination_points = Mock(return_value=[])
        agent.nearby_healers = Mock(return_value=[])
        agent.annotate_navigation = Mock(return_value={})
        agent.transport_frontiers = Mock()
        agent.find_training_sites = Mock(return_value={'Route2': (7, 7)})
        agent.find_catch_areas = Mock(return_value={})
        agent.maps = {'Route2': {}, 'Route3': {'npcs': [
            {'textId': 2, 'trainerClass': 'Youngster', 'trainerSet': 1}]}}
        agent.trainers = {'Youngster': {'parties': [
            {'pokemon': [{'species': 'Bellsprout', 'level': 21}]}]}}
        rule = Rule('youngster', 'Route3', 'Route3:youngster', ['npc:2'], [], [],
                    ('flag', 'EVENT_BEAT_YOUNGSTER', True), [('battle', 'Youngster:1', True)])
        facts = {'party': [{'species': 'Charmeleon', 'level': 16, 'hp': 47, 'max_hp': 47,
                            'status': 'None', 'moves': list(moves), 'pp': [35] * len(moves)}],
                 'bag': {}, 'flags': {}, 'fully_recovered': True,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        return agent, facts, {'youngster': {'target': rule.effect, 'rules': [rule],
                                           'objectives': ['Defeat the route trainer']}}

    def preparation_groups(self, preference, moves=('Scratch', 'Ember', 'Growl')):
        from openpokered.story_agent import DualStoryAgent
        agent, facts, groups = self.preparation_agent(preference, moves)
        with patch.object(DualStoryAgent, 'strategy_groups', return_value=groups):
            return agent.strategy_groups(facts)

    def test_level_preference_trains_a_margin_past_the_pending_threat(self):
        from openpokered.autonomous_story import LEVEL_PREFERENCE_MARGIN
        self.assertEqual(LEVEL_PREFERENCE_MARGIN, 2)
        self.assertEqual(self.preparation_groups('none')['prepare:train']['target'],
                         ('level', 'leader', 21))
        biased = self.preparation_groups('level')['prepare:train']
        self.assertEqual(biased['target'], ('level', 'leader', 21 + LEVEL_PREFERENCE_MARGIN))
        self.assertEqual(biased['context']['target_level'], 21 + LEVEL_PREFERENCE_MARGIN)

    def test_type_preference_adds_super_effective_options_to_the_battle_context(self):
        context = self.preparation_groups('type')['youngster']['context']
        self.assertEqual(context['type_options']['Bellsprout']['types'], ['Grass', 'Poison'])
        self.assertEqual(context['type_options']['Bellsprout']['super_effective_moves'],
                         {'Ember': {'pokemon': 'Charmeleon', 'type': 'Fire', 'power': 40,
                                    'effectiveness': 2}})
        self.assertNotIn('type_options', self.preparation_groups('none')['youngster']['context'])

    def test_tactical_preference_adds_status_moves_and_none_adds_no_options(self):
        context = self.preparation_groups('tactic')['youngster']['context']
        self.assertEqual(context['tactical_options']['opponent_species'], ['Bellsprout'])
        self.assertEqual(context['tactical_options']['status_moves'], {'Growl': {
            'pokemon': 'Charmeleon', 'type': 'Normal', 'effect': 'AttackDown1Effect'}})
        story = self.preparation_groups('none')['youngster']['context']
        self.assertNotIn('tactical_options', story)
        self.assertNotIn('type_options', story)

    def test_coverage_completion_requires_every_bordering_map(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'PalletTown': {'connections': {'north': {'targetMap': 'Route1'}},
                                     'warps': [{'x': 5, 'y': 5, 'destMap': 'RedsHouse1F'}]},
                      'Route1': {'connections': {'south': {'targetMap': 'PalletTown'},
                                                 'north': {'targetMap': 'ViridianCity'}}},
                      'RedsHouse1F': {}, 'ViridianCity': {}}
        agent.visited = set()
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F'}
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F', 'Route1'}
        self.assertFalse(agent.coverage_complete({}))
        agent.visited = {'PalletTown', 'RedsHouse1F', 'Route1', 'ViridianCity'}
        self.assertTrue(agent.coverage_complete({}))

    def test_max_coverage_objective_is_decided_by_bordering_maps(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'PalletTown': {'connections': {'north': {'targetMap': 'Route1'}}}, 'Route1': {}}
        objective = {'id': 'max-coverage', 'agent_verified': True,
                     'name': 'Leave no bordering area unexplored and finish the first playthrough'}
        agent.visited = {'PalletTown'}
        self.assertFalse(agent.objective_satisfied(objective, {}))
        agent.visited = {'PalletTown', 'Route1'}
        self.assertTrue(agent.objective_satisfied(objective, {}))

    def test_speed_goal_drops_optional_preparation_groups(self):
        from openpokered.story_agent import DualStoryAgent
        mon = {'species': 'Charmeleon', 'level': 16, 'hp': 20, 'max_hp': 47,
               'status': 'None', 'moves': ['Scratch', 'Ember'], 'pp': [8, 25]}
        facts = {'party': [mon], 'bag': {'POTION': 1}, 'flags': {}, 'fully_recovered': False,
                 'map': 'PewterCity', 'x': 12, 'y': 18}
        with patch.object(DualStoryAgent, 'strategy_groups', side_effect=lambda facts: {}):
            speed = self.goal_agent([{'id': 'fast-clear', 'satisfied_when': {
                'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}]).strategy_groups(facts)
            story = self.goal_agent([{'id': 'beat-brock', 'satisfied_when': {
                'flag': 'EVENT_BEAT_BROCK'}}]).strategy_groups(facts)
        self.assertEqual(list(story), ['prepare:medicine'])
        self.assertTrue(story['prepare:medicine']['context']['optional_preparation'])
        self.assertEqual(speed, {})

    def test_code_layers_pick_candidates_without_calling_the_model(self):
        from openpokered.run_autonomous import DEX_OBJECTIVE
        client = Mock()
        client.state.return_value = {'map_name': 'PewterCity', 'hall_of_fame_count': 0}
        model = Mock()
        agent = AutonomousStoryAgent(client, model, [DEX_OBJECTIVE], game=Mock(),
                                     strategy_jev=False, action_jev=False)
        self.assertEqual(agent.layer_jev, {'strategy': False, 'action': False})
        self.assertEqual(agent.choose('strategy', {}, {'a': 'x', 'b': 'y'}, 'pick'), 'a')
        self.assertFalse(model.system_one.called)

    def test_coverage_groups_offer_unvisited_bordering_areas(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'A': {'connections': {'north': {'targetMap': 'B'}}, 'warps': []},
                      'B': {'connections': {}, 'warps': []},
                      'C': {'connections': {}, 'warps': [{'x': 4, 'y': 6, 'destMap': 'D'}]},
                      'D': {'connections': {}, 'warps': []}}
        agent.visited = {'A', 'C'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': True, 'legs': [{'to_map': 'B'}]}
        agent.entry_tile = lambda name: (3, 5)
        groups = {}
        agent.add_coverage_groups(groups, {'map': 'A'})
        self.assertEqual(sorted(groups), ['explore:B', 'explore:D'])
        self.assertEqual(groups['explore:B']['target'], ('explore', 'B', True))
        self.assertEqual(groups['explore:B']['rules'][0].map, 'B')
        self.assertEqual(groups['explore:B']['rules'][0].triggers, ['coord:(3,5)'])

    def test_coverage_groups_skip_areas_with_no_confirmed_route(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {'A': {'connections': {'north': {'targetMap': 'B'}}, 'warps': []},
                      'B': {'connections': {}, 'warps': []}}
        agent.visited = {'A'}
        agent.client = Mock()
        agent.client.route.return_value = {'found': False, 'legs': []}
        agent.entry_tile = lambda name: (3, 5)
        groups = {}
        agent.add_coverage_groups(groups, {'map': 'A'})
        self.assertEqual(groups, {})

    def test_receiving_travel_supplies_replans_before_a_full_bag_pickup(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.active = {'target': ('item', 'GOLD_TEETH', True)}
        agent.replan_after_defeat, agent.needs_healing = False, Mock(return_value=False)
        bag = {f'item{i}': 1 for i in range(19)}
        self.assertFalse(agent.should_replan({'bag': bag}))
        self.assertTrue(agent.should_replan({'bag': {**bag, 'SAFARIBALL': 30}}))

    def test_inventory_preparation_teaches_only_compatible_tms_and_preserves_hms(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        options = agent.inventory_use_options({'bag': {'TM24': 1}, 'party': [
            {'species': 'Charizard', 'level': 60, 'hp': 100, 'moves': ['Flamethrower', 'Cut', 'Slash', 'FireSpin']},
            {'species': 'Lapras', 'level': 16, 'hp': 60, 'moves': ['WaterGun', 'Growl', 'Surf', 'Sing']}]})
        self.assertTrue(options)
        self.assertTrue(all(op.startswith('teach_tm:Tm24,Thunderbolt,1,') for op, _ in options))
        self.assertFalse(any(op.endswith(',Surf') for op, _ in options))

    def test_canonical_gate_warps_override_an_outdated_map_export(self):
        import playthrough as pt
        self.assertEqual(pt.warp_edges_from('Route22Gate', 4, 0, 'Route22'), [('Route23', 7, 139)])
        self.assertEqual(pt.warp_edges_from('Route22Gate', 4, 7, 'Route23'), [('Route22', 8, 5)])
        path = pt.bfs_cross('Route22', (8, 6), 'Route23', (7, 138), last_map='Route22')
        self.assertTrue(path)
        self.assertIn('Route22Gate', {node[0] for node, _ in path[1:]})

    def test_consumable_skill_closes_result_menus_after_exactly_one_use(self):
        game = JevGame.__new__(JevGame)
        game.d, game.judgments, game.tap = Mock(), Mock(), Mock()
        game.d.cmd.side_effect = [
            {'data': [{'item': 'RareCandy', 'qty': 1}]},
            {'data': [{'item': 'RareCandy', 'qty': 1}]},
            {'data': []}, {'data': []}, {'data': []}]
        game.st = Mock(side_effect=[
            {'field_menu': {'kind': 'party', 'phase': 'Browsing', 'cursor': 1}},
            {'field_menu': {'kind': 'party', 'phase': 'ItemUseNotice { text: "Level up" }'}},
            {'field_menu': {'kind': 'bag', 'phase': 'Browsing', 'cursor': 0}},
            {'field_menu': None}])
        game.use_consumable('RareCandy', 1)
        self.assertEqual([c.args[0] for c in game.tap.call_args_list], ['a', 'a', 'b'])

    def test_forced_bike_road_is_not_a_surf_shortcut(self):
        import playthrough as pt
        from openpokered.navigation_skills import forced_bike_region
        region = forced_bike_region()
        self.assertIn(('Route18', 23, 6), region)
        self.assertNotIn(('FuchsiaCity', 5, 14), region)
        with water_planning():
            self.assertFalse(pt.walkable_edge('Route18', (23, 6), (23, 5)))
            self.assertTrue(pt.walkable_edge('PalletTown', (5, 13), (5, 14)))

    def test_push_search_solves_geometry_without_changing_the_map(self):
        import playthrough as pt
        from openpokered.boulder_skills import BOULDER_TARGETS, plan_pushes
        from openpokered.story_rules import MAPS_DIR
        name = 'VictoryRoad1F'
        flag = 'EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH'
        npcs = json.loads((MAPS_DIR / name / 'map.json').read_text())['npcs']
        rocks = {(n['x'], n['y']) for n in npcs if n['spriteName'] == 'Boulder'}
        fixed = {(n['x'], n['y']) for n in npcs if n['spriteName'] != 'Boulder'}
        blocks = list(pt.MAPS[name]['blocks'])
        plan = plan_pushes(name, (8, 16), rocks, fixed, BOULDER_TARGETS[flag]['target'])
        self.assertTrue(plan)
        player = (8, 16)
        for step in plan:
            self.assertIn(step['boulder'], rocks)
            self.assertIsNotNone(pt.bfs_cross(name, player, name, step['stance'],
                                             blocked_maps={name: rocks | fixed}, last_map='Route23'))
            self.assertNotIn(step['landing'], rocks | fixed)
            self.assertTrue(pt.walkable(name, *step['landing']))
            rocks.remove(step['boulder'])
            rocks.add(step['landing'])
            player = step['boulder']
        self.assertIn(BOULDER_TARGETS[flag]['target'], rocks)
        self.assertEqual(pt.MAPS[name]['blocks'], blocks)
        self.assertEqual(BOULDER_TARGETS['EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2']['target'], (23, 15))

    def test_current_displacement_returns_observation_and_restores_land_planning(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        obstacle = {'map': 'UpperCave', 'landing': ['UpperCave', 3, 9]}
        agent.active = {'context': obstacle}
        agent.field_requirements = {'Surf': obstacle}
        agent.observed_barrier_maps = set()
        agent.game, agent.record = Mock(), Mock()
        agent.game.st.return_value = {'player_transport': 'Surfing', 'map_name': 'LowerCave',
                                     'player_x': 20, 'player_y': 17}
        agent.game.nav_to_map.side_effect = pt.NavError('current displaced the player')
        rule = Rule('water', 'UpperCave', 'water', [], [], [], (), [])
        original = pt.walkable, pt.walkable_edge
        result = agent.execute('surf:1', rule)
        self.assertEqual(result['result'], 'blocked')
        self.assertEqual(result['position'], ['LowerCave', 20, 17])
        self.assertEqual(agent.observed_barrier_maps, {'UpperCave', 'LowerCave'})
        self.assertNotIn('Surf', agent.field_requirements)
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        self.assertFalse(agent.game.navigation_active)

    def test_intermediate_menu_knows_the_destination_not_only_the_final_effect(self):
        from openpokered.story_agent import DualStoryAgent
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.client, agent.game = Mock(), Mock(navigation_active=True)
        agent.check_budget, agent.tap, agent.record = Mock(), Mock(), Mock()
        agent.settle_special = Mock(return_value=False)
        agent.choose = Mock(return_value='0')
        agent.navigation_intent = {'destination': 'HealingCenter'}
        agent.client.state.side_effect = [
            {'screen': 'overworld', 'map_name': 'Gate', 'choice': {'options': ['YES', 'NO'], 'selected': 0}},
            {'screen': 'overworld', 'map_name': 'Gate'}]
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.settle(('heal', 'party', True))
        offered = agent.choose.call_args.args[1]
        self.assertEqual(offered['travel_in_progress']['destination'], 'HealingCenter')
        self.assertEqual(offered['current_map'], 'Gate')

    def test_settle_dismisses_the_post_catch_pokedex_screen(self):
        from openpokered.story_agent import DualStoryAgent
        agent = DualStoryAgent.__new__(DualStoryAgent)
        agent.client = Mock()
        agent.check_budget, agent.tap, agent.record = Mock(), Mock(), Mock()
        agent.settle_special = Mock(return_value=False)
        settled = {'screen': 'overworld'}
        agent.client.state.side_effect = [
            {'screen': 'pokedex', 'active_script_effect': None},
            {'screen': 'pokedex', 'active_script_effect': None},
            settled, settled, settled]
        agent.client.observe.return_value = {'mode': 'overworld'}
        agent.settle(('dex', 'count', 10))
        self.assertEqual([c.args[0] for c in agent.tap.call_args_list], ['b', 'b'])

    def battle_state(self, **live_overrides):
        live = {'is_wild': True, 'is_safari': False,
                'enemy': {'species': 'Caterpie', 'level': 3, 'hp': 16, 'max_hp': 16, 'status': 'None'},
                'player': {'species': 'Charmander'},
                'player_party': [{'species': 'Charmander', 'level': 12, 'hp': 30, 'max_hp': 30,
                                  'status': 'None', 'moves': ['Scratch', 'None'], 'pp': [35, 0]}]}
        live.update(live_overrides)
        return {'battle_live': live,
                'party': [{'species': 'Charmander', 'level': 12, 'hp': 30, 'max_hp': 30, 'status': 'None'}],
                'battle_inventory': [{'item': 'PokeBall', 'qty': 5}],
                'pokedex': {'owned_species': ['Pidgey']}}

    def test_wild_encounter_offers_balls_and_honours_the_chosen_one(self):
        from openpokered.playthrough_judgments import ball_options, JevGame
        state = self.battle_state()
        offered = list(ball_options(state['battle_live'], {'PokeBall': 5, 'Potion': 2}, ['Pidgey']))
        self.assertEqual([name for name, _t, _d in offered], ['PokeBall'])
        details = offered[0][2]
        self.assertEqual((details['catch_rate'], details['already_owned'], details['quantity']),
                         (255, False, 5))
        self.assertEqual((details['capture_probability_now'], details['hp_percent']),
                         (0.3359, 100.0))
        agent = JevGame.__new__(JevGame)
        agent.judgments = Mock()
        agent.judgments.choose.return_value = 'ball:PokeBall'
        self.assertEqual(JevGame.battle_recovery_plan(agent, state), ('PokeBall', None))
        self.assertIn('ball:PokeBall', agent.judgments.choose.call_args.args[2])

    def test_capture_probability_reflects_hp_status_and_gen1_ball_formula(self):
        from openpokered.playthrough_judgments import capture_probability
        enemy = {'hp': 16, 'max_hp': 16, 'catch_rate': 45, 'status': 'None'}
        full = capture_probability('PokeBall', enemy)
        sleeping = capture_probability('PokeBall', {**enemy, 'status': 'Sleep'})
        weakened = capture_probability('PokeBall', {**enemy, 'hp': 1})
        ultra = capture_probability('UltraBall', enemy)
        safari = capture_probability('SafariBall', enemy)
        self.assertLess(full, sleeping)
        self.assertLess(full, weakened)
        self.assertGreater(ultra, full)
        self.assertEqual(safari, ultra)
        self.assertEqual(capture_probability('MasterBall', enemy), 1.0)

    def test_collecting_frames_the_ball_choice_around_the_goal(self):
        from openpokered.playthrough_judgments import JevGame
        state = self.battle_state()
        def instruction(collects):
            agent = JevGame.__new__(JevGame)
            agent.judgments = Mock()
            agent.judgments.collects_dex = collects
            agent.judgments.choose.return_value = 'fight'
            JevGame.battle_recovery_plan(agent, state)
            return agent.judgments.choose.call_args.args[3]
        phrase = 'register species that are not in the Pokédex yet'
        self.assertIn(phrase, instruction(True))
        self.assertNotIn(phrase, instruction(False))

    def test_balls_are_never_offered_against_a_trainer_or_in_the_safari_zone(self):
        from openpokered.playthrough_judgments import ball_options, JevGame
        self.assertEqual(list(ball_options({'is_wild': False, 'enemy': {'species': 'Caterpie'}}, {'PokeBall': 5})), [])
        self.assertEqual(list(ball_options({'is_wild': True, 'is_safari': True,
                                           'enemy': {'species': 'Caterpie'}}, {'PokeBall': 5})), [])
        for live in ({'is_wild': False}, {'is_wild': True, 'is_safari': True}):
            state = self.battle_state(**live)
            agent = JevGame.__new__(JevGame)
            agent.judgments = Mock()
            # No ball and no useful medicine leaves nothing to spend the turn
            # on, so the plan stays None and the judge is not even consulted.
            self.assertIsNone(JevGame.battle_recovery_plan(agent, state))
            self.assertFalse(agent.judgments.choose.called)

    def test_safari_action_exposes_capture_flee_and_ball_factors(self):
        from openpokered.playthrough_judgments import safari_action_options, JevGame
        state = self.battle_state(is_safari=True)
        state['battle_live']['safari'] = {
            'base_catch_rate': 45, 'catch_rate': 45, 'bait_factor': 0,
            'escape_factor': 0, 'balls': 9, 'enemy_speed': 60,
        }
        options = safari_action_options(state['battle_live'], state['pokedex']['owned_species'])
        self.assertEqual(set(options), {'ball', 'bait', 'rock', 'run'})
        self.assertGreater(options['rock']['projected_catch_probability'],
                           options['ball']['capture_probability_now'])
        self.assertEqual(options['ball']['capture_probability_now'], 0.1023)
        self.assertEqual(options['rock']['duration_turns_uniform'], [1, 5])
        self.assertGreater(options['rock']['projected_flee_probability_next_turn'],
                           options['ball']['flee_probability_if_not_caught'])
        self.assertLess(options['bait']['projected_flee_probability_next_turn'],
                        options['ball']['flee_probability_if_not_caught'])
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'ball'
        self.assertEqual(game.safari_battle_action(state), 'ball')
        self.assertIn('capture probability', game.judgments.choose.call_args.args[3])

    def test_native_door_interaction_reaches_the_corridor_side(self):
        import playthrough as pt
        from types import SimpleNamespace
        from openpokered.story_agent import DualStoryAgent
        from openpokered.story_rules import MAPS_DIR
        from openpokered.autonomous_story import NATIVE_INTERACTIONS
        name = 'SilphCo11F'
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        rule = Rule('door', name, name + ':cardKeyDoor', ['coord:(6,12)'], [], [],
                    ('flag', 'UNLOCKED', True), [])
        agent.index = SimpleNamespace(coordinates=lambda _: [(6, 12)])
        agent.active = {'target': rule.effect, 'rules': [rule]}
        agent.client = Mock()
        agent.client.cmd.return_value = []
        blocks = list(pt.MAPS[name]['blocks'])
        blocks[6*pt.MAPS[name]['width']+3] = 32
        facts = {'map': name, 'x': 3, 'y': 12}
        with patch.dict(pt.MAPS[name], {'blocks': blocks}):
            self.assertIn((6, 14), agent.destination_points(name, rule))
            with patch.object(DualStoryAgent, 'action_candidates', return_value=({}, {})):
                _, bindings = agent.action_candidates(facts)
            self.assertIn('interact_tile:6,13', [op for op, _ in bindings.values()])
        # Other floors only declare OnStep: do not invent A-button bindings.
        self.assertNotIn('SilphCo7F:cardKeyDoor1', NATIVE_INTERACTIONS)

    def test_recovery_does_not_forget_a_healer_behind_a_locked_door(self):
        from types import SimpleNamespace
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        near = Rule('near', 'LockedRoom', 'Room:heal', ['npc:1'], [], [], ('heal', 'party', True), [])
        far = Rule('far', 'Center', 'Center:heal', ['npc:1'], [], [], ('heal', 'party', True), [])
        agent.index = SimpleNamespace(by_effect={near.effect: [near, far]})
        agent.client = Mock()
        agent.client.route.side_effect = lambda start, dest: {'found': True, 'legs': [0] * (1 if dest == 'LockedRoom' else 3)}
        agent.navigation_memory = {'LockedRoom': {'detail': 'locked'}}
        agent.game = Mock()
        agent.destination_points = Mock(return_value=[(3, 3)])
        facts = {'map': 'Hallway', 'x': 1, 'y': 1}
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=None):
            self.assertEqual(agent.nearby_healers(facts), [far])
        with patch('openpokered.autonomous_story.pt.bfs_cross', return_value=[('Hallway', 1, 1)]):
            self.assertEqual(agent.nearby_healers(facts), [near, far])

    def test_possible_battle_loss_does_not_seal_off_the_challenge_trigger(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Tower'}
        pushback = Rule('loss', 'Tower', 'Tower:ghost', ['coord:(10,16)'], [], [],
                        ('movement', 'movePlayerRelative', True), [('battle', 'Marowak', True)])
        agent.index = Mock(rules=[pushback])
        agent.index.coordinates.return_value = [(10, 16)]
        self.assertEqual(agent.observed_navigation_barriers({'flags': {}}), {})

    def test_declining_an_affordable_entrance_does_not_make_its_trigger_a_wall(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Gate'}
        refusal = Rule('refuse', 'Gate', 'Gate:entry', ['coord:(3,2)'], [], ['NO'],
                       ('movement', 'movePlayerRelative', True), [])
        payment = {'Call': {'callee': 'hasMoney', 'args': [{'NumberLit': 500}]}}
        entrance = Rule('pay', 'Gate', 'Gate:entry', ['coord:(3,2)'], [(payment, True)], ['YES'],
                        ('transport', ('Park', 14, 25), True), [])
        agent.index = Mock(rules=[refusal, entrance])
        agent.index.coordinates.return_value = [(3, 2)]
        self.assertEqual(agent.observed_navigation_barriers({'money': 500}), {})
        self.assertEqual(agent.observed_navigation_barriers({'money': 499}), {'Gate': {(3, 2)}})
        # Another transport in the building cannot open this guarded trigger.
        entrance.storyline = 'Gate:unrelated'
        self.assertEqual(agent.observed_navigation_barriers({'money': 500}), {'Gate': {(3, 2)}})

    def test_exit_confirmation_can_disable_its_own_movement_guard(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Gate'}
        guard = {'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'INSIDE'}]}}
        movements = [Rule(choice, 'Gate', 'Gate:exit', ['coord:(3,2)'], [(guard, True)], [choice],
                          ('movement', 'movePlayerRelative', True), []) for choice in ('YES', 'NO')]
        leave = Rule('leave', 'Gate', 'Gate:exit', ['coord:(3,2)'], [(guard, True)], ['YES'],
                     ('flag', 'INSIDE', False), [])
        agent.index = Mock(rules=[*movements, leave])
        agent.index.coordinates.return_value = [(3, 2)]
        self.assertEqual(agent.observed_navigation_barriers({'flags': {'INSIDE': True}}), {})
        # An unrelated flag write does not remove the active guard.
        leave.effect = ('flag', 'UNRELATED', False)
        self.assertEqual(agent.observed_navigation_barriers({'flags': {'INSIDE': True}}),
                         {'Gate': {(3, 2)}})

    def test_npc_approach_reassesses_after_an_interrupting_battle_before_talking(self):
        from openpokered.playthrough_judgments import NavigationPause
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.actions, agent.max_actions = 0, 10
        agent.game = Mock()
        agent.game.approach_object.side_effect = NavigationPause('trainer interrupted')
        agent.client = Mock()
        agent.client.cmd.return_value = [{'npc_index': 0, 'x': 25, 'y': 3, 'visible': True}]
        agent.tap, agent.settle, agent.record = Mock(), Mock(), Mock()
        rule = Rule('boss', 'Base', 'Base:boss', ['npc:1'], [], [], ('flag', 'WON', True), [])
        agent.active = {'target': rule.effect}
        result = agent.execute('interact_with:npc:0', rule)
        self.assertEqual(result['result'], 'paused_after_battle')
        agent.tap.assert_not_called()
        self.assertFalse(agent.game.navigation_active)
        agent.settle.assert_called_once_with(rule.effect, rule)
        agent.game.approach_object.side_effect = None
        self.assertEqual(agent.execute('interact_with:npc:0', rule)['result'], 'interacted_with_npc')
        agent.tap.assert_called_once_with('a')

    def test_unreachable_region_offers_transport_without_an_old_failed_trip(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        elevator = Rule('panel', 'RocketHideoutElevator', 'Elevator:panel', ['sign:1'], [], ['B4F'],
                        ('transport', ('RocketHideoutB4F', 25, 15), True), [])
        agent.index = Mock(rules=[elevator])
        agent.index.frontier.return_value = [elevator]
        groups = {'boss': {'rules': [], 'context': {'trigger_navigation': [
            {'map': 'RocketHideoutB4F', 'tile_route_found': False}]}}}
        self.assertTrue(agent.transport_frontiers(groups, {}))
        option = next(g for k, g in groups.items() if k != 'boss')
        self.assertEqual(option['target'], elevator.effect)
        agent.index.frontier.assert_called_once_with(elevator.effect, {})

    def test_scripted_entrance_can_lead_to_a_target_on_another_map(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        destination = 'SafariZoneSecretHouse'
        agent.maps = {destination: json.loads((MAPS_DIR / destination / 'map.json').read_text())}
        reward = Rule('reward', destination, destination + ':reward', ['npc:1'], [], [],
                      ('item', 'HM03', True), [])
        entrance = Rule('entrance', 'SafariZoneGate', 'Gate:pay', ['npc:1'], [], ['YES'],
                        ('transport', ('SafariZoneCenter', 14, 25), True), [])
        agent.index = SimpleNamespace(rules=[entrance], coordinates=lambda _: [],
                                      frontier=Mock(return_value=[entrance]))
        agent.game = Mock(last_map='FuchsiaCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {'reward': {'rules': [reward], 'context': {'trigger_navigation': [
            {'map': destination, 'tile_route_found': False}]}}}
        self.assertTrue(agent.transport_frontiers(groups, {}))
        agent.index.frontier.assert_called_once_with(entrance.effect, {})
        self.assertIn(json.dumps(entrance.effect), groups)
        agent.game.nav_to_map.assert_not_called()

    def test_transport_to_same_floor_must_reach_the_requested_region(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'RocketHideoutB4F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        # The stairs/key area (19,9) is disconnected from the boss wing.
        transport = Rule('panel', 'RocketHideoutElevator', 'Elevator:panel', ['sign:1'], [], [],
                         ('transport', (name, 19, 9), True), [])
        boss = Rule('boss', name, name + ':boss', ['npc:1'], [], [], ('flag', 'BOSS', True), [])
        agent.index = SimpleNamespace(rules=[transport], coordinates=lambda _: [],
                                      frontier=Mock(return_value=[transport]))
        agent.game = Mock(last_map='CeladonCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {'boss': {'rules': [boss], 'context': {'trigger_navigation': [
            {'map': name, 'tile_route_found': False}]}}}
        self.assertFalse(agent.transport_frontiers(groups, {}))
        agent.index.frontier.assert_not_called()

    def test_navigation_preview_distinguishes_regions_on_the_same_map(self):
        from types import SimpleNamespace
        from openpokered.story_rules import MAPS_DIR
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        name = 'RocketHideoutB4F'
        agent.maps = {name: json.loads((MAPS_DIR / name / 'map.json').read_text())}
        agent.index = SimpleNamespace(rules=[], coordinates=lambda _: [])
        agent.observed_barrier_maps = set()
        agent.navigation_memory = {name: {'goal': ['flag', 'BOSS', True]}}
        agent.game = Mock(last_map='CeladonCity')
        agent.game.navigation_barriers.return_value = {}
        agent.game.live_npcs.return_value = set()
        agent.game.navigation_excluded_maps.return_value = ()
        groups = {label: {'rules': [Rule(label, name, name + ':' + label, [f'npc:{npc}'],
                                        [], [], ('flag', label, True), [])]}
                  for label, npc in [('key', 4), ('boss', 1)]}
        agent.annotate_navigation(groups, {'map': name, 'x': 19, 'y': 9, 'flags': {}})
        self.assertTrue(groups['key']['context']['trigger_navigation'][0]['tile_route_found'])
        self.assertNotIn('boss', groups)  # Its known failure must not suppress the reachable key region.
        agent.game.nav_to_map.assert_not_called()

    def test_battle_preparation_uses_selected_trainer_without_overwriting_causal_context(self):
        from types import SimpleNamespace
        from openpokered.story_agent import DualStoryAgent
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        rule = Rule('key-grunt', 'Base', 'Base:grunt', ['npc:2'], [], [],
                    ('flag', 'GRUNT_BEATEN', True), [('battle', 'Rocket:1', True)])
        agent.maps = {'Base': {'npcs': [
            {'textId': 1, 'trainerClass': 'Boss', 'trainerSet': 1},
            {'textId': 2, 'trainerClass': 'Rocket', 'trainerSet': 1}]}}
        agent.trainers = {'Boss': {'parties': [{'pokemon': [{'species': 'Rhydon', 'level': 50}]}]},
                          'Rocket': {'parties': [{'pokemon': [{'species': 'Zubat', 'level': 21}]}]}}
        self.assertEqual(agent.opponent_parties([rule, rule]), [{'species': 'Zubat', 'level': 21}])
        agent.battle_requirements, agent.field_requirements = {}, {}
        agent.navigation_blockage, agent.navigation_memory = None, {}
        agent.index = SimpleNamespace(rules=[])
        group = {'target': rule.effect, 'rules': [rule], 'objectives': ['Obtain key'],
                 'context': {'required_for': 'elevator'}}
        with patch.object(DualStoryAgent, 'strategy_groups', return_value={'key': group}):
            result = agent.strategy_groups({'party': []})
        self.assertEqual(result['key']['context']['required_for'], 'elevator')
        self.assertEqual(result['key']['context']['opponent_parties'], [{'species': 'Zubat', 'level': 21}])

    def test_stationary_obstruction_survives_map_exit_and_expires_when_hidden(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.judgments = SimpleNamespace(
            maps={'Route12': {'npcs': [{'textId': 1, 'movement': 'Stationary'},
                                      {'textId': 2, 'movement': 'Walk'}]}},
            index=SimpleNamespace(npc_toggles={('Route12', 1): ('SLEEPER', False)}),
            navigation_facts={'flags': {}})
        game.remember_npcs('Route12', [{'text_id': 1, 'x': 10, 'y': 62, 'visible': True},
                                       {'text_id': 2, 'x': 9, 'y': 10, 'visible': True}])
        game._prev_map = 'LavenderTown'
        self.assertEqual(game.navigation_barriers(), {'Route12': {(10, 62)}})
        game._prev_map = 'Route12'
        self.assertEqual(game.navigation_barriers(), {})
        game._prev_map = 'LavenderTown'
        game.judgments.navigation_facts['flags']['__OBJ_HIDDEN_SLEEPER'] = True
        self.assertEqual(game.navigation_barriers(), {})

    def test_scripted_npc_move_and_hide_clear_its_old_patrol_obstacle(self):
        from types import SimpleNamespace
        game = JevGame.__new__(JevGame)
        game.observed_npcs = {}
        game.judgments = SimpleNamespace(maps={'Room': {'npcs': [
            {'textId': 1, 'movement': 'Stationary'}, {'textId': 2, 'movement': 'Walk'}]}})
        game.d = Mock()
        game.d.cmd.return_value = {'data': [{'text_id': 1, 'x': 10, 'y': 62, 'visible': True},
                                          {'text_id': 2, 'x': 9, 'y': 10, 'visible': True}]}
        self.assertIn((10, 62), game.npc_blocked('Room'))
        game.d.cmd.return_value = {'data': [{'text_id': 1, 'x': 10, 'y': 63, 'visible': True},
                                          {'text_id': 2, 'x': 9, 'y': 11, 'visible': True}]}
        blocked = game.npc_blocked('Room')
        self.assertNotIn((10, 62), blocked)
        self.assertTrue({(10, 63), (9, 10), (9, 11)} <= blocked)
        game.d.cmd.return_value['data'][0]['visible'] = False
        blocked = game.npc_blocked('Room')
        self.assertNotIn((10, 63), blocked)
        self.assertTrue({(9, 10), (9, 11)} <= blocked)

    def test_actual_defeat_offers_further_training_but_escape_does_not(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.battle_defeats, agent.defeat_preparation = [], 0
        agent.record = Mock()
        before = {'map_name': 'PewterGym', 'battle_live': {'enemy_party': [{'species': 'Onix'}]}}
        after = {'party': [{'level': 14}], 'battle_phase': 'BattleOver { won: false, escaped: true }'}
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 0)
        after['battle_phase'] = 'TrainerVictory { player_won: false }'
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 16)
        self.assertTrue(agent.should_replan({}))
        self.assertEqual(len(agent.battle_defeats), 1)
        self.assertEqual(after['party'][0]['level'], 14)
        after['battle_phase'] = 'TrainerVictory { player_won: true }'
        agent.observe_battle_result(before, after)
        self.assertEqual(agent.defeat_preparation, 0)
        self.assertTrue(agent.battle_defeats[0]['resolved_by_victory'])

    def test_water_requirement_finds_real_shores_and_restores_land_collision(self):
        import playthrough as pt
        original = pt.walkable, pt.walkable_edge
        state = {'map_name': 'FuchsiaCity', 'player_x': 5, 'player_y': 14}
        obstacle = surf_requirement(state, 'CinnabarGym', [(3, 2), (4, 3)], 'FuchsiaCity')
        self.assertEqual(obstacle['move'], 'Surf')
        self.assertTrue(pt.walkable(obstacle['map'], *obstacle['stance']))
        dx, dy = pt.DELTA[obstacle['direction']]
        x, y = obstacle['stance']
        self.assertTrue(water_tile(obstacle['map'], x+dx, y+dy))
        self.assertFalse(water_tile(*obstacle['landing']))
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        with self.assertRaises(ValueError), water_planning():
            raise ValueError('search failed')
        self.assertEqual((pt.walkable, pt.walkable_edge), original)
        self.assertTrue(hm_compatible('LAPRAS', 'Surf'))
        self.assertFalse(hm_compatible('VENUSAUR', 'Surf'))

    def test_water_requirement_skips_shortcuts_with_existing_land_routes(self):
        import playthrough as pt
        state = {'map_name': 'Route12', 'player_x': 10, 'player_y': 10}
        obstacle = surf_requirement(state, 'CinnabarGym', [(3, 2), (4, 3)], 'Route12')
        self.assertIsNotNone(obstacle)
        self.assertIsNotNone(pt.bfs_cross('Route12', (10, 10), obstacle['map'], tuple(obstacle['stance']),
            last_map='Route12', allow_ledges=True, allow_spinners=True))
        landing = obstacle['landing']
        self.assertIsNone(pt.bfs_cross('Route12', (10, 10), landing[0], tuple(landing[1:]),
            last_map='Route12', allow_ledges=True, allow_spinners=True))

    def test_observed_gate_barrier_allows_detour_and_expires_with_prerequisite(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.observed_barrier_maps = {'Route18Gate1F'}
        guard = {'Call': {'callee': 'game.hasItem', 'args': [{'StringLit': 'BICYCLE'}]}}
        rule = Rule('gate', 'Route18Gate1F', 'gate:check', [], [(guard, False)], [],
                    ('movement', 'push', True), [])
        agent.index = Mock(rules=[rule])
        agent.index.coordinates.return_value = [(4, y) for y in range(3, 7)]
        barriers = agent.observed_navigation_barriers({'bag': {}})
        self.assertEqual(barriers['Route18Gate1F'], {(4, y) for y in range(3, 7)})
        path = pt.bfs_cross('Route18Gate1F', (5, 3), 'PokemonFanClub', (2, 1),
                            last_map='Route18', allow_ledges=True, blocked_maps=barriers)
        self.assertIsNotNone(path)
        self.assertFalse(any(node[0] == 'Route18Gate1F' and tuple(node[1:]) in barriers['Route18Gate1F']
                             for node, _ in path[1:]))
        self.assertEqual(agent.observed_navigation_barriers({'bag': {'BICYCLE': 1}}), {})
        agent.observed_barrier_maps.clear()
        self.assertEqual(agent.observed_navigation_barriers({'bag': {}}), {})

    def test_shifted_final_protocol_responses_are_not_resumable_evidence(self):
        data = {'get_state': {'screen': 'overworld'}, 'get_flags': {'EVENT_A': True},
                'get_party': [{'species': 'Venusaur'}], 'get_bag': [{'item': 'PokeFlute'}],
                'get_npcs': [{'text_id': 1}]}
        observations = {cmd: {'ok': True, 'data': value} for cmd, value in data.items()}
        self.assertTrue(observations_valid(observations))
        observations['get_bag'] = observations['get_party']
        self.assertFalse(observations_valid(observations))

    def test_soft_stop_waits_for_the_current_protocol_round_trip(self):
        raw = Mock()
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        def finish(**kwargs):
            protocol.stop_requested = True
            return {'ok': True, 'data': {'screen': 'overworld'}}
        raw.cmd.side_effect = finish
        self.assertEqual(protocol.cmd(cmd='get_state')['data']['screen'], 'overworld')
        with self.assertRaisesRegex(StoryStopped, 'command_boundary'):
            protocol.cmd(cmd='get_flags')
        raw.cmd.assert_called_once()

    def test_observed_unidentified_ghost_escapes_and_records_story_requirement(self):
        import playthrough as pt
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.battle_requirements = {}
        game.battles_driven = 0
        before = {'screen': 'battle', 'map_name': 'Tower', 'battle_live': {'is_ghost': True},
                  'script_awaiting_battle': True, 'party': []}
        after = {'screen': 'overworld', 'map_name': 'Tower', 'battle_phase': 'Over',
                 'party': [], 'frame_count': 100}
        game.st = Mock(side_effect=[before, after])
        with patch.object(pt.Game, 'battle_loop') as drive:
            game.battle_loop(prefer='fight')
        self.assertEqual(drive.call_args.kwargs['prefer'], 'run')
        self.assertIn('SILPH_SCOPE', game.judgments.battle_requirements)
        game.judgments.choose.assert_called_once()

    def test_failed_tile_route_reports_the_object_occupying_its_passage(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'PokemonTower6F', 'player_x': 18, 'player_y': 9}
        agent.game.last_map = 'LavenderTown'
        agent.game.navigation_excluded_maps.return_value = ()
        agent.game.nav_to_map.side_effect = pt.NavError('blocked passage')
        agent.game.live_npcs.return_value = {(16, 5), (6, 8), (14, 14)}
        agent.maps = {'PokemonTower6F': {'npcs': [{'textId': 3, 'isTrainer': True}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'text_id': 3, 'x': 16, 'y': 5, 'visible': True},
                                         {'text_id': 4, 'x': 6, 'y': 8, 'visible': True},
                                         {'text_id': 5, 'x': 14, 'y': 14, 'visible': True}]
        result = agent.travel('PokemonTower7F', Rule('', '', '', [], [], [], (), []), [(10, 4)])
        self.assertEqual(result['blocking_npcs'], [4])
        self.assertEqual(result['blocking_trainers'], [])
        self.assertEqual(result['result'], 'blocked')

    def test_npc_barrier_still_offers_a_reachable_alternative_cut_passage(self):
        import playthrough as pt
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.game = Mock()
        agent.game.st.return_value = {'map_name': 'Route12', 'player_x': 0, 'player_y': 62}
        agent.game.last_map = 'Route12'
        agent.game.navigation_excluded_maps.return_value = ('SaffronCity',)
        agent.game.nav_to_map.side_effect = pt.NavError('blocked by sleeping Pokemon')
        agent.game.live_npcs.return_value = {(10, 62), (9, 52)}
        agent.maps = {'Route12': {'npcs': [{'textId': 3, 'isTrainer': True}]}}
        agent.client = Mock()
        agent.client.cmd.return_value = [{'text_id': 3, 'x': 9, 'y': 52, 'visible': True},
                                         {'text_id': 1, 'x': 10, 'y': 62, 'visible': True}]
        agent.field_requirements = {}
        result = agent.travel('Route8', Rule('', '', '', [], [], [], (), []), [(41, 11)])
        self.assertEqual(result['blocking_npcs'], [1])
        self.assertEqual(result['blocking_trainers'], [])
        self.assertEqual(result['field_obstruction']['map'], 'Route9')
        self.assertEqual(agent.field_requirements['Cut']['tree'], [5, 8])

    def test_regrown_tree_invalidates_clearance_during_an_intermediate_map_visit(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.cleared_terrain = {'Route9,5,8', 'CeladonCity,35,32'}
        agent.invalidate_terrain({'map_name': 'Route9'})
        self.assertEqual(agent.cleared_terrain, {'CeladonCity,35,32'})

    def test_machine_boot_and_teach_confirmation_precede_party_selection(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        menus = [
            {'kind': 'bag', 'phase': 'Browsing', 'cursor': 0, 'items': [{'item': 'Hm01'}]},
            {'kind': 'bag', 'phase': 'ActionMenu { cursor: 0 }'},
            {'kind': 'bag', 'phase': 'MachineBoot { item: Hm01 }'},
            {'kind': 'bag', 'phase': 'MachineTeach { item: Hm01, cursor: 0 }'},
            {'kind': 'party', 'phase': 'Browsing', 'cursor': 0},
            {'kind': 'party', 'phase': 'ChooseMove { cursor: 0 }', 'known_moves': ['Tackle', 'Growl']},
            {'kind': 'party', 'phase': 'ChooseMove { cursor: 1 }', 'known_moves': ['Tackle', 'Growl']},
            {'kind': 'party', 'phase': 'ItemUseNotice { wait_frames: 0 }'},
            None,
        ]
        snapshots = iter({'field_menu': menu, 'party': [{'moves': ['Tackle', 'Cut' if i >= 7 else 'Growl']}]}
                         for i, menu in enumerate(menus))
        game.st = lambda: next(snapshots)
        game.tap = Mock()
        game.learn_machine('Hm01', 'Cut', 0, 'Growl')
        self.assertEqual([call.args[0] for call in game.tap.call_args_list],
                         ['a', 'a', 'a', 'a', 'a', 'down', 'a', 'b'])

    def test_cut_obstruction_is_derived_without_changing_planning_maps(self):
        import playthrough as pt
        before = {name: tuple(data['blocks']) for name, data in pt.MAPS.items()}
        obstacle = cut_requirement({'map_name': 'CeruleanCity', 'player_x': 19, 'player_y': 18},
                                   'VermilionGym', [(5, 2)], 'CeruleanCity')
        self.assertEqual(obstacle['move'], 'Cut')
        self.assertEqual(obstacle['destination'], 'VermilionGym')
        self.assertEqual(before, {name: tuple(data['blocks']) for name, data in pt.MAPS.items()})
        self.assertTrue(hm_compatible('Ivysaur', 'Cut'))
        self.assertFalse(hm_compatible('Venusaur', 'Strength'))

    def test_champion_flag_is_not_durable_first_clear_proof(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.first_clear_verification = None
        objective = {'id': 'become-champion',
                     'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'}}
        facts = {'flags': {'EVENT_BEAT_CHAMPION_RIVAL': True}}
        self.assertFalse(agent.objective_satisfied(objective, facts))
        agent.first_clear_verification = {'separate_process_continue': {}}
        facts['flags'].clear()  # The transient flag resets in Hall of Fame.
        self.assertTrue(agent.objective_satisfied(objective, facts))

    def test_interrupted_route_does_not_make_unreached_maps_visited(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.visited = {'PewterCity'}
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'Route3'}
        agent.record_travel({'result': 'entered_battle', 'legs_completed': 1,
                             'legs': [{'from_map': 'PewterCity', 'to_map': 'Route3'},
                                      {'from_map': 'Route3', 'to_map': 'Route4'}]})
        self.assertEqual(agent.visited, {'PewterCity', 'Route3'})

    def test_training_grass_is_reachable_from_current_route_two_section(self):
        import playthrough as pt
        spot = reachable_grass('Route2', (8, 71))
        self.assertIsNotNone(spot)
        self.assertGreater(spot[1], 40)
        self.assertTrue(pt.bfs('Route2', (8, 71), spot, pt.warp_tiles('Route2')))

    def test_nurse_is_approached_across_the_canonical_counter_tile(self):
        self.assertIn(((3, 3), 'up'), list(counter_approaches('ViridianPokecenter', {'x': 3, 'y': 1})))
        self.assertEqual(list(counter_approaches('OaksLab', {'x': 5, 'y': 2})), [])

    def test_rejects_state_shortcuts_before_they_reach_game(self):
        raw = Mock()
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        for command in ('warp', 'give_pokemon', 'give_item', 'set_flag', 'restore_state', 'save'):
            with self.assertRaisesRegex(StoryStopped, 'forbidden_debug_command'):
                protocol.cmd(cmd=command)
        raw.cmd.assert_not_called()

    def test_neutral_input_and_frame_count_are_forwarded(self):
        raw = Mock()
        raw.cmd.return_value = {'ok': True, 'data': {'advanced': True, 'queue_start_frame': 100, 'frame_count': 113}}
        protocol = ObservedProtocol(raw, Mock(), time.monotonic()+60)
        protocol.drive([None, 'a', None], frames=13)
        raw.cmd.assert_called_once_with(cmd='press_timeline', buttons=[None, 'a', None]+[None]*10, advance=True)
        raw.cmd.return_value['data']['frame_count'] = 114
        with self.assertRaisesRegex(StoryStopped, 'input_timeline_not_advanced_atomically'):
            protocol.drive(['b']*13)

    def state(self):
        return {'battle_live': {
            'player': {'species': 'Bulbasaur', 'hp': 20, 'max_hp': 30},
            'enemy': {'species': 'Onix', 'hp': 30, 'max_hp': 30}}}

    def test_move_cache_changes_when_pp_or_disable_changes(self):
        menu = {'moves': [{'move': 'Tackle', 'pp': 10, 'disabled': False},
                          {'move': 'VineWhip', 'pp': 10, 'disabled': False}]}
        before, options = move_question(self.state(), menu)
        self.assertEqual(before['moves']['1']['effectiveness'], 4)
        menu['moves'][1]['pp'] = 2
        low, _ = move_question(self.state(), menu)
        self.assertNotEqual(json.dumps(before, sort_keys=True), json.dumps(low, sort_keys=True))
        menu['moves'][1]['disabled'] = True
        after, options = move_question(self.state(), menu)
        self.assertNotIn('1', options)
        self.assertNotIn('1', after['moves'])

    def test_high_critical_attack_exposes_its_expected_advantage(self):
        state = {'battle_live': {
            'player': {'species': 'Charizard', 'level': 60, 'hp': 186, 'max_hp': 186},
            'enemy': {'species': 'Lapras', 'level': 40, 'hp': 150, 'max_hp': 150}}}
        menu = {'moves': [{'move': name, 'pp': 10, 'disabled': False}
                          for name in ['Strength', 'Slash']]}
        compact, _ = move_question(state, menu)
        self.assertAlmostEqual(compact['moves']['1']['critical_probability_without_focus_energy'], 255/256, places=4)
        self.assertGreater(compact['moves']['1']['effective_expected_power'],
                           1.4 * compact['moves']['0']['effective_expected_power'])

    def test_machine_replacement_preserves_stronger_same_type_attack(self):
        mon = {'species': 'Charizard', 'level': 60,
               'moves': ['Flamethrower', 'Cut', 'FireSpin', 'Slash']}
        self.assertNotIn('Flamethrower', replacement_options(mon, 'Toxic'))
        self.assertIn('FireSpin', replacement_options(mon, 'Toxic'))
        self.assertNotIn('Cut', replacement_options(mon, 'Toxic'))
        self.assertIn('Flamethrower', replacement_options(mon, 'FireBlast'))

    def test_medicine_options_match_actual_injury_and_status(self):
        party = [{'hp': 40, 'max_hp': 40, 'status': 'Sleep(2)'},
                 {'hp': 0, 'max_hp': 70, 'status': 'None'}]
        bag = {'FullRestore': 2, 'HyperPotion': 1, 'Antidote': 1, 'Revive': 1}
        offered = {(item, index) for item, index, _ in medicine_options(party, bag)}
        self.assertEqual(offered, {('FullRestore', 0), ('Revive', 1)})
        party[0]['hp'] = 20
        self.assertIn(('HyperPotion', 0), {(item, index) for item, index, _ in medicine_options(party, bag)})

    def test_cave_training_uses_ordinary_floor_but_avoids_exit_warps(self):
        self.assertTrue(training_tile('VictoryRoad2F', 2, 8))
        self.assertFalse(training_tile('VictoryRoad2F', 0, 8))
        self.assertIsNotNone(reachable_grass('VictoryRoad2F', (0, 8)))

    def test_level_up_replacement_respects_one_judgment_across_confirmation(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'skip'
        game.tap = Mock()
        state = {'party': [{'species': 'Charizard', 'level': 55,
                            'moves': ['Flamethrower', 'Cut', 'Slash', 'Strength']}],
                 'battle_phase': 'LearnMoveAsk { party_index: 0, move_id: FireSpin, resume: PlayerMenu }'}
        game.learn_move(state)
        game.tap.assert_called_with('b', 8)
        state['battle_phase'] = state['battle_phase'].replace('LearnMoveAsk', 'LearnMoveGiveUpConfirm')
        game.learn_move(state)
        self.assertEqual(game.judgments.choose.call_count, 1)
        self.assertEqual([call.args[0] for call in game.tap.call_args_list], ['b', 'up', 'a'])

    def test_immune_active_battler_offers_a_conscious_teammate(self):
        game = JevGame.__new__(JevGame)
        game.judgments = Mock()
        game.judgments.choose.return_value = 'switch:1'
        party = [{'species': 'Charizard', 'level': 67, 'hp': 80, 'max_hp': 210,
                  'moves': ['Slash', 'FireBlast'], 'pp': [10, 0], 'status': 'None'},
                 {'species': 'Lapras', 'level': 16, 'hp': 67, 'max_hp': 67,
                  'moves': ['Surf'], 'pp': [15], 'status': 'None'}]
        state = {'party': party, 'battle_inventory': [], 'battle_live': {
            'player': party[0], 'enemy': {'species': 'Gengar'}, 'player_party': party}}
        self.assertEqual(game.battle_recovery_plan(state), ('switch', 1))
        self.assertIn('switch:1', game.judgments.choose.call_args.args[2])

    def test_failed_battle_preparation_changes_with_pp_or_recovery_stock(self):
        party = [{'species': 'Charizard', 'level': 65, 'hp': 203, 'max_hp': 203,
                  'moves': ['FireBlast', 'Slash'], 'pp': [1, 15], 'status': 'None'}]
        before = battle_readiness(party, {'NUGGET': 1})
        self.assertEqual(before, battle_readiness(party, {}))
        self.assertNotEqual(before, battle_readiness(party, {'FULLRESTORE': 1}))
        party[0]['pp'] = [5, 20]
        self.assertNotEqual(before, battle_readiness(party, {}))

    def test_training_navigation_remembers_the_same_barrier_as_story_travel(self):
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.client = Mock()
        agent.client.state.return_value = {'map_name': 'PewterCity', 'player_x': 35, 'player_y': 17}
        agent.active = {'target': ('level', 'leader', 14)}
        agent.navigation_memory, agent.navigation_history = {}, {}
        agent.observed_barrier_maps = set()
        agent.remember_travel_result('Route3', {'result': 'blocked', 'detail': 'A guide returns the player'})
        self.assertEqual(agent.navigation_memory['Route3']['map'], 'PewterCity')
        self.assertIn('PewterCity', agent.observed_barrier_maps)
        agent.remember_travel_result('Route3', {'result': 'reached'})
        self.assertNotIn('Route3', agent.navigation_memory)

    def test_entry_autowalk_does_not_make_one_way_exit_guard_block_entry(self):
        from types import SimpleNamespace
        entry = Rule('entry', 'Room', 'Room:@load', ['load'],
                     [({'Call': {'callee': 'getFlag', 'args': [{'StringLit': 'ENTERED'}]}}, False)],
                     [], ('movement', 'movePlayerRelative', True), [])
        gate = Rule('gate', 'Room', 'Room:exit', ['coord:exit'], [], [],
                    ('movement', 'movePlayerRelative', True), [])
        agent = AutonomousStoryAgent.__new__(AutonomousStoryAgent)
        agent.index = SimpleNamespace(rules=[entry, gate], coordinates=lambda r: [(4, 10)] if r.id == 'gate' else [])
        agent.observed_barrier_maps = {'Room'}
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Lobby', 'flags': {}}), {})
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Room', 'flags': {'ENTERED': True}}),
                         {'Room': {(4, 10)}})
        self.assertEqual(agent.observed_navigation_barriers({'map': 'Lobby', 'flags': {'ENTERED': True}}),
                         {'Room': {(4, 10)}})

    def test_depleted_attacks_require_recovery_even_at_full_hp(self):
        facts = {'party': [{'hp': 30, 'max_hp': 30, 'status': 'None',
                            'moves': ['Tackle', 'Growl'], 'pp': [0, 40]}]}
        self.assertTrue(AutonomousStoryAgent.needs_healing(facts))
        facts['party'][0]['pp'][0] = 35
        self.assertFalse(AutonomousStoryAgent.needs_healing(facts))
        # Total PP can hide the depletion of the only suitable attack.
        facts['party'][0]['moves'] = ['Cut', 'RazorLeaf', 'VineWhip']
        facts['party'][0]['pp'] = [4, 8, 10]
        self.assertTrue(AutonomousStoryAgent.needs_healing(facts))


if __name__ == '__main__':
    unittest.main()
