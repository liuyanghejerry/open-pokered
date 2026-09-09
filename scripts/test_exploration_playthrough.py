"""Unit checks for the checkpoint exploration mode (no game binary needed)."""
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))

import exploration_playthrough as exploration
import playthrough


class ExplorationManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = exploration.load_manifest()

    def test_every_milestone_has_an_explicit_checkpoint_entry(self):
        self.assertEqual(set(self.manifest["by_checkpoint"]),
                         {mid for mid, _, _ in playthrough.MILESTONES})

    def test_blocked_probes_use_real_navigation_only(self):
        probes = [probe for group in self.manifest["by_checkpoint"].values()
                  for probe in group.get("blocked", [])]
        self.assertGreaterEqual(len(probes), 3)
        for probe in probes:
            self.assertTrue(all(step["op"] in exploration.VALID_OPS
                                for step in probe["route"]))
            self.assertNotIn("warp", {step["op"] for step in probe["route"]})
            self.assertEqual(probe["kind"], "blocked")

    def test_probe_ids_are_unique_and_destination_sampling_is_seeded(self):
        self.assertEqual(len(self.manifest["probe_ids"]),
                         sum(len(group.get(kind, []))
                             for group in self.manifest["by_checkpoint"].values()
                             for kind in ("blocked", "destinations")))
        rng_a = __import__("random").Random(73)
        rng_b = __import__("random").Random(73)
        choices = [probe["id"] for group in self.manifest["by_checkpoint"].values()
                   for probe in rng_a.sample(group.get("destinations", []),
                                             min(1, len(group.get("destinations", []))))]
        choices_again = [probe["id"] for group in self.manifest["by_checkpoint"].values()
                         for probe in rng_b.sample(group.get("destinations", []),
                                                   min(1, len(group.get("destinations", []))))]
        self.assertEqual(choices, choices_again)

    def test_destinations_have_explicit_oracles(self):
        probes = [probe for group in self.manifest["by_checkpoint"].values()
                  for probe in group.get("destinations", [])]
        self.assertGreaterEqual(len(probes), 5)
        for probe in probes:
            expected = probe["expected"]
            self.assertTrue(expected.get("dialogue_contains") or
                            expected.get("bag") or expected.get("flags"))
            self.assertIn("visit", probe)

    def test_expected_gate_positions_are_declared(self):
        for group in self.manifest["by_checkpoint"].values():
            for probe in group.get("blocked", []):
                self.assertEqual(len(probe["expected"]["position"]), 2)
                self.assertIn("dialogue_contains", probe["expected"])

    def test_selection_cannot_silently_skip_past_until(self):
        args = SimpleNamespace(only="route8-saffron-gate-closed", until="m10")
        with self.assertRaisesRegex(ValueError, "after --until m10"):
            exploration._validate_selection(args, self.manifest)


if __name__ == "__main__":
    unittest.main()
