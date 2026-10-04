"""Checkpoint lineage audit tests (no renderer or video mutation)."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import contextlib
import io


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    'dex_dashboard', ROOT / 'docs/jev-retrospective-assets/build_jev_dex_dashboard.py')
dashboard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dashboard)


def dex(frame, species, **extra):
    return {'kind': 'dex_progress', 'frame': frame,
            'owned': len(species), 'owned_species': species, 'acquired': species, **extra}


def segment(events, duration=2, fps=240):
    return {'run': Path('/example'), 'trace': events, 'duration_s': duration, 'fps': fps}


class DexDashboardTest(unittest.TestCase):
    def test_resume_deduplicates_snapshot_and_offsets_reset_frame_clock(self):
        events, boundaries = dashboard.merge_traces([
            segment([dex(240, ['Charmander'])]),
            segment([dex(120, ['Charmander']), dex(240, ['Charmander', 'Pidgey'])], fps=120),
        ])
        self.assertEqual([event['acquired'] for event in events],
                         [['Charmander'], [], ['Pidgey']])
        self.assertEqual([event['source_s'] for event in events], [1, 3, 4])
        self.assertEqual(boundaries[1]['source_s'], 2)

    def test_missing_operation_frame_is_bracketed_not_claimed_exact(self):
        events, _ = dashboard.merge_traces([segment([
            dex(120, ['Charmander']), {'kind': 'operation'}, dex(240, ['Charmander']),
            {'kind': 'operation'},
        ])])
        self.assertEqual(events[1]['source_interval_s'], [0.5, 1])
        self.assertEqual(events[3]['source_interval_s'], [1, 2])
        self.assertEqual(events[1]['timing_basis'], 'preceding_frame_estimate')
        self.assertEqual(events[2]['timing_basis'], 'engine_frame')

    def test_repeat_milestones_are_not_new_achievements(self):
        event = {'kind': 'milestone', 'frame': 240, 'objective': 'beat-brock', 'flag': 'brock'}
        events, _ = dashboard.merge_traces([segment([event]), segment([event])])
        self.assertEqual(len(events), 1)

    def test_rejects_lost_species_and_wrong_counts(self):
        with self.assertRaisesRegex(ValueError, 'species lost'):
            dashboard.merge_traces([segment([dex(1, ['Abra'])]), segment([dex(1, [])])])
        with self.assertRaisesRegex(ValueError, 'count disagrees'):
            dashboard.merge_traces([segment([dex(1, ['Abra'], owned=2)])])

    def test_rejects_nonmonotonic_or_out_of_clip_frames(self):
        for frames in ([240, 120], [1000]):
            with self.subTest(frames=frames), self.assertRaisesRegex(ValueError, 'recording clock'):
                dashboard.merge_traces([segment([dex(frame, []) for frame in frames])])

    def test_final_summary_must_match_trace(self):
        item = segment([dex(1, ['Abra'])])
        item['summary'] = {'final_dex': {'owned_species': ['Mankey']}}
        with self.assertRaisesRegex(ValueError, 'final dex disagrees'):
            dashboard.merge_traces([item])

    def test_unknown_source_does_not_become_gift(self):
        self.assertEqual(dashboard.method(dex(1, ['Abra'])), 'unknown')
        self.assertEqual(dashboard.method(dex(1, ['Abra'], acquisition_method='grass')), 'grass')

    def test_actual_wild_producer_is_not_overwritten_by_planned_terrain(self):
        self.assertEqual(dashboard.method(dex(1, ['Gastly'],
            acquisition_method='wild_capture', planned_acquisition_method='grass')), 'wild_capture')
        self.assertEqual(dashboard.method(dex(1, ['Gastly'],
            planned_acquisition_method='grass')), 'unknown')

    def test_checkpoint_lineage_missing_parent_and_cycle(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            (run / 'trace.jsonl').write_text('')
            summary = run / 'summary.json'
            summary.write_text(json.dumps({'resumed_from': str(run)}))
            with self.assertRaisesRegex(ValueError, 'cyclic'):
                dashboard.load_chain(run, True)
            summary.write_text(json.dumps({'resumed_from': str(run / 'missing')}))
            with self.assertRaises(FileNotFoundError):
                dashboard.load_chain(run, True)

    def test_source_audit_preserves_native_counts_and_requires_remediation(self):
        events, _ = dashboard.merge_traces([segment([
            dex(1, ['Cubone']),
            dex(2, ['Cubone', 'Marowak'], map='PokemonTower6F'),
            dex(3, ['Cubone', 'Marowak']),
        ])])
        audit = dashboard.collection_audit(events, {'collection_audit_schema': 1,
            'collection_audit_pending': {'Marowak': {}}})
        self.assertEqual(audit['pending_species'], ['Marowak'])
        self.assertEqual(events[-1]['owned'], 2)
        self.assertEqual(events[-1]['validated_owned'], 1)
        self.assertEqual(len(audit['history']), 1)
        with self.assertRaisesRegex(ValueError, 'audit disagrees'):
            dashboard.collection_audit(events, {'collection_audit_schema': 1,
                'collection_audit_pending': {}})

    def test_native_evolution_adds_a_validity_waypoint_not_a_new_owned_bit(self):
        events, _ = dashboard.merge_traces([segment([
            dex(1, ['Cubone']), dex(2, ['Cubone', 'Marowak'], map='PokemonTower6F'),
            {'kind': 'collection_audit_resolved', 'frame': 3, 'species': 'Marowak',
             'acquisition_method': 'evolution',
             'before': {'species': 'CUBONE', 'level': 27},
             'after': {'species': 'MAROWAK', 'level': 28}},
        ])])
        audit = dashboard.collection_audit(events, {'collection_audit_schema': 1,
            'collection_audit_pending': {}})
        self.assertEqual(audit['pending_species'], [])
        self.assertEqual([entry['status'] for entry in audit['history']],
                         ['invalid_source', 'resolved'])
        self.assertEqual(events[-1]['validated_owned'], 2)
        events[-1]['after']['level'] = 27
        with self.assertRaisesRegex(ValueError, 'evolution evidence'):
            dashboard.collection_audit(events, {})

    def test_initial_snapshot_and_ordinary_marowak_are_not_invalid_sources(self):
        for trace in ([dex(1, ['Marowak'], map='PokemonTower6F')],
                      [dex(1, ['Cubone']), dex(2, ['Cubone', 'Marowak'], map='Route1')]):
            events, _ = dashboard.merge_traces([segment(trace)])
            self.assertEqual(dashboard.collection_audit(events, {})['pending_species'], [])

    def build_manifest(self, summaries):
        """Fixture containers only: exercise metadata, not video decoding."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            segments = []
            for index, metadata in enumerate(summaries):
                run = root / f'run-{index}'
                run.mkdir()
                clip = run / 'clip.mp4'
                clip.write_bytes(b'fixture container')
                summary = {**metadata, 'final_dex': {'owned_species': ['Abra']},
                           'recording': {**metadata.get('recording', {}),
                                         'path': str(clip), 'game_frames_per_video_second': 240}}
                segments.append({**segment([dex(240, ['Abra'])]),
                                 'run': run, 'summary': summary})
            assembled = root / 'assembled.mp4'
            assembled.write_bytes(b'fixture assembled container')
            duration = lambda path: len(segments) if Path(path).resolve() == assembled.resolve() else 1
            output = root / 'out'
            with patch.object(dashboard, 'load_chain', return_value=segments), \
                    patch.object(dashboard, 'video_duration', side_effect=duration), \
                    patch.object(dashboard.subprocess, 'check_output', return_value='builder-head\n'), \
                    contextlib.redirect_stdout(io.StringIO()):
                dashboard.build(segments[-1]['run'], output, video=assembled, chain=True)
            return json.loads((output / 'manifest.json').read_text())

    def test_export_preserves_each_segments_recorded_runtime_fingerprints(self):
        summaries = [
            {'policy_sha256': 'a' * 64, 'binary_sha256': 'b' * 64,
             'policy_files': {'scripts/controller.py': 'c' * 64},
             'recording': {'assets': {'sha256': 'd' * 64, 'png_count': 668}}},
            {'source_commit': 'recorded-second-commit', 'policy_sha256': 'e' * 64,
             'binary_sha256': 'f' * 64,
             'policy_files': {'scripts/controller.py': '0' * 64},
             'recording': {'assets': {'sha256': '1' * 64, 'png_count': 669}}},
        ]
        manifest = self.build_manifest(summaries)
        self.assertEqual(manifest['builder_commit'], 'builder-head')
        for exported, recorded in zip(manifest['segments'], summaries):
            for field in ('policy_sha256', 'policy_files', 'binary_sha256'):
                self.assertEqual(exported.get(field), recorded[field])
            self.assertEqual(exported.get('recording_assets'), recorded['recording']['assets'])
            self.assertEqual(exported['source_commit'], recorded.get('source_commit'))

    def test_legacy_segment_identity_stays_unknown_instead_of_using_builder(self):
        exported = self.build_manifest([{}])['segments'][0]
        for field in ('source_commit', 'policy_sha256', 'policy_files',
                      'binary_sha256', 'recording_assets'):
            self.assertIn(field, exported)
            self.assertIsNone(exported[field])


if __name__ == '__main__':
    unittest.main()
