"""Behavior checks for condition-preserving, two-layer story exploration."""
import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
from openpokered.story_rules import compile_story, evaluate, literal, requirements, StoryIndex, DEFAULT_HIDDEN, Rule, trainer_victory_rules, MAPS_DIR
from openpokered.story_agent import DualStoryAgent, StoryStopped, progress_key, attempt_key
from openpokered.typesafe import ChoiceAnswer, SystemOneResult, TypeSafeError
from test_openpokered_judgment import FakeEnv, StubJudge, OBS, OBJECTIVES
from openpokered.judgment_agent import JudgmentAgent


def call(name,*args):
    return {'Call':{'callee':name,'args':[literal(a) for a in args]}}


def command(name,*args):
    return {'Command':{'name':name,'args':[literal(a) for a in args]}}


def conditional(expr,yes,no=()):
    return {'If':{'condition':expr,'then_branch':list(yes),'else_branch':list(no)}}


def story(program):
    return {'id':'Mart:@load','map':'Mart','triggers':['load'],'program':program}


def facts(**flags):
    return {'flags':flags,'bag':{},'party':[],'badges':0}


class RulesTests(unittest.TestCase):
    def test_game_version_uses_only_valid_explicit_native_observation(self):
        for version in (0, 1):
            for name in ('getGameVersion', 'game.getGameVersion'):
                self.assertEqual(evaluate(call(name), {'game_version': version}), version)
        for version in (None, False, True, -1, 2, 'Red', '0', 0.0, []):
            self.assertIsNone(evaluate(call('getGameVersion'), {'game_version': version}))
        self.assertIsNone(evaluate(call('getGameVersion'), {}))
        self.assertIsNone(evaluate(call('getGameVersion', 0), {'game_version': 0}))

    def test_prize_version_guard_keeps_red_blue_and_unknown_distinct(self):
        expr = {'BinaryOp': {'op': 'Eq', 'left': call('getGameVersion'), 'right': literal(0)}}
        self.assertEqual(requirements(expr, True, {'game_version': 0}), [[]])
        self.assertEqual(requirements(expr, False, {'game_version': 1}), [[]])
        for state, wanted in (({}, True), ({}, False), ({'game_version': 1}, True),
                              ({'game_version': 0}, False)):
            self.assertEqual(requirements(expr, wanted, state)[0][0][0], 'unknown')

    def test_party_count_reads_the_observed_roster_without_assuming_an_empty_party(self):
        for size in range(7):
            for name in ('getPartyCount', 'game.getPartyCount'):
                self.assertEqual(evaluate(call(name), {'party': [{}] * size}), size)
        for state in ({}, {'party': None}, {'party': {}}, {'party': 'six'},
                      {'party': [{}] * 7}):
            self.assertIsNone(evaluate(call('getPartyCount'), state))
        self.assertIsNone(evaluate(call('getPartyCount', 0), {'party': []}))

    def test_scene_party_room_guard_becomes_a_real_deposit_prerequisite(self):
        full = {'party': [{}] * 6}
        room = {'party': [{}] * 5}
        for op, bound, wanted in (('Gte', 6, False), ('Lt', 6, True),
                                  ('Gt', 5, False), ('Lte', 5, True)):
            expr = {'BinaryOp': {'op': op, 'left': call('game.getPartyCount'),
                                 'right': literal(bound)}}
            self.assertEqual(requirements(expr, wanted, full), [[('party_space', 'party', True)]])
            self.assertEqual(requirements(expr, wanted, room), [[]])
            self.assertEqual(requirements(expr, wanted, {})[0][0][0], 'unknown')
        index = StoryIndex.__new__(StoryIndex)
        self.assertTrue(index.satisfied(('party_space', 'party', True), room))
        self.assertFalse(index.satisfied(('party_space', 'party', True), full))
        self.assertFalse(index.satisfied(('party_space', 'party', True), {}))
        self.assertFalse(index.satisfied(('party_space', 'party', False), {}))

    def test_other_party_count_thresholds_are_not_replaced_by_a_free_slot(self):
        # Daycare's minimum-party guard is not a gift-capacity prerequisite.
        for op, bound, wanted in (('Lte', 1, False), ('Gte', 3, False),
                                  ('Eq', 5, True), ('Gte', 6, True)):
            expr = {'BinaryOp': {'op': op, 'left': call('getPartyCount'),
                                 'right': literal(bound)}}
            state = {'party': [{}] * (1 if op == 'Lte' else 5 if wanted and op == 'Gte' else 6)}
            self.assertEqual(requirements(expr, wanted, state)[0][0][0], 'unknown')

    def test_native_ending_transport_requires_the_real_ceremony_and_credits(self):
        from openpokered.story_rules import native_ending_destination
        self.assertEqual(native_ending_destination(), ('PalletTown', 5, 6))
        rules = compile_story(story([command('resetFlag', 'ROUND_WON'), command('enterHallOfFame')]))
        self.assertEqual(rules[-1].effect, ('transport', native_ending_destination(), True))
        self.assertIn(('ending', 'hall_of_fame_and_credits', True), rules[-1].preceding)
        self.assertIn(('flag', 'ROUND_WON', False), rules[-1].preceding)

    def test_badge_queries_use_native_bits_not_count_or_story_flags(self):
        from openpokered.story_rules import BADGE_BITS
        for name, bit in BADGE_BITS.items():
            query = call('game.hasBadge', name.lower())
            self.assertTrue(evaluate(query, {**facts(), 'badges': 1, 'badge_bits': 1 << bit}))
            self.assertFalse(evaluate(query, {**facts(EVENT_BEAT_ERIKA=True),
                                             'badges': 7, 'badge_bits': 255 ^ (1 << bit)}))
        self.assertIsNone(evaluate(call('hasBadge', 'RAINBOWBADGE'), facts(EVENT_BEAT_ERIKA=True)))
        self.assertIsNone(evaluate(call('hasBadge', 'INVALID'), {'badge_bits': 255}))
        self.assertEqual(requirements(call('hasBadge', 'RAINBOWBADGE'), True,
                                     {**facts(), 'badge_bits': 0}),
                         [[('badge', 'RAINBOWBADGE', True)]])

    def test_badge_guard_backchains_real_give_badge_producer(self):
        earned = compile_story(story([conditional(call('getFlag', 'WON_GYM'),
            [command('giveBadge', 'RAINBOWBADGE')])]))
        index = StoryIndex.__new__(StoryIndex)
        index.by_effect = {('badge', 'RAINBOWBADGE', True): earned}
        missing = {**facts(WON_GYM=True), 'badge_bits': 0}
        self.assertEqual(index.frontier(('badge', 'RAINBOWBADGE', True), missing), earned)
        owned = {**missing, 'badge_bits': 8}
        self.assertTrue(index.satisfied(('badge', 'RAINBOWBADGE', True), owned))
        self.assertEqual(index.frontier(('badge', 'RAINBOWBADGE', True), owned), [])
        self.assertFalse(index.satisfied(('badge', 'RAINBOWBADGE', False), facts()))
        guard = compile_story(story([conditional(call('hasBadge', 'RAINBOWBADGE'), [],
            [command('movePlayerRelative', 'down')])]))[0]
        self.assertEqual(guard.missing(missing), [])
        self.assertNotEqual(guard.missing(owned), [])

    def test_rival_triplet_base_survives_into_following_effect(self):
        rules = compile_story(story([
            command('startBattleSet', 'OPP_RIVAL2', 6), command('setFlag', 'RIVAL_BEATEN')]))
        self.assertEqual(rules[-1].preceding, [('battle', ('OPP_RIVAL2', 6), True)])
        ordinary = compile_story(story([
            command('startBattle', 'OPP_ROCKET7'), command('setFlag', 'ROCKET_BEATEN')]))
        self.assertEqual(ordinary[-1].preceding, [('battle', 'OPP_ROCKET7', True)])

    def test_pokedex_count_gate_becomes_a_pursuable_goal(self):
        from openpokered.story_rules import requirements, StoryIndex
        expr = {'BinaryOp': {'op': 'Gte', 'left': call('getPokedexOwnedCount'),
                             'right': {'NumberLit': 10.0}}}
        self.assertEqual(requirements(expr, True, facts()), [[('dex', 'count', 10)]])
        self.assertEqual(requirements(expr, True, {**facts(), 'dex': {'owned': 10}}), [[]])
        index = StoryIndex.__new__(StoryIndex)
        self.assertFalse(index.satisfied(('dex', 'count', 10), facts()))
        self.assertTrue(index.satisfied(('dex', 'count', 10), {**facts(), 'dex': {'owned': 10}}))

    def test_explore_target_is_satisfied_by_standing_on_the_map(self):
        from openpokered.story_rules import StoryIndex
        index = StoryIndex.__new__(StoryIndex)
        self.assertTrue(index.satisfied(('explore', 'Route3', True), {**facts(), 'map': 'Route3'}))
        self.assertFalse(index.satisfied(('explore', 'Route3', True), {**facts(), 'map': 'Route2'}))

    def test_catch_target_tracks_the_owned_species_list(self):
        from openpokered.story_rules import StoryIndex
        index = StoryIndex.__new__(StoryIndex)
        index._wild_cache = {'Route1': {'Pidgey', 'Rattata'}, 'IndigoPlateau': set()}
        dex = {'owned': 2, 'owned_species': ['Pidgey', 'Rattata']}
        self.assertTrue(index.satisfied(('catch', 'Route1', True), {**facts(), 'dex': dex}))
        partial = {'owned': 1, 'owned_species': ['Pidgey']}
        self.assertFalse(index.satisfied(('catch', 'Route1', True), {**facts(), 'dex': partial}))
        self.assertFalse(index.satisfied(('catch', 'IndigoPlateau', True), {**facts(), 'dex': dex}))
        self.assertFalse(index.satisfied(('catch', 'Route1', True), facts()))

    def test_catch_completion_uses_exact_method_and_rod(self):
        from openpokered.story_rules import StoryIndex, MAPS_DIR
        index = StoryIndex.__new__(StoryIndex)
        index.maps_dir, index._wild_cache = MAPS_DIR, {}
        cases = {
            'water:Route19': {'Tentacool'},
            'fishing:OldRod:VermilionCity': {'Magikarp'},
            'fishing:GoodRod:VermilionCity': {'Goldeen', 'Poliwag'},
            'fishing:SuperRod:VermilionCity': {'Krabby', 'Shellder'},
            'safari:SafariZoneCenter': index.wild_species('SafariZoneCenter'),
            'grass:Route1': {'Pidgey', 'Rattata'},
        }
        for key, expected in cases.items():
            with self.subTest(key=key):
                self.assertTrue(expected)
                self.assertEqual(index.wild_species(key), expected)
                owned = {**facts(), 'dex': {'owned_species': sorted(expected)}}
                self.assertTrue(index.satisfied(('catch', key, True), owned))
                self.assertFalse(index.satisfied(('catch', key, True), facts()))
                owned['collection_audit_pending'] = [next(iter(expected))]
                self.assertFalse(index.satisfied(('catch', key, True), owned))
        # A completed water target never borrows unrelated grass/rod species.
        water = {**facts(), 'dex': {'owned_species': ['Tentacool']}}
        self.assertTrue(index.satisfied(('catch', 'water:Route19', True), water))
        self.assertFalse(index.satisfied(('catch', 'Route19', True), water))
        for key in ('unknown:Route19', 'fishing:MissingRod:Route19',
                    'fishing:OldRod:MissingMap', 'water:MissingMap'):
            self.assertEqual(index.wild_species(key), set())
            self.assertFalse(index.satisfied(('catch', key, True), water))

    def test_lower_floor_boulder_backchains_the_matching_stone_from_above(self):
        floors = [('SeafoamIslandsB1F', 'EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE', 'SEAFOAM_ISLANDS_B1F_OBJ_1'),
                  ('SeafoamIslandsB2F', 'EVENT_SEAFOAM2_BOULDER1_DOWN_HOLE', 'SEAFOAM_ISLANDS_B2F_OBJ_1')]
        programs = {name: {'storylines': [{'id': name + ':@load', 'map': name, 'triggers': ['load'],
            'program': [conditional(call('getFlag', flag), [command('showObject', toggle)])]}]}
            for name, flag, toggle in floors}
        client = Mock()
        client.script_semantics.side_effect = lambda name=None: programs[name] if name else {'maps': list(programs)}
        index = StoryIndex(client)
        target = ('flag', 'EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE', True)
        expected = ('flag', 'EVENT_SEAFOAM1_BOULDER1_DOWN_HOLE', True)
        self.assertEqual([r.effect for r in index.frontier(target, facts())], [expected])
        # The other stone cannot satisfy this hole's geometric prerequisite.
        wrong = facts(__OBJ_SHOWN_SEAFOAM_ISLANDS_B2F_OBJ_2=True)
        self.assertEqual([r.effect for r in index.frontier(target, wrong)], [expected])
        ready = facts(__OBJ_SHOWN_SEAFOAM_ISLANDS_B2F_OBJ_1=True)
        self.assertEqual([r.effect for r in index.frontier(target, ready)], [target])

    def test_shop_stock_keeps_script_access_conditions(self):
        stock = {'ArrayLit': [literal('HYPER_POTION'), literal('REVIVE')]}
        program = [conditional(call('getFlag', 'SHOP_OPEN'),
                    [{'Command': {'name': 'openShop', 'args': [stock]}}])]
        rule = compile_story(story(program))[0]
        self.assertEqual(rule.effect, ('shop', ('HYPER_POTION', 'REVIVE'), True))
        self.assertEqual(rule.missing(facts()), [('flag', 'SHOP_OPEN', True)])
        self.assertEqual(rule.missing(facts(SHOP_OPEN=True)), [])

    def test_absolute_current_movement_retains_its_release_condition(self):
        rules = compile_story(story([conditional(call('getFlag', 'CURRENT_BLOCKED'), [],
                                   [command('movePlayer', [[15, 9], [20, 17]])])]))
        movement = next(r for r in rules if r.effect[0] == 'movement')
        self.assertEqual(movement.missing(facts()), [])
        guard, wanted = movement.guards[0]
        self.assertEqual(requirements(guard, not wanted, facts()), [[('flag', 'CURRENT_BLOCKED', True)]])

    def test_engine_trainer_outcome_unlocks_dialogue_only_reward(self):
        name = 'RocketHideoutB4F'
        configs = {name: json.loads((MAPS_DIR / name / 'script_config.json').read_text())}
        rules = trainer_victory_rules(MAPS_DIR, configs, {name})
        flag = 'EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_2'
        trainer = next(r for r in rules if r.effect == ('flag', flag, True))
        self.assertEqual(trainer.triggers, ['npc:4'])  # Giovanni is script-owned, outside the header order.
        reward = Rule('reward', name, name + ':reward', ['npc:4'],
                      [(call('getFlag', flag), True)], [], ('item', 'LIFT_KEY', True), [])
        index = StoryIndex.__new__(StoryIndex)
        index.by_effect = {r.effect: [r] for r in [*rules, reward]}
        self.assertEqual(index.frontier(reward.effect, facts()), [trainer])
        self.assertEqual(index.frontier(reward.effect, facts(**{flag: True})), [reward])

    def test_elevator_floors_retain_item_guard_and_correct_arrival(self):
        menu = {'Assign': {'name': 'floor', 'value': {'Call': {
            'callee': 'elevatorMenu', 'args': [{'ArrayLit': [literal('B1F'), literal('B4F')]}]}}}}
        branches = [conditional({'BinaryOp': {'op': 'Eq', 'left': {'Variable': 'floor'},
                                              'right': literal(i)}},
                                [command('warpTo', name, 25, y)])
                    for i, name, y in [(0, 'UpperFloor', 19), (1, 'LowerFloor', 15)]]
        rules = compile_story(story([conditional(call('hasItem', 'LIFT_KEY'), [menu, *branches])]))
        self.assertEqual({r.effect[1] for r in rules}, {('UpperFloor', 25, 19), ('LowerFloor', 25, 15)})
        lower = next(r for r in rules if r.effect[1][0] == 'LowerFloor')
        self.assertEqual(lower.choices, ['B4F'])
        self.assertEqual(lower.missing(facts()), [('item', 'LIFT_KEY', True)])
        self.assertEqual(lower.missing({**facts(), 'bag': {'LIFTKEY': 1}}), [])
        index = StoryIndex.__new__(StoryIndex)
        self.assertTrue(index.satisfied(lower.effect, {'map': 'LowerFloor', 'x': 25, 'y': 15}))

    def test_independent_visibility_branches_keep_their_own_guards_without_path_explosion(self):
        program = [conditional(call('getFlag', f'DONE_{i}'), [command('hideObjectByName', f'NPC_{i}')])
                   for i in range(12)]
        rules = compile_story(story(program))
        self.assertEqual(len(rules), 12)
        self.assertEqual(rules[-1].missing(facts()), [('flag', 'DONE_11', True)])
        self.assertEqual(rules[-1].missing(facts(DONE_11=True)), [])

    def test_opening_geometry_retains_key_requirement_and_checks_observed_block(self):
        rules = compile_story(story([conditional(call('hasItem', 'CARD_KEY'),
                                   [command('replaceTileBlock', 3, 6, 14)])]))
        rule = rules[0]
        self.assertEqual(rule.effect, ('block', 'Mart,3,6', 14))
        self.assertEqual(rule.missing(facts()), [('item', 'CARD_KEY', True)])
        index = StoryIndex.__new__(StoryIndex)
        self.assertFalse(index.satisfied(rule.effect, facts()))
        self.assertTrue(index.satisfied(rule.effect, {**facts(), 'block_values': {'Mart,3,6': 14}}))

    def test_sram_defaults_cover_npcs_without_config_default_hidden(self):
        self.assertIn('MRFUJISHOUSE_MR_FUJI', DEFAULT_HIDDEN)
        self.assertIn('MR_FUJI', DEFAULT_HIDDEN)
        self.assertIn('ROCKET_HIDEOUT_B4F_OBJ_8', DEFAULT_HIDDEN)
        self.assertNotIn('POKEMONTOWER7F_MR_FUJI', DEFAULT_HIDDEN)

    def test_live_visibility_overrides_default_but_not_later_script_changes(self):
        condition = {'Visible': ['NPC', True]}
        f = {**facts(), 'object_visibility': {'NPC': True}}
        self.assertTrue(evaluate(condition, f))
        f['flags']['__OBJ_HIDDEN_NPC'] = True
        self.assertFalse(evaluate(condition, f))

    def test_ticket_pushback_retains_its_guard_for_navigation_recovery(self):
        rules = compile_story(story([conditional(call('hasItem', 'SS_TICKET'), [],
                                   [command('movePlayerRelative', 'up')])]))
        pushback = next(r for r in rules if r.effect[0] == 'movement')
        self.assertEqual(pushback.missing(facts()), [])
        guard, wanted = pushback.guards[0]
        self.assertEqual(requirements(guard, not wanted, facts()), [[('item', 'SS_TICKET', True)]])

    def test_hidden_npc_is_a_visibility_prerequisite(self):
        condition = {'Visible': ['BILL_HUMAN', True]}
        self.assertEqual(requirements(condition, True, facts()), [[('visibility', 'BILL_HUMAN', True)]])
        self.assertEqual(requirements(condition, True, facts(__OBJ_SHOWN_BILL_HUMAN=True)), [[]])
        self.assertFalse(evaluate(condition, facts(__OBJ_HIDDEN_BILL_HUMAN=True)))

    def test_trigger_position_is_deferred_until_the_action_lands(self):
        condition = {'BinaryOp': {'op': 'Eq', 'left': call('getPlayerX'), 'right': literal(20)}}
        self.assertEqual(requirements(condition, True, {**facts(), 'x': 19}), [[]])
        guarded = {'BinaryOp': {'op': 'And', 'left': call('getFlag', 'READY'), 'right': condition}}
        self.assertEqual(requirements(guarded, True, {**facts(), 'x': 19}), [[('flag', 'READY', True)]])

    def test_facing_is_an_action_condition_while_story_guards_remain_required(self):
        pose = {'BinaryOp': {'op': 'Eq', 'left': call('getPlayerFacing'), 'right': literal('up')}}
        guarded = {'BinaryOp': {'op': 'And', 'left': call('getFlag', 'READY'), 'right': pose}}
        self.assertEqual(requirements(guarded, True, {**facts(), 'facing': 'Down'}), [[('flag', 'READY', True)]])
        self.assertTrue(evaluate(pose, {'facing': 'Up'}))
        self.assertFalse(evaluate(pose, {'facing': 'Left'}))

    def test_backchain_retains_reachable_alternative_when_first_has_no_producer(self):
        expr={'BinaryOp':{'op':'Or','left':call('getFlag','A'),'right':call('getFlag','B')}}
        rules=compile_story(story([conditional(expr,[command('setFlag','DONE')])]))
        rules+=compile_story({'id':'House:gift','map':'House','triggers':['npc:1'],
                              'program':[command('setFlag','B')]})
        index=StoryIndex.__new__(StoryIndex);index.by_effect={r.effect:[r] for r in rules}
        self.assertEqual([r.effect for r in index.frontier(('flag','DONE',True),facts())],
                         [('flag','B',True)])

    def test_early_return_requires_unset_flag(self):
        rules=compile_story(story([conditional(call('getFlag','PARCEL'),[{'Return':{}}]),
                                  command('giveItem','OAKS_PARCEL',1)]))
        self.assertEqual(rules[0].missing(facts()),[])
        self.assertEqual(rules[0].missing(facts(PARCEL=True)),[('flag','PARCEL',False)])

    def test_and_preserves_both_missing_prerequisites(self):
        expr={'BinaryOp':{'op':'And','left':call('getFlag','RIVAL'),
                          'right':call('hasItem','OAKS_PARCEL')}}
        self.assertEqual(requirements(expr,True,facts()),[[('flag','RIVAL',True),('item','OAKS_PARCEL',True)]])

    def test_or_keeps_alternative_prerequisites(self):
        expr={'BinaryOp':{'op':'Or','left':call('getFlag','A'),'right':call('getFlag','B')}}
        self.assertEqual(requirements(expr,True,facts()),[[('flag','A',True)],[('flag','B',True)]])
        self.assertEqual(requirements(expr,True,facts(B=True)),[[]])

    def test_prior_write_satisfies_later_guard_in_same_script(self):
        rules=compile_story(story([command('setFlag','A'),
                                  conditional(call('getFlag','A'),[command('setFlag','B')])]))
        rule=next(r for r in rules if r.effect[1]=='B')
        self.assertEqual(rule.missing(facts()),[])

    def test_static_wild_battle_result_is_verified_after_execution(self):
        program = [{'Assign': {'name': 'result', 'value': call('startWildBattle', 'SNORLAX', 30)}},
                   conditional({'BinaryOp': {'op': 'Eq', 'left': {'Variable': 'result'},
                                             'right': literal('win')}}, [command('setFlag', 'CLEARED')])]
        rule = next(r for r in compile_story(story(program)) if r.effect[0] == 'flag')
        self.assertEqual(rule.missing(facts()), [])
        self.assertIn(('battle', 'SNORLAX', True), rule.preceding)

    def test_choice_carries_required_confirmation_and_preceding_gift(self):
        rules=compile_story(story([{'Choice':{'options':[
            {'label':literal('YES'),'body':[command('givePokemon','BULBASAUR',5),command('setFlag','STARTER')]},
            {'label':literal('NO'),'body':[]} ]}}]))
        rule=next(r for r in rules if r.effect[1]=='STARTER')
        self.assertEqual(rule.choices,['YES'])
        self.assertIn(('pokemon','BULBASAUR',5),rule.preceding)

    def test_dialogue_forks_do_not_explode_or_constrain_later_effects(self):
        text={'Say':{'texts':[literal('hi')],'name':literal('')}}
        program=[conditional(call('getFlag',str(i)),[text],[text]) for i in range(20)]
        rules=compile_story(story(program+[command('setFlag','DONE')]))
        self.assertEqual(len(rules),1)
        self.assertEqual(rules[0].missing(facts()),[])

    def test_unknown_guard_is_not_silently_true(self):
        rule=compile_story(story([conditional(call('futureUnknown'),[command('setFlag','DONE')])]))[0]
        self.assertEqual(rule.missing(facts())[0][0],'unknown')

    def test_npc_trade_result_is_a_postcondition_not_an_unknown_entry_guard(self):
        program = [conditional(call('getFlag', 'TRADED'), [], [
            {'Choice': {'options': [{'label': literal('YES'), 'body': [
                {'Assign': {'name': 'traded',
                            'value': call('game.tradePokemon', 'ABRA', 'MR_MIME', 'MARCEL')}},
                conditional({'Variable': 'traded'}, [command('setFlag', 'TRADED')])
            ]}]}}
        ])]
        rule = compile_story(story(program))[0]
        self.assertEqual(rule.missing(facts()), [])
        self.assertEqual(rule.choices, ['YES'])
        self.assertEqual(rule.missing(facts(TRADED=True)), [('flag', 'TRADED', False)])
        self.assertIn('Result', json.dumps(rule.guards))

    def test_unknown_assigned_call_remains_an_unknown_entry_guard(self):
        rule = compile_story(story([
            {'Assign': {'name': 'ready', 'value': call('futureUnknown')}},
            conditional({'Variable': 'ready'}, [command('setFlag', 'DONE')])
        ]))[0]
        self.assertEqual(rule.missing(facts())[0][0], 'unknown')

    def test_bag_uses_runtime_names_without_underscores(self):
        f=facts();f['bag']={'OAKSPARCEL':1}
        self.assertTrue(evaluate(call('hasItem','OAKS_PARCEL'),f))

    def test_backchain_produces_prerequisite_instead_of_locked_goal(self):
        rules=compile_story(story([conditional(call('getFlag','KEY'),[command('setFlag','DONE')])]))
        rules+=compile_story({'id':'House:gift','map':'House','triggers':['npc:1'],
                              'program':[command('setFlag','KEY')]})
        index=StoryIndex.__new__(StoryIndex);index.by_effect={r.effect:[r] for r in rules}
        self.assertEqual([r.effect for r in index.frontier(('flag','DONE',True),facts())],[('flag','KEY',True)])


