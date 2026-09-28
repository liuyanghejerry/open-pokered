import unittest
from scripts.gba_frame_timing import Evidence, SCENES


def frame(scene, tick, updates=1):
    return (f"timing: frame scene={scene} tick={tick} clock={tick * 4389} "
            f"updates={updates} update=100 draw=2000 present=700 map=12")


class FrameTimingTests(unittest.TestCase):
    def test_full_suite_measures_draws_and_frame_gaps(self):
        evidence = Evidence()
        for index, scene in enumerate(SCENES):
            evidence.consume(frame(scene, index * 2))
            evidence.consume(frame(scene, index * 2 + 1))
        evidence.consume("timing: DONE tick=100")
        self.assertTrue(evidence.complete)
        self.assertEqual(evidence.report()["scenes"]["10"]["logic_ticks"], 2)
        self.assertAlmostEqual(evidence.report()["scenes"]["1"]["display_gap_max_ms"], 16.7427, places=3)

    def test_skipped_animation_ticks_fail(self):
        with self.assertRaisesRegex(ValueError, "Invisible"):
            Evidence().consume(frame(10, 4, 4))

    def test_old_loop_can_be_recorded_as_evidence(self):
        evidence = Evidence(allow_skips=True)
        evidence.consume(frame(10, 4, 4))
        self.assertEqual(evidence.rows[10][0]["updates"], 4)

    def test_missing_or_malformed_samples_fail(self):
        for line in ["timing: DONE", "timing: frame scene=10 tick=1", frame(99, 1)]:
            with self.assertRaises(ValueError):
                Evidence().consume(line)

    def test_runtime_failure_and_truncated_log_fail(self):
        with self.assertRaises(ValueError):
            Evidence().consume("panicked at src/main.rs")
        with self.assertRaises(ValueError):
            Evidence().report()


if __name__ == "__main__":
    unittest.main()
