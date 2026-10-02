import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.stage_jev_dashboard import ASSETS, DEX_REQUIRED, LFS_PREFIX, REQUIRED, stage


class PagesStagingTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / 'repo'
        self.source = self.repo / ASSETS
        self.site = Path(self.temp.name) / 'site'
        self.site.mkdir()
        (self.site / 'index.html').write_text('existing game')
        (self.site / 'editor').mkdir()
        (self.site / 'editor/index.html').write_text('existing editor')
        for name in REQUIRED:
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('<main><a href="../../jev-autonomous-retrospective.md">notes</a></main>')
        self.recording = self.source / 'full-run/jev-full.mp4'
        manifest = {'recordings': [{'file': self.recording.name,
                    'sha256': hashlib.sha256(self.recording.read_bytes()).hexdigest()}]}
        (self.source / 'full-run/manifest.json').write_text(json.dumps(manifest))

    def test_preserves_existing_site_and_resolves_links(self):
        (self.source / 'full-run/jev-00-x32.mp4').write_bytes(b'render intermediate')
        stage(self.repo, self.site, 'abc123')
        self.assertEqual((self.site / 'index.html').read_text(), 'existing game')
        self.assertEqual((self.site / 'editor/index.html').read_text(), 'existing editor')
        target = self.site / 'jev-dashboard'
        self.assertFalse((target / 'full-run/jev-00-x32.mp4').exists())
        self.assertEqual((target / 'full-run/jev-full.mp4').read_bytes(), self.recording.read_bytes())
        page = (target / 'full-run/jev-player.html').read_text()
        self.assertIn('/blob/abc123/docs/jev-autonomous-retrospective.md', page)
        self.assertNotIn('../../jev-autonomous-retrospective.md', page)
        self.assertNotIn('<nav', page)  # Navigation comes from the shared locale script.
        self.assertEqual((target / 'jev-dashboard-i18n.js').read_bytes(),
                         (self.source / 'jev-dashboard-i18n.js').read_bytes())
        redirect = (target / 'index.html').read_text()
        self.assertIn('location.search+location.hash', redirect)

    def test_rejects_unresolved_lfs_before_replacing_dashboard(self):
        target = self.site / 'jev-dashboard'
        target.mkdir()
        (target / 'index.html').write_text('previous dashboard')
        self.recording.write_bytes(LFS_PREFIX)
        with self.assertRaisesRegex(ValueError, 'Git LFS pointer'):
            stage(self.repo, self.site, 'abc123')
        self.assertEqual((target / 'index.html').read_text(), 'previous dashboard')

    def test_rejects_corrupt_recording(self):
        self.recording.write_bytes(b'changed recording')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            stage(self.repo, self.site, 'abc123')

    def prepare_dex(self):
        for name in DEX_REQUIRED:
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('dex fixture')
        video = self.source / 'dex-run/jev-dex-full.mp4'
        digest = hashlib.sha256(video.read_bytes()).hexdigest()
        (self.source / 'dex-run/manifest.json').write_text(json.dumps({
            'files': {'jev-dex-full.mp4': {'bytes': video.stat().st_size, 'sha256': digest}}}))
        data = {'schema': 3, 'run': {'success': True, 'video_sha256': digest},
            'target': {'solo_ceiling': 124, 'owned': 124, 'validated_owned': 124,
                       'pending_source_validation': []},
            'collection_audit': {'pending_species': []},
            'species': [{'number': number, 'name': f'Mon{number}',
                         'status': 'owned' if number <= 124 else 'unreachable'}
                        for number in range(1, 152)],
            'progress': [{'owned': 124, 'validated_owned': 124,
                          'pending_source_validation': [],
                          'owned_species': [f'Mon{number}' for number in range(1, 125)]}]}
        snapshot = {'dex': {'owned': 124, 'owned_species': data['progress'][0]['owned_species']},
                    'state': {'box_counts': [0] * 12, 'current_box_index': 0},
                    'party': [{'species': 'Mon1'}], 'stored_pokemon': [], 'bag': [], 'flags': {}}
        data['run']['collection_continue_verification'] = {
            'schema': 2, 'verified': True, 'save_sha256': 'a' * 64, 'expected': snapshot, 'restored': snapshot}
        self.write_dex_data(data)
        return video

    def write_dex_data(self, data):
        text = json.dumps(data)
        (self.source / 'dex-run/jev-dex-dashboard.json').write_text(text)
        (self.source / 'dex-run/jev-dex-dashboard-data.js').write_text(
            'window.JEV_DEX_DASHBOARD=' + text + ';\n')

    def test_stages_complete_dex_alongside_existing_players(self):
        video = self.prepare_dex()
        stage(self.repo, self.site, 'abc123')
        target = self.site / 'jev-dashboard/dex-run'
        self.assertEqual((target / video.name).read_bytes(), video.read_bytes())
        self.assertTrue((target / 'jev-dex-player.html').is_file())
        self.assertTrue((self.site / 'jev-dashboard/full-run/jev-player.html').is_file())

    def test_completion_requires_independent_continue_proof(self):
        import copy
        self.prepare_dex()
        original = json.loads((self.source / 'dex-run/jev-dex-dashboard.json').read_text())
        for proof in (None, {'verified': False}, {'verified': True, 'save_sha256': 'a'*64,
                      'expected': {'dex': {'owned': 124}}, 'restored': {'dex': {'owned': 123}}}):
            data = copy.deepcopy(original)
            data['run']['collection_continue_verification'] = proof
            self.write_dex_data(data)
            with self.assertRaisesRegex(ValueError, 'CONTINUE evidence'):
                stage(self.repo, self.site, 'abc123')

    def test_template_alone_is_not_a_publishable_dashboard(self):
        template = self.source / DEX_REQUIRED[0]
        template.parent.mkdir()
        template.write_text('template')
        stage(self.repo, self.site, 'abc123')
        self.assertFalse((self.site / 'jev-dashboard/dex-run').exists())

    def test_completion_rejects_legacy_count_only_storage_proof(self):
        import copy
        self.prepare_dex()
        original = json.loads((self.source / 'dex-run/jev-dex-dashboard.json').read_text())
        for missing_schema in (True, False):
            data = copy.deepcopy(original)
            proof = data['run']['collection_continue_verification']
            if missing_schema:
                proof.pop('schema')
            else:
                proof['expected'].pop('stored_pokemon')
                proof['restored'].pop('stored_pokemon')
            self.write_dex_data(data)
            with self.assertRaisesRegex(ValueError, 'CONTINUE evidence'):
                stage(self.repo, self.site, 'abc123')

    def test_rejects_partial_dex_before_replacing_previous_dashboard(self):
        self.prepare_dex()
        (self.source / DEX_REQUIRED[1]).unlink()
        target = self.site / 'jev-dashboard'
        target.mkdir()
        (target / 'index.html').write_text('previous dashboard')
        with self.assertRaisesRegex(ValueError, 'Missing Pokédex'):
            stage(self.repo, self.site, 'abc123')
        self.assertEqual((target / 'index.html').read_text(), 'previous dashboard')

    def test_rejects_corrupt_dex_recording(self):
        self.prepare_dex().write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            stage(self.repo, self.site, 'abc123')

    def test_rejects_incomplete_or_unresolved_run_before_replacing_site(self):
        import copy
        self.prepare_dex()
        original = json.loads((self.source / 'dex-run/jev-dex-dashboard.json').read_text())
        cases = [('run', 'success', False), ('target', 'owned', 50),
                 ('target', 'validated_owned', 123),
                 ('target', 'pending_source_validation', ['Marowak']),
                 ('collection_audit', 'pending_species', ['Marowak'])]
        target = self.site / 'jev-dashboard'
        target.mkdir()
        (target / 'index.html').write_text('previous dashboard')
        for field, key, value in cases:
            with self.subTest(field=field, key=key):
                changed = copy.deepcopy(original)
                changed[field][key] = value
                self.write_dex_data(changed)
                with self.assertRaisesRegex(ValueError, '124-species completion'):
                    stage(self.repo, self.site, 'abc123')
                self.assertEqual((target / 'index.html').read_text(), 'previous dashboard')

    def test_final_species_list_must_support_the_completion_count(self):
        self.prepare_dex()
        data = json.loads((self.source / 'dex-run/jev-dex-dashboard.json').read_text())
        data['progress'][-1]['owned_species'][-1] = 'Mon1'
        self.write_dex_data(data)
        with self.assertRaisesRegex(ValueError, 'final progress evidence'):
            stage(self.repo, self.site, 'abc123')

    def test_browser_runtime_must_equal_the_audited_json(self):
        self.prepare_dex()
        path = self.source / 'dex-run/jev-dex-dashboard-data.js'
        path.write_text('window.JEV_DEX_DASHBOARD={};')
        with self.assertRaisesRegex(ValueError, 'runtime data disagrees'):
            stage(self.repo, self.site, 'abc123')

    def test_rejects_missing_runtime_dependency(self):
        for name in ['full-run/jev-inputs-data.js', 'jev-dashboard-i18n.js']:
            with self.subTest(dependency=name):
                path = self.source / name
                content = path.read_bytes()
                path.unlink()
                with self.assertRaisesRegex(ValueError, 'Missing dashboard dependency'):
                    stage(self.repo, self.site, 'abc123')
                path.write_bytes(content)


if __name__ == '__main__':
    unittest.main()
