#!/usr/bin/env python3
"""Render two animation windows at identical elapsed-frame offsets."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError as exc:  # pragma: no cover - depends on caller environment
    raise SystemExit("Pillow is required to render a raw-time contact sheet") from exc


FRAME_NUMBER = re.compile(r"(\d+)(?=\.png$)")


def parse_range(value: str) -> tuple[int, int]:
    try:
        start, end = (int(part) for part in value.split(":", 1))
    except (TypeError, ValueError) as exc:
        raise argparse.ArgumentTypeError("range must be START:END (inclusive)") from exc
    if start < 0 or end < start:
        raise argparse.ArgumentTypeError("range must satisfy 0 <= START <= END")
    return start, end


def parse_offsets(value: str) -> list[int]:
    try:
        offsets = [int(part) for part in value.split(",")]
    except ValueError as exc:
        raise argparse.ArgumentTypeError("offsets must be comma-separated integers") from exc
    if not offsets or any(offset < 0 for offset in offsets):
        raise argparse.ArgumentTypeError("offsets must be non-negative")
    if offsets != sorted(set(offsets)):
        raise argparse.ArgumentTypeError("offsets must be unique and ascending")
    return offsets


def indexed_frames(directory: Path) -> dict[int, Path]:
    frames = {}
    for path in directory.glob("*.png"):
        match = FRAME_NUMBER.search(path.name)
        if match:
            frames[int(match.group(1))] = path
    return frames


def load_frame(
    frames: dict[int, Path], number: int, expected_size: tuple[int, int]
) -> Image.Image:
    path = frames.get(number)
    if path is None:
        raise ValueError(f"missing frame {number}")
    with Image.open(path) as source:
        image = source.convert("RGB")
    if image.size != expected_size:
        raise ValueError(f"frame {number} is {image.size}, expected {expected_size}")
    return image


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render reference/current frames at the same raw t+N offsets."
    )
    parser.add_argument("--reference-dir", required=True, type=Path)
    parser.add_argument("--current-dir", required=True, type=Path)
    parser.add_argument("--reference-range", required=True, type=parse_range)
    parser.add_argument("--current-range", required=True, type=parse_range)
    parser.add_argument("--offsets", required=True, type=parse_offsets)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--title", default="RAW-TIME ALIGNMENT (no temporal resampling)")
    args = parser.parse_args()

    ref_frames = indexed_frames(args.reference_dir)
    cur_frames = indexed_frames(args.current_dir)
    first_path = ref_frames.get(args.reference_range[0])
    if first_path is None:
        raise ValueError(f"missing reference start frame {args.reference_range[0]}")
    with Image.open(first_path) as first:
        frame_size = first.size

    frame_width, frame_height = frame_size
    label_height = 26
    title_height = 18
    row_height = label_height + frame_height
    sheet = Image.new(
        "RGB",
        (frame_width * len(args.offsets), title_height + row_height * 2),
        "white",
    )
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    draw.text((3, 3), args.title, fill="black", font=font)

    def draw_row(
        y: int,
        label: str,
        frames: dict[int, Path],
        frame_range: tuple[int, int],
    ) -> None:
        start, end = frame_range
        draw.text((3, y), label, fill="black", font=font)
        for column, offset in enumerate(args.offsets):
            x = column * frame_width
            draw.text((x + 3, y + 12), f"t+{offset}", fill="black", font=font)
            number = start + offset
            frame_y = y + label_height
            if number > end:
                draw.rectangle(
                    (x, frame_y, x + frame_width - 1, frame_y + frame_height - 1),
                    fill=(144, 144, 144),
                )
                draw.text(
                    (x + frame_width // 2 - 17, frame_y + frame_height // 2 - 4),
                    "ENDED",
                    fill="white",
                    font=font,
                )
            else:
                sheet.paste(load_frame(frames, number, frame_size), (x, frame_y))

    draw_row(title_height, "REFERENCE", ref_frames, args.reference_range)
    draw_row(title_height + row_height, "CURRENT", cur_frames, args.current_range)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(args.output)
    print(args.output)


if __name__ == "__main__":
    main()