class FakeModel:
    def __init__(self,choice='none',error=False):self.choice=choice;self.error=error
    def system_one(self,*args,**kwargs):
        if self.error:raise TypeSafeError('offline')
        key=next(iter(args[1]))
        return SystemOneResult('jev-test',{key:ChoiceAnswer(self.choice,{self.choice:1},1)},100,0)


class Client:
    def state(self):return {'frame_count':0}


class DecisionTests(unittest.TestCase):
    def test_failed_physical_trigger_shares_retry_limit_across_predicted_effects(self):
        first = Rule('a', 'Town', 'Town:talk', ['npc:1'], [], [], ('flag', 'WON', True), [])
        second = Rule('b', 'Town', 'Town:talk', ['npc:1'], [], [], ('visibility', 'ROCKET', False), [])
        self.assertEqual(attempt_key(first, facts()), attempt_key(second, facts()))
        second.choices = ['NO']
        self.assertNotEqual(attempt_key(first, facts()), attempt_key(second, facts()))
        self.assertNotEqual(attempt_key(first, facts()), attempt_key(first, facts(UNLOCKED=True)))
        self.assertNotEqual(attempt_key(first, facts()),
                            attempt_key(first, {**facts(), 'navigation_revision': 1}))
        # Failing to travel from elsewhere must not suppress the actual
        # interaction once the destination has finally been reached.
        self.assertNotEqual(attempt_key(first, {**facts(), 'map': 'Road'}),
                            attempt_key(first, {**facts(), 'map': 'Town'}))

    def test_blocked_navigation_replans_even_after_real_movement(self):
        agent = self.agent(FakeModel(choice='a'))
        agent.objectives = agent.objectives[:1]
        flag = agent.objectives[0]['satisfied_when']['flag']
        current = {**facts(), 'map': 'Road', 'x': 0, 'y': 0}
        agent.facts = lambda: {**current, 'flags': dict(current['flags'])}
        rule = Rule('barrier', 'Gym', 'Gym:leader', ['npc:1'], [], [], ('flag', flag, True), [])
        index = Mock(rules=[rule], errors=[], sha256='test')
        index.satisfied.return_value = False
        agent.settle = Mock()
        agent.action_candidates = lambda f: ({'a': 'walk'}, {'a': ('travel_to:Gym', rule)})
        strategies = []
        def select(f):
            strategies.append(f['x'])
            agent.active = {'target': ('flag', flag, True), 'objectives': ['progress'], 'rules': [rule]}
        agent.select_strategy = select
        def execute(*args):
            current['x'] += 1
            if current['x'] == 2:
                current['flags'][flag] = True
            return {'result': 'blocked' if current['x'] == 1 else 'reached'}
        agent.execute = execute
        with patch('openpokered.story_agent.StoryIndex', return_value=index):
            self.assertTrue(agent.run()['success'])
        self.assertEqual(strategies, [0, 1])

    def agent(self,model=None,**kwargs):
        return DualStoryAgent(Client(),model or FakeModel(),OBJECTIVES[:2],trace=io.StringIO(),**kwargs)

    def test_completed_stochastic_hunts_do_not_blacklist_an_unchanged_site(self):
        from openpokered.autonomous_story import AutonomousStoryAgent
        from types import MethodType
        for observed, result_kind, expected_selections in [(True, 'hunted', 1),
                (False, 'hunted', 4), (True, 'blocked', 5)]:
            with self.subTest(observed=observed, result_kind=result_kind):
                agent = self.agent(FakeModel(choice='a'))
                agent.completed_stochastic_attempt = MethodType(
                    AutonomousStoryAgent.completed_stochastic_attempt, agent)
                agent.objectives = agent.objectives[:1]
                flag = agent.objectives[0]['satisfied_when']['flag']
                current = {**facts(), 'map': 'Park', 'x': 5, 'y': 8}
                agent.facts = lambda: {**current, 'flags': dict(current['flags'])}
                target = ('catch', 'safari:Park', True)
                rule = Rule('hunt', 'Park', 'skill:catch_encounter', [], [], [], target, [])
                index = Mock(rules=[rule], errors=[], sha256='test')
                index.satisfied.return_value = False
                agent.settle = Mock()
                agent.action_candidates = lambda f: ({'a': 'hunt'}, {'a': ('catch_encounter:safari,Park,5,8', rule)})
                selections, executions = [], []
                def select(f):
                    selections.append(len(executions))
                    agent.active = {'target': target, 'objectives': ['catch'], 'rules': [rule]}
                agent.select_strategy = select
                def execute(*args):
                    executions.append(1)
                    if observed:
                        agent.resolved_battles += 1
                    if len(executions) == 5:
                        current['flags'][flag] = True
                    return {'result': result_kind}
                agent.execute = execute
                with patch('openpokered.story_agent.StoryIndex', return_value=index):
                    self.assertTrue(agent.run()['success'])
                self.assertEqual(len(selections), expected_selections)
                if observed and result_kind == 'hunted':
                    self.assertFalse(any(agent.failures.values()))

    def test_refused_strategy_is_not_completion(self):
        with self.assertRaisesRegex(StoryStopped,'strategy:no_selection'):
            self.agent().choose('strategy',{}, {'a':'obtain key'},'pick a goal')

    def test_split_support_for_valid_options_does_not_become_abstention(self):
        model = Mock()
        model.system_one.return_value = SystemOneResult('jev-test', {
            'action': ChoiceAnswer('none', {'a': .3, 'b': .29, 'c': .05, 'none': .36}, .1)}, 100, 0)
        agent = self.agent(model)
        self.assertEqual(agent.choose('action', {}, {'a': 'train here', 'b': 'train there', 'c': 'another site'}, 'pick'), 'a')

    def test_service_failure_is_separate_from_refusal(self):
        agent=self.agent(FakeModel(error=True))
        agent.record = Mock()
        with self.assertRaisesRegex(StoryStopped,'service_unavailable'):
            agent.choose('action',{}, {'a':'talk'},'pick')
        self.assertEqual(agent.calls['action'],1)
        event = agent.record.call_args
        self.assertEqual(event.args, ('judgment_error',))
        self.assertEqual(event.kwargs['state'], {})
        self.assertEqual(event.kwargs['question']['criteria']['a'], 'talk')
        self.assertIn('frame', event.kwargs)

    def test_both_layers_share_budget_but_keep_separate_accounting(self):
        agent=self.agent(FakeModel(choice='a'),max_calls=2)
        for layer in ('strategy','action'):agent.choose(layer,{}, {'a':'go'},'pick')
        self.assertEqual(dict(agent.calls),{'strategy':1,'action':1})
        self.assertEqual(agent.tokens['strategy'],100)
        with self.assertRaisesRegex(StoryStopped,'judgment_cap'):
            agent.choose('action',{}, {'a':'go'},'pick')

    def test_inventory_and_flag_changes_invalidate_failure_memory(self):
        before=facts();after=facts(PARCEL=True);after['bag']={'OAKSPARCEL':1}
        self.assertNotEqual(progress_key(before),progress_key(after))
        moved={**before,'map':'AnotherMap','x':7}
        self.assertEqual(progress_key(before),progress_key(moved))

    def test_invalid_objectives_are_rejected(self):
        with self.assertRaises(ValueError):DualStoryAgent(Client(),FakeModel(),[])

    def test_agent_verified_objective_needs_no_completion_flag(self):
        objectives = [{'id': 'collect-dex', 'agent_verified': True,
                       'name': 'Register every wild species; clear the first playthrough to open the areas '
                               'that hold the rest'}]
        agent = DualStoryAgent(Client(), FakeModel(), objectives, trace=io.StringIO())
        self.assertEqual([o['id'] for o in agent.objectives], ['collect-dex'])
        # There is no flag for the frontier to backchain, so it contributes
        # no story group; its candidates come from the catch skill instead.
        self.assertIsNone(agent.index)
        self.assertEqual(agent.strategy_groups(facts()), {})
        # The milestone still records the decision the agent itself made.
        agent.objective_satisfied = lambda objective, facts: True
        agent.mark_milestones(facts())
        self.assertEqual(agent.completed, ['collect-dex'])
        self.assertIsNone(json.loads(agent.trace.getvalue())['flag'])

    def test_objective_without_a_flag_needs_agent_verification(self):
        with self.assertRaises(ValueError):DualStoryAgent(Client(),FakeModel(),OBJECTIVES[2:])

    def test_action_budget_stops_before_executing_an_extra_operation(self):
        agent=self.agent(max_actions=1)
        agent.actions=1
        with self.assertRaisesRegex(StoryStopped,'action_budget'):
            agent.execute('travel_to:PewterGym',None)
        self.assertEqual(agent.actions,1)

    def test_old_explorer_no_longer_reports_refusal_as_success(self):
        env=FakeEnv([OBS]);env.client.flags=lambda:{}
        agent=JudgmentAgent(StubJudge(None),explore=True,objectives=OBJECTIVES[:2])
        self.assertEqual(agent.run(env,{'id':'explore'},999),(False,'no_objective_selected'))
        self.assertEqual(env.taken,[])

    def test_old_explorer_rejects_empty_objective_file(self):
        agent=JudgmentAgent(StubJudge(None),explore=True,objectives=[])
        self.assertEqual(agent.run(FakeEnv([OBS]),{'id':'explore'},999),(False,'invalid_objectives'))


class FactsTests(unittest.TestCase):
    def agent(self,state):
        agent=DualStoryAgent.__new__(DualStoryAgent)
        agent.client=Mock()
        agent.client.state.return_value=state
        agent.client.flags.return_value={}
        agent.client.bag.return_value={}
        agent.client.observe.return_value={'badges':{'count':0}}
        return agent

    def test_facts_carry_pokedex_progress_for_a_collecting_objective(self):
        agent=self.agent({'map_name':'PalletTown','player_x':5,'player_y':6,'money':3000,'coins':0,
                          'party':[],
                          'pokedex':{'seen':9,'owned':3,'total':151,
                                     'owned_numbers':[1,4,7],'seen_numbers':[1,4,7,10]}})
        self.assertEqual(agent.facts()['dex']['owned'],3)
        self.assertEqual(agent.facts()['dex']['owned_numbers'],[1,4,7])

    def test_facts_tolerate_a_binary_without_pokedex_progress(self):
        agent=self.agent({'map_name':'PalletTown','player_x':5,'player_y':6,'money':3000,'coins':0,
                          'party':[]})
        self.assertEqual(agent.facts()['dex'],{})


if __name__=='__main__':unittest.main()
