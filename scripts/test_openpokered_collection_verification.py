import copy
import json
import tempfile
import unittest
import sys
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from scripts.openpokered.collection_verification import (
    collection_snapshot, require_collection_completion, verify_collection_continue)
from scripts.openpokered.collection_planner import (
    SUPER_ROD_MAP_GROUP, complete_acquisition_graph, solo_plan)
from scripts.openpokered.story_rules import MAPS_DIR


def solo_species():
    maps = {path.parent.name: json.loads(path.read_text())
            for path in MAPS_DIR.glob('*/map.json')}
    graph = complete_acquisition_graph(maps, SUPER_ROD_MAP_GROUP)
    return solo_plan(graph, forced_choices={'starter': 'Charmander', 'fossil': 'Kabuto',
        'dojo': 'Hitmonlee', 'eevee_evolution': 'Jolteon'})['reachable_species']


def observations(count=124):
    names = solo_species()[:count]
    counts = [20, 19, 2] + [0] * 9
    stored = [{'box': box, 'index': index, 'species': f'Stored{box}_{index}',
               'level': 10, 'hp': 30, 'max_hp': 30, 'status': 'None',
               'moves': ['Tackle', 'None', 'None', 'None'], 'pp': [35, 0, 0, 0]}
              for box, size in enumerate(counts) for index in range(size)]
    party = [{'species': 'Charizard', 'level': 60, 'hp': 181, 'max_hp': 181,
              'status': 'None', 'moves': ['Slash', 'Cut', 'Flamethrower', 'Dig'],
              'pp': [18, 30, 15, 10]}]
    return {cmd: {'ok': True, 'data': value} for cmd, value in {
        'get_state': {'screen': 'overworld', 'pokedex': {'owned': count, 'seen': count,
            'owned_species': names, 'seen_species': names}, 'map_name': 'PalletTown',
            'player_x': 5, 'player_y': 6, 'money': 15441, 'coins': 0, 'badges': 15,
            'current_box_index': 1, 'box_counts': counts, 'stored_pokemon': stored,
            'party': party,
            'safari_game': {'active': False, 'balls_remaining': 0, 'steps_remaining': 0}},
        # The actual native get_party reports XP/current_hp but has no PP.
        'get_party': [{**{key: value for key, value in mon.items() if key not in ('hp', 'pp')},
                       'current_hp': mon['hp'], 'experience': 216000} for mon in party],
        'get_bag': [{'item': 'PokeBall', 'qty': 12}],
        'get_flags': {'EVENT_GOT_POKEDEX': True},
    }.items()}


