import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.stage_jev_dashboard import ASSETS, LFS_PREFIX, REQUIRED, stage


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
        self.assertIn('location.hash', (target / 'index.html').read_text())

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

    def test_rejects_missing_runtime_dependency(self):
        (self.source / 'full-run/jev-inputs-data.js').unlink()
        with self.assertRaisesRegex(ValueError, 'Missing dashboard dependency'):
            stage(self.repo, self.site, 'abc123')


if __name__ == '__main__':
    unittest.main()
