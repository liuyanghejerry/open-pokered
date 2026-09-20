"""Autonomous skill contracts: real-input boundary and grounded preparation."""
import json
import sys
import time
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openpokered.playthrough_judgments import ObservedProtocol, move_question, JevGame, replacement_options, medicine_options
from openpokered.autonomous_story import AutonomousStoryAgent, counter_approaches, reachable_grass, training_tile, battle_readiness
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
