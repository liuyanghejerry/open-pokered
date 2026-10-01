"""Checkpoint lineage audit tests (no renderer or video mutation)."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


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


if __name__ == '__main__':
    unittest.main()
