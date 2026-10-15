"""Failure diagnostics must survive partial/old TCP responses without SAVE."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock

from playthrough import Game


class FailureArtifactsTests(unittest.TestCase):
    def capture(self, responses):
        game = Game.__new__(Game)
        game.d = Mock()
        game.d.cmd.side_effect = lambda **command: responses[command['cmd']]
        return game

    def test_unsupported_export_is_reported_and_other_observations_survive(self):
        responses = {
            'get_state': {'ok': True, 'data': {'frame_count': 383212, 'screen': 'overworld'}},
            'get_npcs': {'ok': False, 'error': 'NPC inspection unavailable'},
            'get_flags': {'ok': True, 'data': {'EVENT_GOT_STARTER': True}},
            'get_agent_state': {'ok': True, 'data': {'party': [{'hp': 43}]}},
            'export_fixture': {'ok': False, 'error': 'unknown variant export_fixture'},
        }
        game = self.capture(responses)
        with tempfile.TemporaryDirectory() as directory:
            # Previous captures must never impersonate the failed export.
            (Path(directory) / 'failure-fixture.json').write_text('{"stale": true}')
            game.capture_failure_artifacts(directory, 'original healing assertion\n')
            directory = Path(directory)
            self.assertEqual((directory / 'failure.txt').read_text(), 'original healing assertion\n')
            self.assertEqual(json.loads((directory / 'failure-state.json').read_text()), responses['get_state']['data'])
            self.assertTrue((directory / 'failure-flags.json').exists())
            self.assertTrue((directory / 'failure-agent.json').exists())
            self.assertFalse((directory / 'failure-fixture.json').exists())
            captures = json.loads((directory / 'failure-capture.json').read_text())['captures']
            errors = {v['command']['cmd']: v for v in captures if not v['ok']}
            self.assertEqual(set(errors), {'get_npcs', 'export_fixture'})
            self.assertEqual(errors['export_fixture']['response'], responses['export_fixture'])
            self.assertIn('unknown variant', errors['export_fixture']['error'])
        self.assertEqual([v.kwargs['cmd'] for v in game.d.cmd.call_args_list], list(responses))
        self.assertNotIn('save', [v.kwargs['cmd'] for v in game.d.cmd.call_args_list])

    def test_fixture_is_bootable_save_data_without_response_envelope(self):
        fixture = {'player_name': [145], 'game_data': {'position': {'map_id': 89, 'x': 3, 'y': 3}},
                   'party': [{'hp': 43, 'pp': [5, 40, 10, 10]}]}
        responses = {name: {'ok': True, 'data': fixture if name == 'export_fixture' else {}}
                     for name in ['get_state', 'get_npcs', 'get_flags', 'get_agent_state', 'export_fixture']}
        game = self.capture(responses)
        with tempfile.TemporaryDirectory() as directory:
            captures = game.capture_failure_artifacts(directory, 'original failure')
            self.assertTrue(all(v['ok'] for v in captures))
            self.assertEqual(json.loads((Path(directory) / 'failure-fixture.json').read_text()), fixture)

    def test_disconnected_transport_keeps_original_error_and_reports_every_capture(self):
        game = self.capture({})
        game.d.cmd.side_effect = ConnectionError('connection closed')
        with tempfile.TemporaryDirectory() as directory:
            captures = game.capture_failure_artifacts(directory, 'original failure')
            self.assertEqual((Path(directory) / 'failure.txt').read_text(), 'original failure')
            self.assertEqual(len(captures), 6)
            self.assertTrue(all(not v['ok'] and 'connection closed' in v['error'] for v in captures[1:]))
            self.assertEqual(json.loads((Path(directory) / 'failure-capture.json').read_text())['captures'], captures)


if __name__ == '__main__':
    unittest.main()
