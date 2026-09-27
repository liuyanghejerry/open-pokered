"""The memory gate must reject hangs, incomplete playthroughs and OOMs."""
import importlib.util
from pathlib import Path
import shlex
import sys
import tempfile
from types import SimpleNamespace
import unittest

SPEC = importlib.util.spec_from_file_location("gba_memory", Path(__file__).with_name("gba_memory.py"))
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class GbaMemoryTests(unittest.TestCase):
    def completed(self):
        evidence = module.Evidence()
        evidence.consume("repro: heap low 7600B at frame 1234")
        for i in range(module.ROUNDS):
            evidence.consume(f"route22: PASS round={i + 1} case={module.CASES[i % len(module.CASES)]}")
        return evidence

    def test_requires_encounters_and_save(self):
        evidence = self.completed()
        self.assertFalse(evidence.complete)
        evidence.consume(f"route22: ALL PASS rounds={module.ROUNDS} save=ok stack=18000B")
        self.assertTrue(evidence.complete)
        self.assertEqual((evidence.min_heap, evidence.min_stack), (7600, 18000))

    def test_rejects_false_completion(self):
        with self.assertRaisesRegex(ValueError, "Incomplete"):
            module.Evidence().consume(f"route22: ALL PASS rounds={module.ROUNDS} save=ok stack=18000B")

    def test_rejects_skipped_or_wrong_case(self):
        for line in ["route22: PASS round=2 case=before-parcel-upper",
                     "route22: PASS round=1 case=final-lower"]:
            with self.subTest(line=line), self.assertRaisesRegex(ValueError, "incorrect encounter"):
                module.Evidence().consume(line)

    def test_rejects_runtime_failure_and_low_memory(self):
        for line in ["memory allocation of 23040 bytes failed", "panicked at alloc.rs",
                     "repro: heap low 4095B", "route22: frame=100 stack=4095B"]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                module.Evidence().consume(line)

    def test_requires_heap_evidence(self):
        evidence = self.completed()
        evidence.min_heap = None
        with self.assertRaisesRegex(ValueError, "Missing memory"):
            evidence.consume(f"route22: ALL PASS rounds={module.ROUNDS} save=ok stack=18000B")

    def test_scenario_suite_requires_each_workload_and_save_roundtrip(self):
        evidence = module.Evidence("scenarios")
        evidence.consume("repro: heap low 12000B at frame 5000")
        for name, count in module.SCENARIOS:
            evidence.consume(f"memory: PASS case={name} count={count} free=12000B stack=17000B")
        self.assertFalse(evidence.complete)
        evidence.consume("memory: ALL PASS stack=17000B")
        self.assertTrue(evidence.complete)

    def test_scenario_suite_rejects_skips_wrong_counts_and_wrong_rom(self):
        for line in ["memory: PASS case=maps count=248 stack=17000B",
                     "memory: PASS case=pokedex count=151 stack=17000B",
                     "memory: ALL PASS stack=17000B"]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                module.Evidence("scenarios").consume(line)
        evidence = self.completed()
        evidence.consume("memory: ALL PASS stack=17000B")
        self.assertFalse(evidence.complete)

    def test_scenario_stack_margin_is_checked(self):
        with self.assertRaisesRegex(ValueError, "stack margin"):
            module.Evidence("scenarios").consume("memory: PASS case=pokedex count=302 free=10000B stack=4092B")

    def test_silent_emulator_times_out_and_preserves_user_save(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rom = root / "player.gba"
            rom.write_bytes(b"rom")
            save = rom.with_suffix(".sav")
            save.write_bytes(b"player save")
            fake = root / "emulator.py"
            fake.write_text("import sys,time\nprint('boot without newline', end='', flush=True)\ntime.sleep(30)\n")
            args = SimpleNamespace(rom=rom, log=root / "evidence.log", timeout_seconds=0.2,
                emulator=f"{shlex.quote(sys.executable)} {shlex.quote(str(fake))} {{rom}}")
            with self.assertRaises(TimeoutError):
                module.run(args)
            self.assertEqual(save.read_bytes(), b"player save")
            self.assertIn("boot without newline", args.log.read_text())


if __name__ == "__main__":
    unittest.main()
