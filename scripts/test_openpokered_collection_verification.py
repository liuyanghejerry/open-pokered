import copy
import tempfile
import unittest
import sys
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from scripts.openpokered.collection_verification import (
    collection_snapshot, require_collection_completion, verify_collection_continue)


def observations(count=124):
    names = [f'Species{i}' for i in range(count)]
    counts = [20, 19, 2] + [0] * 9
    stored = [{'box': box, 'index': index, 'species': f'Stored{box}_{index}',
               'level': 10, 'hp': 30, 'max_hp': 30, 'status': 'None',
               'moves': ['Tackle', 'None', 'None', 'None'], 'pp': [35, 0, 0, 0]}
              for box, size in enumerate(counts) for index in range(size)]
    return {cmd: {'ok': True, 'data': value} for cmd, value in {
        'get_state': {'screen': 'overworld', 'pokedex': {'owned': count, 'seen': count,
            'owned_species': names, 'seen_species': names}, 'map_name': 'PalletTown',
            'player_x': 5, 'player_y': 6, 'money': 15441, 'coins': 0, 'badges': 15,
            'current_box_index': 1, 'box_counts': counts, 'stored_pokemon': stored},
        'get_party': [{'species': 'Charizard', 'hp': 181, 'pp': [18, 30, 15, 10]}],
        'get_bag': [{'item': 'PokeBall', 'qty': 12}],
        'get_flags': {'EVENT_GOT_POKEDEX': True},
    }.items()}


class CollectionContinueTests(unittest.TestCase):
    def test_final_gate_rejects_partial_duplicate_and_unvalidated_collection(self):
        require_collection_completion(observations(), {})
        with self.assertRaisesRegex(ValueError, '124-species'):
            require_collection_completion(observations(50), {})
        with self.assertRaisesRegex(ValueError, 'source-validated'):
            require_collection_completion(observations(), {'Marowak': {}})
        invalid = observations()
        invalid['get_state']['data']['pokedex']['owned_species'][-1] = 'Species0'
        with self.assertRaisesRegex(ValueError, 'Invalid'):
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
                self.assertEqual(proof['schema'], 2)
                self.assertEqual(proof['expected'], proof['restored'])
                self.assertNotEqual(create.call_args.kwargs['save_path'], saved)
                self.assertNotEqual(create.call_args.kwargs['binary'], binary)
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


if __name__ == '__main__':
    unittest.main()
