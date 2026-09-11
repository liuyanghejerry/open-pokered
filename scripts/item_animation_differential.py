#!/usr/bin/env python3
"""Frame-by-frame differential for every Gen-I item battle animation branch.

The pinned DEBUG ROM is used only to prepare a battle. Compared frames execute
in the pinned retail Red ROM. Ball scenarios enter the original
``TossBallAnimation`` with the exact ``wCurItem`` / ``wPokeBallAnimData`` /
``wIsInBattle`` tuple; the current side enters the equivalent semantic
``BattleAnimEvent`` through pokered-app's production renderer.
"""

from __future__ import annotations

import argparse
import gzip
import json
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from move_animation_differential import (
    PINNED_ROM_SHA1,
    PINNED_SETUP_ROM_SHA1,
    PINNED_SOURCE_COMMIT,
    PINNED_SYMBOLS_SHA1,
    ReferenceRunner,
    compare_side,
    expanded_rle,
    require_empty_output,
    require_pinned,
    rle,
    sha1_file,
    source_commit,
    summarize_capture,
    write_json,
)


# The original constants are offsets in SFX_Headers_1 and therefore do not
# share pokered-audio's de-duplicated enum values. Normalize the five sounds
# reachable from this audit to their semantic track names before comparing.
REFERENCE_SFX_NAMES = {
    140: "Tink",
    145: "BallToss",
    147: "BallPoof",
    149: "FaintThud",
    221: "Battle32",
}


@dataclass(frozen=True)
class Case:
    key: str
    current_scenario: str
    animation_id: int
    side: str = "player"
    ball: str | None = None
    ball_id: int | None = None
    shakes: int = 0
    outcome: str | None = None
    anim_data: int | None = None
    in_battle: int = 1

    def memory_overrides(self) -> dict[str, int]:
        if self.ball_id is None:
            return {}
        return {
            "wCurItem": self.ball_id,
            "wPokeBallAnimData": self.anim_data if self.anim_data is not None else 0x43,
            "wIsInBattle": self.in_battle,
        }


def cases() -> list[Case]:
    result = [
        Case("x-stat-player", "x-stat-player", 0xAE),
        Case("x-stat-enemy", "x-stat-enemy", 0xAF, side="enemy"),
        Case("safari-bait", "safari-bait", 0xCA),
        Case("safari-rock", "safari-rock", 0xC9),
    ]
    balls = (
        ("master", 0x01, False, True),
        ("ultra", 0x02, True, True),
        ("great", 0x03, True, True),
        ("poke", 0x04, True, True),
        ("safari", 0x08, True, False),
    )
    for ball, ball_id, can_fail, can_special_target in balls:
        result.append(
            Case(
                f"{ball}-caught",
                "ball-caught",
                0xC1,
                ball=ball,
                ball_id=ball_id,
                shakes=3,
                outcome="caught",
                anim_data=0x43,
            )
        )
        if can_fail:
            result.append(
                Case(
                    f"{ball}-missed",
                    "ball-broke-free",
                    0xC1,
                    ball=ball,
                    ball_id=ball_id,
                    outcome="broke-free",
                    anim_data=0x20,
                )
            )
            for shakes in (1, 2, 3):
                result.append(
                    Case(
                        f"{ball}-broke-free-{shakes}",
                        "ball-broke-free",
                        0xC1,
                        ball=ball,
                        ball_id=ball_id,
                        shakes=shakes,
                        outcome="broke-free",
                        anim_data=0x60 + shakes,
                    )
                )
        if can_special_target:
            result.extend(
                (
                    Case(
                        f"{ball}-ghost-dodged",
                        "ball-dodged",
                        0xC1,
                        ball=ball,
                        ball_id=ball_id,
                        outcome="dodged",
                        anim_data=0x10,
                    ),
                    Case(
                        f"{ball}-trainer-blocked",
                        "ball-blocked",
                        0xC1,
                        ball=ball,
                        ball_id=ball_id,
                        outcome="blocked",
                        in_battle=2,
                    ),
                )
            )
    return result


def select_cases(spec: str) -> list[Case]:
    available = {case.key: case for case in cases()}
    if spec == "all":
        return list(available.values())
    keys = [part.strip() for part in spec.split(",") if part.strip()]
    unknown = sorted(set(keys) - available.keys())
    if unknown:
        raise ValueError(f"unknown cases: {unknown}; valid: {sorted(available)}")
    return [available[key] for key in keys]


def load_current_capture(path: Path) -> dict[str, Any]:
    manifest = json.loads((path / "manifest.json").read_text())
    pixel_trace = []
    rendered_oam = []
    source_oam = []
    sfx_events = []
    for frame in manifest["frames"][1:]:
        pixel_trace.append(frame["pixel"])
        rendered_oam.append(frame["state"]["oam"])
        source_oam.append(frame["state"].get("source_oam", frame["state"]["oam"]))
        for event in frame.get("sfx", []):
            sfx_events.append(
                {
                    "capture_index": frame["capture_index"],
                    "name": event.get("name", f"Unknown{event.get('id')}"),
                }
            )
    result = summarize_capture(pixel_trace, rendered_oam, source_oam=source_oam)
    result["sfx_events"] = sfx_events
    return result


