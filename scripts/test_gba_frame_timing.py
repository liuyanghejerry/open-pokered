import unittest
from scripts.gba_frame_timing import Evidence, SCENES, HARDWARE_SCENES, OPENING_SCENES, slow_cart_rom


def frame(scene, tick, updates=1):
    return (f"timing: frame scene={scene} tick={tick} clock={tick * 4389} "
            f"updates={updates} update=100 draw=2000 present=700 map=12")


class FrameTimingTests(unittest.TestCase):
    def test_slow_cart_patch_is_guarded_and_preserves_source(self):
        source = bytes(0x124) + bytes.fromhex("b010c0e1") + b"assets"
        result = slow_cart_rom(source)
        self.assertEqual(result[0x124:0x128], bytes.fromhex("0000a0e1"))
        self.assertEqual(source[0x124:0x128], bytes.fromhex("b010c0e1"))
        self.assertEqual(result[0x128:], b"assets")
        with self.assertRaises(ValueError):
            slow_cart_rom(result)

    def test_hardware_suite_requires_all_reported_scenes(self):
        evidence = Evidence(scenes=HARDWARE_SCENES)
        for index, scene in enumerate(HARDWARE_SCENES):
            evidence.consume(frame(scene, index * 2))
            evidence.consume(frame(scene, index * 2 + 1))
        evidence.consume("timing: DONE")
        self.assertEqual(len(evidence.report()["scenes"]), 6)

    def test_opening_suite_covers_unskipped_intro_logo_and_version(self):
        evidence = Evidence(scenes=OPENING_SCENES)
        for index, scene in enumerate(OPENING_SCENES):
            evidence.consume(frame(scene, index * 2))
            evidence.consume(frame(scene, index * 2 + 1))
        evidence.consume("timing: DONE")
        self.assertEqual(set(evidence.report()["scenes"]), {"41", "42", "43"})

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
