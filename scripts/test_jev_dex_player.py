from pathlib import Path
import shutil
import subprocess
import unittest


class DexPlayerTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('node'), 'Node is required to exercise player JavaScript')
    def test_actual_browser_script_audit_and_waypoint_contracts(self):
        result = subprocess.run(['node', str(Path(__file__).with_suffix('.cjs'))],
                                capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == '__main__':
    unittest.main()
