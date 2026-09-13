#!/usr/bin/env python3
"""Unit tests for the GBA performance-gate parser and thresholds."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shlex
import sys
import tempfile
from types import SimpleNamespace
import unittest


SCRIPT = Path(__file__).with_name("gba_performance.py")
SPEC = importlib.util.spec_from_file_location("gba_performance", SCRIPT)
assert SPEC and SPEC.loader
gba_performance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gba_performance)


def metrics(scenario: str = "overworld-movement-v1", **overrides: int) -> dict[str, int | str]:
    result: dict[str, int | str] = {
        "scenario": scenario,
        "samples": 240,
        "renders": 18,
        "update_avg_ticks": 1400,
        "draw_avg_ticks": 2000,
        "draw_per_frame_ticks": 150,
        "draw_max_ticks": 9000,
        "present_avg_ticks": 780,
        "present_per_frame_ticks": 60,
        "present_max_ticks": 800,
    }
    result.update(overrides)
    return result


def suite() -> dict[str, dict[str, int | str]]:
    return {scenario: metrics(scenario) for scenario in gba_performance.SCENARIO_METRICS}


class GbaPerformanceTests(unittest.TestCase):
    def test_parses_structured_mgba_line(self) -> None:
        line = (
            "[INFO] GBA Debug: gba-perf scenario=battle-entry-v1 samples=240 "
            "update_avg_ticks=1400 renders=18 draw_avg_ticks=2000 "
            "draw_per_frame_ticks=150 draw_max_ticks=9000 present_avg_ticks=780 "
            "present_per_frame_ticks=60 present_max_ticks=800"
        )
        self.assertEqual(
            gba_performance.parse_performance_line(line),
            metrics("battle-entry-v1"),
        )

    def test_rejects_missing_render_for_visual_scenario(self) -> None:
        line = (
            "gba-perf scenario=pokedex-entry-v1 samples=10 update_avg_ticks=10 "
            "renders=0 draw_avg_ticks=0 draw_per_frame_ticks=0 draw_max_ticks=0 "
            "present_avg_ticks=0 present_per_frame_ticks=0 present_max_ticks=0"
        )
        with self.assertRaisesRegex(ValueError, "required render"):
            gba_performance.parse_performance_line(line)

    def test_accepts_frame_reuse_for_idle_overworld(self) -> None:
        line = (
            "gba-perf scenario=overworld-idle-v1 samples=10 update_avg_ticks=10 "
            "renders=0 draw_avg_ticks=0 draw_per_frame_ticks=0 draw_max_ticks=0 "
            "present_avg_ticks=0 present_per_frame_ticks=0 present_max_ticks=0"
        )
        parsed = gba_performance.parse_performance_line(line)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["renders"], 0)

    def test_rejects_material_regression_in_one_scene(self) -> None:
        baseline = suite()
        candidate = suite()
        candidate["battle-entry-v1"] = metrics(
            "battle-entry-v1", draw_per_frame_ticks=180
        )
        failures = gba_performance.compare_metrics(baseline, candidate, 15.0, 25)
        self.assertTrue(
            any(
                "battle-entry-v1/draw_per_frame_ticks" in failure
                for failure in failures
            )
        )

    def test_allows_small_absolute_variation(self) -> None:
        baseline = suite()
        candidate = suite()
        candidate["pokedex-entry-v1"] = metrics(
            "pokedex-entry-v1", present_per_frame_ticks=82
        )
        self.assertEqual(
            gba_performance.compare_metrics(baseline, candidate, 15.0, 25), []
        )

    def test_idle_scene_gates_update_only(self) -> None:
        baseline = suite()
        candidate = suite()
        candidate["overworld-idle-v1"] = metrics(
            "overworld-idle-v1", renders=1, draw_max_ticks=50_000
        )
        self.assertEqual(
            gba_performance.compare_metrics(baseline, candidate, 15.0, 25), []
        )

    def test_load_requires_the_complete_scenario_suite(self) -> None:
        payload = {
            "suite": "autopilot-v1",
            "emulator": "mGBA test",
            "scenarios": suite(),
        }
        payload["scenarios"].pop("pokedex-entry-v1")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "metrics.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "expected scenarios"):
                gba_performance.load_metrics(path)

    def test_record_timeout_is_not_blocked_by_silent_emulator(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rom = root / "benchmark.gba"
            rom.touch()
            args = SimpleNamespace(
                rom=rom,
                emulator=(
                    f"{shlex.quote(sys.executable)} -c "
                    '"import time; time.sleep(2)"'
                ),
                timeout_seconds=0.05,
                output=root / "metrics.json",
                log=None,
            )
            with self.assertRaisesRegex(TimeoutError, "every scenario"):
                gba_performance.record(args)

    def test_ci_pins_headless_mgba_and_streams_its_output(self) -> None:
        workflow = (SCRIPT.parents[1] / ".github/workflows/gba-performance.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("runs-on: ubuntu-24.04", workflow)
        self.assertIn("MGBA_VERSION=0.10.5", workflow)
        self.assertIn(
            "0bbf1e7ca511cd4b443239b97546f699df72211241a1db9177e331866031d8e9",
            workflow,
        )
        self.assertIn("SDL_AUDIODRIVER: dummy", workflow)
        self.assertIn("MGBA_COMMAND: xvfb-run -a stdbuf -oL -eL mgba", workflow)


if __name__ == "__main__":
    unittest.main()
