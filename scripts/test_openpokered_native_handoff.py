"""Failed collection controllers must not destroy unsaved native runtime."""
import io
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from openpokered import run_autonomous


class NativeHandoffTests(unittest.TestCase):
    def run_fixture(self, root, *, state, live=True, goal='collect-dex', observe_error=False,
                    checkpoint=True, save_reply=None):
        root = Path(root)
        binary = root / 'pokered-app'
        binary.write_bytes(b'frozen native binary')
        save = root / 'play.sav'
        save.write_bytes(b'old SRAM, not the live battle')
        (root / 'game.log').write_text('native log\n')
        game = Mock(run_dir=root, save_path=save, battles_driven=0, move_cache_hits=0)
        game.proc.pid = 12345
        game.proc.poll.return_value = None if live else 0
        game.d.raw.port, game.d.raw.host, game.d.counts = 45678, '127.0.0.1', {}
        data = {'get_state': state, 'get_flags': {},
                'get_party': [], 'get_bag': [], 'get_npcs': []}
        def command(**request):
            if observe_error:
                raise OSError('final observation failed')
            if request['cmd'] == 'save' and save_reply is not None:
                return save_reply
            return {'ok': True, 'data': data.get(request['cmd'])}
        game.d.raw.cmd.side_effect = command
        with patch.object(run_autonomous, 'TypeSafeClient'), \
                patch.object(run_autonomous, 'JevGame', return_value=game), \
                patch.object(run_autonomous, 'boot_new_game', side_effect=RuntimeError('actor stopped')), \
                patch('sys.stdout', new_callable=io.StringIO):
            argv = ['--goal', goal, '--binary', str(binary), '--output', str(root / 'out')]
            self.assertEqual(run_autonomous.main(argv + (['--checkpoint'] if checkpoint else [])), 1)
        folder = next((root / 'out').iterdir())
        summary = json.loads((folder / 'summary.json').read_text())
        return game, folder, summary

    def test_live_battle_keeps_native_and_durable_runtime_but_not_a_checkpoint(self):
        with tempfile.TemporaryDirectory() as root:
            game, folder, summary = self.run_fixture(root, state={'screen': 'battle', 'battle_phase': 'PlayerMenu'})
            game.close.assert_not_called()
            game.d.close.assert_called_once()
            self.assertTrue(summary['native_runtime_preserved'])
            self.assertFalse(summary['success'])
            self.assertFalse(summary['development_checkpoint'])
            self.assertFalse((folder / 'game.sav').exists())
            self.assertEqual((folder / 'nonresumable-native-save.sav').read_bytes(), b'old SRAM, not the live battle')
            handoff = json.loads((folder / 'native-handoff.json').read_text())
            self.assertEqual(handoff['native_pid'], 12345)
            self.assertEqual(handoff['debug_port'], 45678)
            self.assertEqual(handoff['status'], 'native_live_requires_exclusive_reattachment')
            self.assertFalse(handoff['is_sram_checkpoint'])
            self.assertTrue(Path(handoff['runtime_root']).is_dir())
            self.assertEqual(Path(handoff['binary']).read_bytes(), b'frozen native binary')
            self.assertEqual(handoff['policy_sha256'], summary['policy_sha256'])
            self.assertNotIn('save', [call.kwargs['cmd'] for call in game.d.raw.cmd.call_args_list])
            game.proc.terminate.assert_not_called()

    def test_dead_native_is_not_reported_as_retained(self):
        with tempfile.TemporaryDirectory() as root:
            game, folder, summary = self.run_fixture(root, state={'screen': 'battle'}, live=False)
            game.close.assert_called_once()
            self.assertFalse(summary.get('native_runtime_preserved', False))
            self.assertFalse((folder / 'native-handoff.json').exists())
            self.assertEqual(list(folder.glob('.runtime-*')), [])

    def test_finite_tm_reference_native_inputs_are_hash_bound_in_manifest(self):
        with tempfile.TemporaryDirectory() as root:
            _, _, summary = self.run_fixture(root, state={'screen': 'battle'}, live=False)
        for name in ('crates/pokered-data/src/item_data.rs', 'crates/pokered-data/src/items.rs',
                     'crates/pokered-core/src/items/shop.rs'):
            expected = hashlib.sha256((run_autonomous.pt.ROOT / name).read_bytes()).hexdigest()
            self.assertEqual(summary['policy_files'][name], expected)

    def test_safe_overworld_still_saves_and_closes_normally(self):
        state = {'screen': 'overworld', 'warp_fade': 'Idle', 'player_movement_state': 'Idle',
                 'script_running': False, 'script_awaiting_battle': False,
                 'door_exit_pending': False, 'active_script_effect': None,
                 'dialogue': None, 'choice': None, 'field_menu': None, 'fishing_active': False}
        with tempfile.TemporaryDirectory() as root:
            game, folder, summary = self.run_fixture(root, state=state)
            game.close.assert_called_once()
            self.assertTrue(summary['development_checkpoint'])
            self.assertFalse(summary.get('native_runtime_preserved', False))
            self.assertEqual(list(folder.glob('.runtime-*')), [])

    def test_invalid_observations_cannot_justify_destroying_a_live_native(self):
        with tempfile.TemporaryDirectory() as root:
            game, folder, summary = self.run_fixture(root, state=None)
            game.close.assert_not_called()
            self.assertFalse(summary['final_observations_valid'])
            self.assertTrue(summary['native_runtime_preserved'])
            self.assertTrue((folder / 'native-handoff.json').exists())

    def test_observation_exception_also_preserves_native_without_claiming_state(self):
        with tempfile.TemporaryDirectory() as root:
            game, folder, summary = self.run_fixture(root, state=None, observe_error=True)
            game.close.assert_not_called()
            self.assertIn('final_observation_or_export_failed', summary['reason'])
            handoff = json.loads((folder / 'native-handoff.json').read_text())
            self.assertIsNone(handoff['final_observations_sha256'])
            self.assertTrue(Path(handoff['runtime_root']).is_dir())

    def test_legacy_story_shutdown_policy_is_not_changed(self):
        with tempfile.TemporaryDirectory() as root:
            game, folder, _ = self.run_fixture(root, state={'screen': 'battle'}, goal='story')
            game.close.assert_called_once()
            self.assertFalse((folder / 'native-handoff.json').exists())

    def test_safe_control_without_a_confirmed_save_also_retains_live_progress(self):
        state = {'screen': 'overworld', 'warp_fade': 'Idle', 'player_movement_state': 'Idle',
                 'script_running': False, 'script_awaiting_battle': False,
                 'door_exit_pending': False, 'active_script_effect': None,
                 'dialogue': None, 'choice': None, 'field_menu': None, 'fishing_active': False}
        for config in ({'checkpoint': False}, {'save_reply': {'ok': False, 'error': 'save failed'}}):
            with self.subTest(config=config), tempfile.TemporaryDirectory() as root:
                game, folder, summary = self.run_fixture(root, state=state, **config)
                self.assertTrue(summary['native_checkpoint_safe'])
                self.assertFalse(summary['development_checkpoint'])
                self.assertTrue(summary['native_runtime_preserved'])
                self.assertFalse((folder / 'game.sav').exists())
                game.close.assert_not_called()


if __name__ == '__main__':
    unittest.main()
