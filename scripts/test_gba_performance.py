#!/usr/bin/env python3
"""Unit tests for the GBA performance-gate parser and thresholds."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("gba_performance.py")
SPEC = importlib.util.spec_from_file_location("gba_performance", SCRIPT)
assert SPEC and SPEC.loader
gba_performance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gba_performance)


def metrics(**overrides: int) -> dict[str, int | str]:
    result: dict[str, int | str] = {
        "scenario": "overworld-autopilot-v1",
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


class GbaPerformanceTests(unittest.TestCase):
    def test_parses_structured_mgba_line(self) -> None:
        line = (
            "[INFO] GBA Debug: gba-perf scenario=overworld-autopilot-v1 samples=240 "
            "update_avg_ticks=1400 renders=18 draw_avg_ticks=2000 "
            "draw_per_frame_ticks=150 draw_max_ticks=9000 present_avg_ticks=780 "
            "present_per_frame_ticks=60 present_max_ticks=800"
        )
        self.assertEqual(gba_performance.parse_performance_line(line), metrics())

    def test_rejects_material_regression(self) -> None:
        candidate = metrics(draw_per_frame_ticks=180)
        failures = gba_performance.compare_metrics(metrics(), candidate, 15.0, 25)
        self.assertTrue(any("draw_per_frame_ticks" in failure for failure in failures))

    def test_allows_small_absolute_variation(self) -> None:
        candidate = metrics(present_per_frame_ticks=82)
        self.assertEqual(gba_performance.compare_metrics(metrics(), candidate, 15.0, 25), [])


if __name__ == "__main__":
    unittest.main()