class CollectionContinueTests(unittest.TestCase):
    def test_party_pp_comes_from_normal_roster_not_get_party_or_evaluation(self):
        expected = observations()
        state = expected['get_state']['data']
        state['evaluation'] = {'party': [{'pp': [0, 0, 0, 0]}]}
        self.assertNotIn('pp', expected['get_party']['data'][0])
        snapshot = collection_snapshot(expected)
        self.assertEqual(snapshot['party_pp'], [[18, 30, 15, 10]])
        self.assertEqual(snapshot['party'], expected['get_party']['data'])
        self.assertEqual(snapshot['party'][0]['experience'], 216000)
        self.assertNotIn('evaluation', snapshot)

    def test_party_pp_observation_is_required_and_four_native_bytes(self):
        for value in (None, [], [18, 30, 15], [18, 30, 15, 10, 1],
                      [True, 30, 15, 10], [18.0, 30, 15, 10], ['18', 30, 15, 10],
                      [-1, 30, 15, 10], [256, 30, 15, 10]):
            invalid = observations()
            invalid['get_state']['data']['party'][0]['pp'] = value
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'party PP observation'):
                collection_snapshot(invalid)
        invalid = observations()
        state = invalid['get_state']['data']
        state['evaluation'] = {'party': copy.deepcopy(state['party'])}
        state['party'][0].pop('pp')
        with self.assertRaisesRegex(ValueError, 'party PP observation'):
            collection_snapshot(invalid)

    def test_party_observations_must_agree_in_order_and_shared_fields(self):
        for field, value in [('species', 'Muk'), ('level', 59), ('hp', 180),
                             ('max_hp', 182), ('status', 'Paralysis'),
                             ('moves', ['Growl', 'Cut', 'Flamethrower', 'Dig'])]:
            invalid = observations()
            invalid['get_state']['data']['party'][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'party observation'):
                collection_snapshot(invalid)
        for invalid_roster in (None, {}, [], [None] * 7):
            invalid = observations()
            invalid['get_state']['data']['party'] = invalid_roster
            with self.subTest(roster=invalid_roster), self.assertRaisesRegex(ValueError, 'party observation'):
                collection_snapshot(invalid)
        for command in ('get_state', 'get_party'):
            invalid = observations()
            if command == 'get_state':
                invalid[command]['data']['party'][0].pop('species')
            else:
                invalid[command]['data'][0].pop('species')
            with self.subTest(command=command), self.assertRaisesRegex(ValueError, 'party observation'):
                collection_snapshot(invalid)

    def test_party_pp_keeps_slot_order_for_matching_rosters(self):
        expected = observations()
        # Matching species/moves do not establish an individual identity:
        # keep the native slot order rather than sorting/deduplicating by species.
        expected['get_state']['data']['party'].append(copy.deepcopy(expected['get_state']['data']['party'][0]))
        expected['get_state']['data']['party'][1]['pp'] = [17, 30, 15, 10]
        expected['get_party']['data'].append(copy.deepcopy(expected['get_party']['data'][0]))
        reversed_pp = copy.deepcopy(expected)
        reversed_pp['get_state']['data']['party'].reverse()
        before, after = collection_snapshot(expected), collection_snapshot(reversed_pp)
        self.assertEqual(before['party'], after['party'])
        self.assertNotEqual(before['party_pp'], after['party_pp'])
        self.assertEqual(before['party_pp'], [[18, 30, 15, 10], [17, 30, 15, 10]])

    def test_independent_continue_detects_pp_only_loss_and_invalid_restored_roster(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            saved, binary = folder / 'original.sav', folder / 'app'
            saved.write_bytes(b'a' * 32768)
            binary.write_bytes(b'test binary')
            expected, game = observations(), Mock()
            game.proc.pid = 123
            raw = game.d
            with patch('scripts.openpokered.collection_verification.pt.Game', return_value=game), \
                    patch('scripts.openpokered.collection_verification.pt.resume_reentry'):
                for mutation in ('pp_loss', 'missing_pp', 'mixed_roster'):
                    restored = copy.deepcopy(expected)
                    mon = restored['get_state']['data']['party'][0]
                    if mutation == 'pp_loss':
                        mon['pp'][0] -= 1
                        message = 'CONTINUE changed.*party_pp'
                    elif mutation == 'missing_pp':
                        mon.pop('pp')
                        message = 'CONTINUE changed.*party PP observation'
                    else:
                        mon['species'] = 'Muk'
                        message = 'CONTINUE changed.*party observation'
                    self.assertEqual(restored['get_party'], expected['get_party'])
                    raw.cmd.side_effect = lambda **kw: restored[kw['cmd']]
                    with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, message):
                        verify_collection_continue(saved, binary, expected)
                    self.assertEqual(saved.read_bytes(), b'a' * 32768)

    def test_final_gate_rejects_partial_duplicate_and_unvalidated_collection(self):
        require_collection_completion(observations(), {})
        with self.assertRaisesRegex(ValueError, '124-species'):
            require_collection_completion(observations(50), {})
        with self.assertRaisesRegex(ValueError, 'source-validated'):
            require_collection_completion(observations(), {'Marowak': {}})
        invalid = observations()
        invalid['get_state']['data']['pokedex']['owned_species'][-1] = \
            invalid['get_state']['data']['pokedex']['owned_species'][0]
        with self.assertRaisesRegex(ValueError, 'Invalid'):
            require_collection_completion(invalid, {})

    def test_final_gate_requires_exact_red_solo_species_not_just_count(self):
        valid = observations()
        self.assertEqual(valid['get_state']['data']['pokedex']['owned'], 124)
        require_collection_completion(valid, {})
        # Keeping count, uniqueness and owned-subset-of-seen intact cannot
        # make an unavailable, external, mutually-exclusive or unknown source
        # a legitimate substitute for one of the actual solo targets.
        for substitute in ('Mew', 'Alakazam', 'Sandshrew', 'Squirtle', 'TypoSpecies'):
            invalid = copy.deepcopy(valid)
            dex = invalid['get_state']['data']['pokedex']
            dex['owned_species'][-1] = substitute
            dex['seen_species'][-1] = substitute
            with self.subTest(substitute=substitute), self.assertRaisesRegex(ValueError, 'Red solo'):
                require_collection_completion(invalid, {})

    def test_battle_state_cannot_be_final_save_proof(self):
        invalid = observations()
        invalid['get_state']['data']['screen'] = 'battle'
        with self.assertRaisesRegex(ValueError, 'overworld'):
            collection_snapshot(invalid)

    def test_storage_proof_requires_complete_unique_slots_and_all_observed_fields(self):
        expected = observations()
        reordered = copy.deepcopy(expected)
        reordered['get_state']['data']['stored_pokemon'].reverse()
        self.assertEqual(collection_snapshot(expected), collection_snapshot(reordered))
        mutations = [lambda state: state.pop('stored_pokemon'),
                     lambda state: state['stored_pokemon'].pop(),
                     lambda state: state['stored_pokemon'].append(state['stored_pokemon'][0]),
                     lambda state: state['stored_pokemon'][0].pop('pp'),
                     lambda state: state['stored_pokemon'][0].update(index=1)]
        for mutate in mutations:
            invalid = copy.deepcopy(expected)
            mutate(invalid['get_state']['data'])
            with self.assertRaisesRegex(ValueError, 'stored Pokemon'):
                collection_snapshot(invalid)

    def test_safari_session_observation_is_required_and_bounded(self):
        for safari in (None, {}, {'active': False, 'balls_remaining': 0},
                       {'active': 1, 'balls_remaining': 30, 'steps_remaining': 500},
                       {'active': True, 'balls_remaining': 31, 'steps_remaining': 500},
                       {'active': True, 'balls_remaining': 30, 'steps_remaining': -1}):
            invalid = observations()
            invalid['get_state']['data']['safari_game'] = safari
            with self.subTest(safari=safari), self.assertRaisesRegex(ValueError, 'Safari'):
                collection_snapshot(invalid)

    def test_independent_continue_preserves_source_and_detects_persisted_loss(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            saved, binary, flags = folder / 'original.sav', folder / 'app', folder / 'flags.json'
            saved.write_bytes(b'a' * 32768)
            binary.write_bytes(b'test binary')
            flags.write_text('{}')
            expected = observations()
            game = Mock()
            game.proc.pid = 123
            raw = game.d
            raw.cmd.side_effect = lambda **kw: expected[kw['cmd']]
            with patch('scripts.openpokered.collection_verification.pt.Game', return_value=game) as create, \
                    patch('scripts.openpokered.collection_verification.pt.resume_reentry') as resume:
                proof = verify_collection_continue(saved, binary, expected, flags)
                self.assertTrue(proof['verified'])
                self.assertEqual(proof['schema'], 4)
                self.assertEqual(proof['expected'], proof['restored'])
                self.assertNotEqual(create.call_args.kwargs['save_path'], saved)
                self.assertNotEqual(create.call_args.kwargs['binary'], binary)
                isolated = create.call_args.kwargs['binary'].parent
                self.assertEqual(isolated.parent, saved.parent.resolve())
                self.assertEqual(create.call_args.kwargs['runtime_root'], isolated)
                resume.assert_called_once_with(game)
                game.close.assert_called_once()
                self.assertEqual(saved.read_bytes(), b'a' * 32768)
                for section in ('get_state', 'get_party', 'get_bag', 'get_flags'):
                    restored = copy.deepcopy(expected)
                    if section == 'get_state':
                        restored[section]['data']['current_box_index'] = 0
                    elif section == 'get_flags':
                        restored[section]['data']['EVENT_GOT_POKEDEX'] = False
                    else:
                        restored[section]['data'] = []
                    raw.cmd.side_effect = lambda **kw: restored[kw['cmd']]
                    with self.assertRaisesRegex(ValueError, 'CONTINUE changed'):
                        verify_collection_continue(saved, binary, expected, flags)
                # Unchanged dex/counts cannot mask a lost or altered boxed
                # Pokemon. Exercise every persistent field exposed by the
                # native observation, plus a valid swap of two occupied slots.
                for field, value in [('species', 'Different'), ('level', 11),
                        ('hp', 20), ('max_hp', 31), ('status', 'Paralysis'),
                        ('moves', ['Growl', 'None', 'None', 'None']), ('pp', [34, 0, 0, 0]),
                        ('slot_swap', None), ('box_swap', None)]:
                    with self.subTest(field=field):
                        restored = copy.deepcopy(expected)
                        stored = restored['get_state']['data']['stored_pokemon']
                        if field == 'slot_swap':
                            stored[0]['index'], stored[1]['index'] = 1, 0
                        elif field == 'box_swap':
                            stored[0]['box'], stored[20]['box'] = 1, 0
                        else:
                            stored[0][field] = value
                        raw.cmd.side_effect = lambda **kw: restored[kw['cmd']]
                        with self.assertRaisesRegex(ValueError, 'CONTINUE changed.*stored_pokemon'):
                            verify_collection_continue(saved, binary, expected, flags)
                expected['get_state']['data']['safari_game'] = {
                    'active': True, 'balls_remaining': 23, 'steps_remaining': 85}
                for field, value in [('active', False), ('balls_remaining', 0), ('steps_remaining', 0)]:
                    with self.subTest(safari_field=field):
                        restored = copy.deepcopy(expected)
                        restored['get_state']['data']['safari_game'][field] = value
                        raw.cmd.side_effect = lambda **kw: restored[kw['cmd']]
                        with self.assertRaisesRegex(ValueError, 'CONTINUE changed.*state'):
                            verify_collection_continue(saved, binary, expected, flags)


if __name__ == '__main__':
    unittest.main()