def capture_current(
    binary: Path,
    case: Case,
    max_frames: int,
    evidence_dir: Path | None,
) -> dict[str, Any]:
    if evidence_dir is None:
        temporary = tempfile.TemporaryDirectory(prefix="item-anim-current-")
        target = Path(temporary.name) / "capture"
    else:
        temporary = None
        target = evidence_dir
    command = [
        str(binary),
        "item-animation-frames",
        "--scenario",
        case.current_scenario,
        "--output-dir",
        str(target),
        "--max-frames",
        str(max_frames),
        "--shakes",
        str(case.shakes),
    ]
    if case.ball is not None:
        command.extend(("--ball", case.ball))
    if evidence_dir is None:
        command.append("--manifest-only")
    completed = subprocess.run(command, text=True, capture_output=True)
    if completed.returncode != 0:
        raise RuntimeError(
            f"current capture failed for {case.key}:\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    result = load_current_capture(target)
    if temporary is not None:
        temporary.cleanup()
    return result


def compare(reference: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    comparison = compare_side(reference, current)
    reference_sfx = [
        {
            "capture_index": event["capture_index"],
            "name": REFERENCE_SFX_NAMES.get(event["id"], f"OriginalSfx{event['id']}"),
        }
        for event in reference.get("sfx_events", [])
    ]
    current_sfx = current.get("sfx_events", [])
    comparison["sfx_match"] = reference_sfx == current_sfx
    comparison["reference_sfx"] = reference_sfx
    comparison["current_sfx"] = current_sfx
    if not comparison["sfx_match"]:
        comparison["issues"].append("sfx")
        comparison["verdict"] = "FAIL"
    return comparison


def compact_result(result: dict[str, Any]) -> dict[str, Any]:
    compact = dict(result)
    for side in ("reference", "current"):
        capture = compact.pop(side)
        compact[side] = {
            "duration_frames": capture["duration_frames"],
            "trace_sha1": capture["trace_sha1"],
            "max_rendered_objects": capture["max_rendered_objects"],
            "sfx_events": capture.get("sfx_events", []),
        }
    return compact


def report(summary: dict[str, Any]) -> str:
    totals = summary["totals"]
    lines = [
        "# 道具战斗动画逐帧差分",
        "",
        f"- 原版：`pret/pokered@{summary['provenance']['reference_source_commit']}`",
        f"- 正式 Red ROM SHA-1：`{summary['provenance']['reference_rom_sha1']}`",
        f"- 范围：{totals['cases']} 条语义轨迹，每条重复 {summary['scope']['repeat']} 次",
        "- 判定：持续帧数、实际 OAM、动态像素掩码、SFX ID 与触发帧全部精确相同",
        "",
        "## 结论",
        "",
        f"- PASS：{totals['pass']}",
        f"- FAIL：{totals['fail']}",
        f"- 不确定：{totals['inconclusive']}",
        "",
        "## 逐场景结果",
        "",
        "| 场景 | 原版→当前帧数 | OAM | 像素 | SFX | 结论 |",
        "|---|---:|---|---|---|---|",
    ]
    for item in summary["results"]:
        lines.append(
            f"| `{item['case']}` | {item['reference_frames']}→{item['current_frames']} | "
            f"{'✓' if item['rendered_oam_match'] else '✗'} | "
            f"{'✓' if item['dynamic_mask_match'] else '✗'} | "
            f"{'✓' if item['sfx_match'] else '✗'} | {item['verdict']} |"
        )
    lines.extend(
        (
            "",
            "完整 RLE 逐帧轨迹保存在 `frame-traces.json.gz`；`evidence/` 只保存通过 "
            "`--keep-frames` 选择的连续原版/当前 PNG 窗口。",
            "",
        )
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-rom", type=Path, required=True)
    parser.add_argument("--setup-rom", type=Path, required=True)
    parser.add_argument("--reference-symbols", type=Path, required=True)
    parser.add_argument("--reference-source", type=Path, required=True)
    parser.add_argument("--current-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cases", default="all")
    parser.add_argument("--repeat", type=int, default=2)
    parser.add_argument("--max-frames", type=int, default=3000)
    parser.add_argument(
        "--keep-frames",
        default="x-stat-player,safari-rock,poke-caught,ultra-broke-free-2,poke-trainer-blocked",
    )
    args = parser.parse_args()

    require_pinned(args.reference_rom, PINNED_ROM_SHA1, "reference ROM")
    require_pinned(args.setup_rom, PINNED_SETUP_ROM_SHA1, "setup ROM")
    require_pinned(args.reference_symbols, PINNED_SYMBOLS_SHA1, "reference symbols")
    commit = source_commit(args.reference_source)
    if commit != PINNED_SOURCE_COMMIT:
        raise RuntimeError(
            f"reference source commit mismatch: expected {PINNED_SOURCE_COMMIT}, got {commit}"
        )
    if not args.current_binary.is_file():
        raise RuntimeError(f"current binary does not exist: {args.current_binary}")
    if args.repeat < 2:
        raise ValueError("repeat must be at least 2 before PASS can be reported")
    require_empty_output(args.output)

    selected = select_cases(args.cases)
    keep = set(part.strip() for part in args.keep_frames.split(",") if part.strip())
    known = {case.key for case in cases()}
    unknown_keep = sorted(keep - known)
    if unknown_keep:
        raise ValueError(f"unknown --keep-frames cases: {unknown_keep}")

    summary: dict[str, Any] = {
        "schema": 1,
        "provenance": {
            "reference_rom": str(args.reference_rom.resolve()),
            "reference_rom_sha1": sha1_file(args.reference_rom),
            "setup_rom": str(args.setup_rom.resolve()),
            "setup_rom_sha1": sha1_file(args.setup_rom),
            "reference_symbols": str(args.reference_symbols.resolve()),
            "reference_symbols_sha1": sha1_file(args.reference_symbols),
            "reference_source": str(args.reference_source.resolve()),
            "reference_source_commit": commit,
            "current_binary": str(args.current_binary.resolve()),
            "current_git_head": subprocess.run(
                ["git", "rev-parse", "HEAD"], check=True, text=True, capture_output=True
            ).stdout.strip(),
        },
        "scope": {
            "cases": [case.key for case in selected],
            "repeat": args.repeat,
            "max_frames": args.max_frames,
            "kept_continuous_frames": sorted(keep & {case.key for case in selected}),
        },
        "results": [],
    }

    runner = ReferenceRunner(
        args.reference_rom, args.setup_rom, args.reference_symbols, args.max_frames
    )
    try:
        for position, case in enumerate(selected, 1):
            reference_runs = []
            current_runs = []
            for run_index in range(1, args.repeat + 1):
                retain = run_index == 1 and case.key in keep
                reference_runs.append(
                    runner.capture(
                        case.animation_id,
                        case.side,
                        args.output / "evidence" / case.key / "reference" if retain else None,
                        memory_overrides=case.memory_overrides(),
                    )
                )
                current_runs.append(
                    capture_current(
                        args.current_binary,
                        case,
                        args.max_frames,
                        args.output / "evidence" / case.key / "current" if retain else None,
                    )
                )
            reference_deterministic = len({run["trace_sha1"] for run in reference_runs}) == 1
            current_deterministic = len({run["trace_sha1"] for run in current_runs}) == 1
            result = compare(reference_runs[0], current_runs[0])
            if not reference_deterministic or not current_deterministic:
                result["verdict"] = "INCONCLUSIVE"
                result["issues"].append("nondeterministic_repeat")
            result.update(
                {
                    "case": case.key,
                    "animation_id": case.animation_id,
                    "ball_id": case.ball_id,
                    "anim_data": case.anim_data,
                    "reference_deterministic": reference_deterministic,
                    "current_deterministic": current_deterministic,
                    "reference_repeat_sha1": [run["trace_sha1"] for run in reference_runs],
                    "current_repeat_sha1": [run["trace_sha1"] for run in current_runs],
                    "reference": reference_runs[0],
                    "current": current_runs[0],
                }
            )
            summary["results"].append(result)
            write_json(args.output / "summary.partial.json", summary)
            print(
                f"[{position:02}/{len(selected):02}] {case.key}: {result['verdict']} "
                f"{result['reference_frames']}→{result['current_frames']} "
                f"issues={result['issues']}",
                flush=True,
            )
    finally:
        runner.close()

    summary["totals"] = {
        "cases": len(summary["results"]),
        "pass": sum(item["verdict"] == "PASS" for item in summary["results"]),
        "fail": sum(item["verdict"] == "FAIL" for item in summary["results"]),
        "inconclusive": sum(
            item["verdict"] == "INCONCLUSIVE" for item in summary["results"]
        ),
        "duration_mismatch": sum(not item["duration_match"] for item in summary["results"]),
        "rendered_oam_mismatch": sum(
            not item["rendered_oam_match"] for item in summary["results"]
        ),
        "dynamic_mask_mismatch": sum(
            not item["dynamic_mask_match"] for item in summary["results"]
        ),
        "sfx_mismatch": sum(not item["sfx_match"] for item in summary["results"]),
    }
    trace_bytes = (json.dumps(summary, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    with gzip.GzipFile(
        filename=str(args.output / "frame-traces.json.gz"), mode="wb", mtime=0
    ) as archive:
        archive.write(trace_bytes)
    compact = dict(summary)
    compact["results"] = [compact_result(item) for item in summary["results"]]
    compact["frame_trace_archive"] = "frame-traces.json.gz"
    write_json(args.output / "summary.json", compact)
    (args.output / "report.md").write_text(report(compact))
    (args.output / "summary.partial.json").unlink(missing_ok=True)
    print(json.dumps(summary["totals"], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
