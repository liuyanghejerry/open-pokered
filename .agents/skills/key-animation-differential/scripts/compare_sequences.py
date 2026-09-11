#!/usr/bin/env python3
"""Compare the raw timing and background motion of two PNG frame sequences.

The caller must trim both ranges to the same semantic window, such as trigger
input through the first stable post-animation frame.  The ROI should contain
only stable background landmarks; exclude the player, UI, water, and flowers.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

try:
    from PIL import Image, ImageChops, ImageDraw, ImageFont
except ImportError as exc:  # pragma: no cover - exercised by real environments
    raise SystemExit(
        "Pillow is required. Install it in a temporary venv, then run this script "
        "with that venv's python."
    ) from exc


FRAME_NUMBER = re.compile(r"(\d+)(?=\.png$)")


def parse_range(value: str) -> tuple[int, int]:
    try:
        start_text, end_text = value.split(":", 1)
        start, end = int(start_text), int(end_text)
    except (ValueError, TypeError) as exc:
        raise argparse.ArgumentTypeError("range must be START:END (inclusive)") from exc
    if start < 0 or end < start:
        raise argparse.ArgumentTypeError("range must satisfy 0 <= START <= END")
    return start, end


def parse_roi(value: str) -> tuple[int, int, int, int]:
    try:
        x, y, width, height = (int(part) for part in value.split(","))
    except (ValueError, TypeError) as exc:
        raise argparse.ArgumentTypeError("ROI must be X,Y,WIDTH,HEIGHT") from exc
    if x < 0 or y < 0 or width <= 0 or height <= 0:
        raise argparse.ArgumentTypeError("ROI coordinates must be non-negative and sized > 0")
    return x, y, width, height


def indexed_frames(directory: Path) -> dict[int, Path]:
    result: dict[int, Path] = {}
    for path in directory.glob("*.png"):
        match = FRAME_NUMBER.search(path.name)
        if not match:
            continue
        number = int(match.group(1))
        if number in result:
            raise ValueError(f"duplicate frame number {number} in {directory}")
        result[number] = path
    return result


def load_range(
    directory: Path,
    frame_range: tuple[int, int],
    roi: tuple[int, int, int, int],
) -> tuple[list[int], list[Image.Image], list[int]]:
    frames = indexed_frames(directory)
    start, end = frame_range
    requested = list(range(start, end + 1))
    missing = [number for number in requested if number not in frames]
    present = [number for number in requested if number in frames]
    if len(present) < 2:
        raise ValueError(f"need at least two frames in {directory} range {start}:{end}")

    x, y, width, height = roi
    images: list[Image.Image] = []
    expected_size: tuple[int, int] | None = None
    for number in present:
        with Image.open(frames[number]) as source:
            image = source.convert("L")
        if expected_size is None:
            expected_size = image.size
            if x + width > image.width or y + height > image.height:
                raise ValueError(
                    f"ROI {roi} exceeds frame size {image.width}x{image.height}"
                )
        elif image.size != expected_size:
            raise ValueError(
                f"frame {number} has size {image.size}, expected {expected_size}"
            )
        images.append(image.crop((x, y, x + width, y + height)))
    return present, images, missing


def difference_metrics(left: Image.Image, right: Image.Image) -> tuple[float, float]:
    histogram = ImageChops.difference(left, right).histogram()
    pixels = left.width * left.height
    mean = sum(value * count for value, count in enumerate(histogram)) / pixels
    changed_fraction = sum(histogram[8:]) / pixels
    return mean, changed_fraction


def translation_residual(
    previous: Image.Image,
    current: Image.Image,
    dx: int,
    dy: int,
) -> float:
    width, height = previous.size
    prev_x0 = max(0, -dx)
    prev_y0 = max(0, -dy)
    prev_x1 = min(width, width - dx)
    prev_y1 = min(height, height - dy)
    if prev_x1 <= prev_x0 or prev_y1 <= prev_y0:
        return float("inf")

    prev_crop = previous.crop((prev_x0, prev_y0, prev_x1, prev_y1))
    curr_crop = current.crop(
        (prev_x0 + dx, prev_y0 + dy, prev_x1 + dx, prev_y1 + dy)
    )
    residual, _ = difference_metrics(prev_crop, curr_crop)
    return residual


def best_translation(
    previous: Image.Image,
    current: Image.Image,
    max_dx: int,
    max_dy: int,
) -> tuple[int, int, float]:
    candidates: list[tuple[float, int, int, int]] = []
    for dy in range(-max_dy, max_dy + 1):
        for dx in range(-max_dx, max_dx + 1):
            residual = translation_residual(previous, current, dx, dy)
            candidates.append((residual, abs(dx) + abs(dy), dx, dy))
    residual, _, dx, dy = min(candidates)
    return dx, dy, residual


def analyze_sequence(
    label: str,
    directory: Path,
    frame_range: tuple[int, int],
    roi: tuple[int, int, int, int],
    max_dx: int,
    max_dy: int,
    min_improvement: float,
    max_residual: float,
) -> dict[str, object]:
    numbers, images, missing = load_range(directory, frame_range, roi)
    transitions: list[dict[str, object]] = []
    net_dx = 0
    net_dy = 0
    path_l1 = 0
    translated_steps = 0
    largest_step = 0

    for previous_number, current_number, previous, current in zip(
        numbers, numbers[1:], images, images[1:]
    ):
        raw_mad, changed_fraction = difference_metrics(previous, current)
        dx, dy, residual = best_translation(previous, current, max_dx, max_dy)
        improvement = raw_mad - residual
        accepted = (
            (dx != 0 or dy != 0)
            and improvement >= min_improvement
            and residual <= max_residual
        )
        if accepted:
            net_dx += dx
            net_dy += dy
            step_l1 = abs(dx) + abs(dy)
            path_l1 += step_l1
            translated_steps += 1
            largest_step = max(largest_step, step_l1)

        transitions.append(
            {
                "from": previous_number,
                "to": current_number,
                "raw_mad": round(raw_mad, 6),
                "changed_fraction": round(changed_fraction, 6),
                "best_translation": {
                    "dx": dx,
                    "dy": dy,
                    "residual_mad": round(residual, 6),
                    "improvement": round(improvement, 6),
                    "accepted": accepted,
                },
            }
        )

    smoothness = largest_step / path_l1 if path_l1 else 0.0
    return {
        "label": label,
        "directory": str(directory),
        "requested_range": {"start": frame_range[0], "end": frame_range[1]},
        "present_frame_count": len(numbers),
        "missing_frames": missing,
        "background_motion": {
            "translated_steps": translated_steps,
            "net_dx": net_dx,
            "net_dy": net_dy,
            "path_l1": path_l1,
            "largest_step_l1": largest_step,
            "largest_step_share": round(smoothness, 6),
        },
        "transitions": transitions,
    }


def compare(
    reference: dict[str, object],
    current: dict[str, object],
    duration_tolerance: float,
    path_total_tolerance: int,
    max_step_delta: int,
    smoothness_tolerance: float,
) -> dict[str, object]:
    checks: list[dict[str, object]] = []

    def add(name: str, passed: bool, detail: str) -> None:
        checks.append({"name": name, "status": "PASS" if passed else "FAIL", "detail": detail})

    ref_count = int(reference["present_frame_count"])
    cur_count = int(current["present_frame_count"])
    duration_ratio = cur_count / ref_count
    add(
        "contiguous_frame_ids",
        not reference["missing_frames"] and not current["missing_frames"],
        f"reference missing={reference['missing_frames']}; current missing={current['missing_frames']}",
    )
    add(
        "raw_duration",
        abs(duration_ratio - 1.0) <= duration_tolerance,
        f"reference={ref_count} frames; current={cur_count} frames; ratio={duration_ratio:.3f}",
    )

    ref_motion = reference["background_motion"]
    cur_motion = current["background_motion"]
    assert isinstance(ref_motion, dict) and isinstance(cur_motion, dict)
    path_delta = abs(int(cur_motion["path_l1"]) - int(ref_motion["path_l1"]))
    add(
        "background_path_length",
        path_delta <= path_total_tolerance,
        f"reference={ref_motion['path_l1']} px; current={cur_motion['path_l1']} px; delta={path_delta} px",
    )

    step_delta = abs(
        int(cur_motion["largest_step_l1"]) - int(ref_motion["largest_step_l1"])
    )
    add(
        "largest_background_step",
        step_delta <= max_step_delta,
        f"reference={ref_motion['largest_step_l1']} px; current={cur_motion['largest_step_l1']} px; delta={step_delta} px",
    )

    smoothness_delta = abs(
        float(cur_motion["largest_step_share"])
        - float(ref_motion["largest_step_share"])
    )
    add(
        "background_motion_distribution",
        smoothness_delta <= smoothness_tolerance,
        "largest-step/path share: "
        f"reference={ref_motion['largest_step_share']}; "
        f"current={cur_motion['largest_step_share']}; delta={smoothness_delta:.3f}",
    )

    def ordered_motion(sequence: dict[str, object]) -> list[tuple[int, int]]:
        ordered: list[tuple[int, int]] = []
        transitions = sequence["transitions"]
        assert isinstance(transitions, list)
        for transition in transitions:
            assert isinstance(transition, dict)
            best = transition["best_translation"]
            assert isinstance(best, dict)
            ordered.append(
                (int(best["dx"]), int(best["dy"]))
                if best["accepted"]
                else (0, 0)
            )
        return ordered

    ref_ordered = ordered_motion(reference)
    cur_ordered = ordered_motion(current)
    first_mismatch = next(
        (
            index
            for index, (ref_step, cur_step) in enumerate(
                zip(ref_ordered, cur_ordered)
            )
            if ref_step != cur_step
        ),
        None,
    )
    cadence_matches = ref_ordered == cur_ordered
    cadence_detail = f"{len(ref_ordered)} reference / {len(cur_ordered)} current transitions"
    if first_mismatch is not None:
        cadence_detail += (
            f"; first mismatch at t+{first_mismatch}->t+{first_mismatch + 1}: "
            f"reference={ref_ordered[first_mismatch]}, current={cur_ordered[first_mismatch]}"
        )
    elif len(ref_ordered) != len(cur_ordered):
        cadence_detail += f"; first mismatch at t+{min(len(ref_ordered), len(cur_ordered))}"
    add("ordered_background_cadence", cadence_matches, cadence_detail)

    return {
        "verdict": "PASS" if all(check["status"] == "PASS" for check in checks) else "FAIL",
        "duration_ratio": round(duration_ratio, 6),
        "checks": checks,
    }


def cumulative_motion(sequence: dict[str, object], axis: str) -> list[int]:
    value = 0
    values = [value]
    transitions = sequence["transitions"]
    assert isinstance(transitions, list)
    for transition in transitions:
        assert isinstance(transition, dict)
        best = transition["best_translation"]
        assert isinstance(best, dict)
        if best["accepted"]:
            value += int(best[axis])
        values.append(value)
    return values


def render_diagnostic(
    path: Path,
    reference_dir: Path,
    current_dir: Path,
    reference_range: tuple[int, int],
    current_range: tuple[int, int],
    reference: dict[str, object],
    current: dict[str, object],
    comparison: dict[str, object],
    samples: int,
) -> None:
    if samples < 2:
        raise ValueError("--samples must be at least 2")

    ref_count = int(reference["present_frame_count"])
    cur_count = int(current["present_frame_count"])
    max_count = max(ref_count, cur_count)
    elapsed_samples = sorted(
        {round(index * (max_count - 1) / (samples - 1)) for index in range(samples)}
    )
    ref_paths = indexed_frames(reference_dir)
    cur_paths = indexed_frames(current_dir)

    cell_width, cell_height = 160, 144
    label_height = 18
    chart_height = 190
    width = cell_width * len(elapsed_samples)
    height = label_height + 2 * (label_height + cell_height) + chart_height
    canvas = Image.new("RGB", (width, height), "#d0d0d0")
    draw = ImageDraw.Draw(canvas)
    font = ImageFont.load_default()
    draw.rectangle((0, 0, width, label_height), fill="white")
    draw.text(
        (4, 4),
        "RAW-TIME ALIGNMENT: same elapsed-frame offsets; no temporal resampling",
        fill="black",
        font=font,
    )

    def draw_row(
        row_index: int,
        label: str,
        directory_frames: dict[int, Path],
        frame_range: tuple[int, int],
        frame_count: int,
    ) -> None:
        row_y = label_height + row_index * (label_height + cell_height)
        draw.rectangle((0, row_y, width, row_y + label_height), fill="white")
        draw.text((4, row_y + 4), label, fill="black", font=font)
        for column, elapsed in enumerate(elapsed_samples):
            x = column * cell_width
            y = row_y + label_height
            if elapsed < frame_count:
                frame_number = frame_range[0] + elapsed
                with Image.open(directory_frames[frame_number]) as source:
                    tile = source.convert("RGB")
                canvas.paste(tile, (x, y))
            else:
                draw.rectangle(
                    (x, y, x + cell_width - 1, y + cell_height - 1),
                    fill="#888888",
                )
                draw.text((x + 60, y + 66), "ENDED", fill="white", font=font)
            draw.rectangle((x, y, x + 42, y + 13), fill="white")
            draw.text((x + 2, y + 2), f"t+{elapsed}", fill="black", font=font)

    draw_row(0, "REFERENCE", ref_paths, reference_range, ref_count)
    draw_row(1, "CURRENT", cur_paths, current_range, cur_count)

    chart_top = label_height + 2 * (label_height + cell_height)
    draw.rectangle((0, chart_top, width, height), fill="white")
    ref_y = cumulative_motion(reference, "dy")
    cur_y = cumulative_motion(current, "dy")
    all_values = ref_y + cur_y
    min_value, max_value = min(all_values), max(all_values)
    if min_value == max_value:
        min_value -= 1
        max_value += 1

    plot_left, plot_right = 44, width - 12
    plot_top, plot_bottom = chart_top + 35, height - 28
    draw.line((plot_left, plot_top, plot_left, plot_bottom), fill="#666666", width=1)
    draw.line((plot_left, plot_bottom, plot_right, plot_bottom), fill="#666666", width=1)

    def point(elapsed: int, value: int) -> tuple[int, int]:
        px = plot_left + round(elapsed * (plot_right - plot_left) / max(1, max_count - 1))
        py = plot_top + round(
            (max_value - value) * (plot_bottom - plot_top) / (max_value - min_value)
        )
        return px, py

    def draw_series(values: list[int], color: str) -> None:
        points = [point(elapsed, value) for elapsed, value in enumerate(values)]
        if len(points) >= 2:
            draw.line(points, fill=color, width=3)
        for px, py in points:
            draw.ellipse((px - 2, py - 2, px + 2, py + 2), fill=color)

    draw_series(ref_y, "#1261a0")
    draw_series(cur_y, "#c62828")
    draw.text((4, chart_top + 5), "Cumulative background Y translation (px)", fill="black", font=font)
    draw.text((width // 2 - 38, height - 18), "elapsed frames", fill="black", font=font)
    draw.text((width - 230, chart_top + 5), "REFERENCE", fill="#1261a0", font=font)
    draw.text((width - 145, chart_top + 5), "CURRENT", fill="#c62828", font=font)
    draw.text(
        (4, chart_top + 20),
        f"verdict={comparison['verdict']}  ref={ref_count} frames  current={cur_count} frames",
        fill="black",
        font=font,
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, required=True)
    parser.add_argument("--current-dir", type=Path, required=True)
    parser.add_argument("--reference-range", type=parse_range, required=True)
    parser.add_argument("--current-range", type=parse_range, required=True)
    parser.add_argument("--roi", type=parse_roi, required=True)
    parser.add_argument("--max-dx", type=int, default=4)
    parser.add_argument("--max-dy", type=int, default=4)
    parser.add_argument("--min-translation-improvement", type=float, default=1.0)
    parser.add_argument("--max-translation-residual", type=float, default=8.0)
    parser.add_argument("--duration-tolerance", type=float, default=0.05)
    parser.add_argument("--path-total-tolerance", type=int, default=2)
    parser.add_argument("--max-step-delta", type=int, default=1)
    parser.add_argument("--smoothness-tolerance", type=float, default=0.10)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--diagnostic-image", type=Path)
    parser.add_argument("--samples", type=int, default=8)
    parser.add_argument("--strict", action="store_true", help="exit 1 when a check fails")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.max_dx < 0 or args.max_dy < 0:
        raise SystemExit("--max-dx and --max-dy must be non-negative")

    reference = analyze_sequence(
        "reference",
        args.reference_dir,
        args.reference_range,
        args.roi,
        args.max_dx,
        args.max_dy,
        args.min_translation_improvement,
        args.max_translation_residual,
    )
    current = analyze_sequence(
        "current",
        args.current_dir,
        args.current_range,
        args.roi,
        args.max_dx,
        args.max_dy,
        args.min_translation_improvement,
        args.max_translation_residual,
    )
    comparison = compare(
        reference,
        current,
        args.duration_tolerance,
        args.path_total_tolerance,
        args.max_step_delta,
        args.smoothness_tolerance,
    )
    payload = {
        "schema_version": 1,
        "roi": {"x": args.roi[0], "y": args.roi[1], "width": args.roi[2], "height": args.roi[3]},
        "reference": reference,
        "current": current,
        "comparison": comparison,
    }

    rendered = json.dumps(payload, ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered + "\n", encoding="utf-8")
    if args.diagnostic_image:
        render_diagnostic(
            args.diagnostic_image,
            args.reference_dir,
            args.current_dir,
            args.reference_range,
            args.current_range,
            reference,
            current,
            comparison,
            args.samples,
        )

    print(f"verdict: {comparison['verdict']}")
    for check in comparison["checks"]:
        print(f"{check['status']:4} {check['name']}: {check['detail']}")
    if args.output:
        print(f"json: {args.output}")
    if args.diagnostic_image:
        print(f"diagnostic image: {args.diagnostic_image}")

    return 1 if args.strict and comparison["verdict"] == "FAIL" else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
