#!/usr/bin/env python3
"""Replace an exported mobile host's launcher icon with the game's own mark.

The engine export writes a neutral launcher icon; this script swaps in the
Poké Ball mark used by the web favicon and the iOS app icon, on the mobile
backdrop the host templates paint. Footprints match the ones the engine
template draws, so the mark sits exactly where the neutral one did:

- Android: legacy tiles keep the template's alpha silhouette with the mark at
  0.68 of the tile, and the adaptive foreground stays inside the 66/108dp
  safe zone.
- HarmonyOS: the layered-icon foreground keeps the engine mark's 0.625
  footprint, and the start-window tile keeps its silhouette.

scripts/build-android.py and scripts/build-harmony.py call this after the
export; it also runs on an already-exported host:

    python scripts/apply-mobile-icon.py --platform android --host dist/android

Requires Pillow: python3 -m pip install pillow
"""
import argparse
from pathlib import Path
import sys

try:
    from PIL import Image, ImageDraw
except ImportError:
    sys.exit('Pillow is required: python3 -m pip install pillow')

# Mobile backdrop of the engine host templates.
BACKDROP = (23, 32, 51, 255)

# Footprints measured from the engine templates' own launcher icons.
TILE_RATIO = 0.680
ADAPTIVE_SAFE_ZONE = 66 / 108
HARMONY_FOREGROUND_RATIO = 0.625

# favicon.svg geometry (viewBox 64x64): stroke rings sit on the radius, so the
# outer shell is drawn at r=32 with a 4-wide ring and the red half covers the
# top of that ring, exactly as the SVG's clip path does.
VIEWBOX = 64
BALL_RED = (229, 57, 53, 255)
BALL_WHITE = (248, 248, 248, 255)
BALL_BLACK = (17, 17, 17, 255)
SUPERSAMPLE = 4

ANDROID_DENSITIES = ('mdpi', 'hdpi', 'xhdpi', 'xxhdpi', 'xxxhdpi')


def render_mark(size: int) -> Image.Image:
    """Draw the Poké Ball mark on a transparent `size` x `size` canvas."""
    pixels = size * SUPERSAMPLE
    scale = pixels / VIEWBOX
    canvas = Image.new('RGBA', (pixels, pixels), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)

    def shell(outer_radius: float, fill, outline=None, width: float = 0.0) -> None:
        box = tuple(
            (VIEWBOX / 2 + sign * outer_radius) * scale
            for sign in (-1, -1, 1, 1)
        )
        draw.ellipse(box, fill=fill, outline=outline, width=round(width * scale))

    shell(32, BALL_WHITE, BALL_BLACK, 4)
    draw.pieslice((2 * scale, 2 * scale, 62 * scale, 62 * scale), 180, 360, fill=BALL_RED)
    draw.rectangle((2 * scale, 29 * scale, 62 * scale, 35 * scale), fill=BALL_BLACK)
    shell(12, BALL_WHITE, BALL_BLACK, 4)
    shell(4, BALL_BLACK)
    return canvas.resize((size, size), Image.LANCZOS)


def paste_mark(base: Image.Image, ratio: float) -> Image.Image:
    mark = render_mark(max(1, round(base.width * ratio)))
    out = base.copy()
    out.alpha_composite(mark, ((base.width - mark.width) // 2, (base.height - mark.height) // 2))
    return out


def tile(silhouette: Image.Image, ratio: float) -> Image.Image:
    """Backdrop tile carrying the mark, shaped like the template's own tile."""
    base = Image.new('RGBA', silhouette.size, BACKDROP)
    base.putalpha(silhouette.convert('RGBA').getchannel('A'))
    return paste_mark(base, ratio)


def transparent(size: tuple[int, int], ratio: float) -> Image.Image:
    return paste_mark(Image.new('RGBA', size, (0, 0, 0, 0)), ratio)


def apply_android(host: Path) -> list[Path]:
    res = host / 'app/src/main/res'
    if not res.is_dir():
        raise FileNotFoundError(str(res))
    written = []
    for density in ANDROID_DENSITIES:
        icons = res / f'mipmap-{density}'
        for name in ('ic_launcher.png', 'ic_launcher_round.png'):
            path = icons / name
            with Image.open(path) as current:
                tile(current, TILE_RATIO).save(path)
            written.append(path)
        foreground = icons / 'ic_launcher_foreground.png'
        with Image.open(foreground) as current:
            transparent(current.size, ADAPTIVE_SAFE_ZONE).save(foreground)
        written.append(foreground)
    return written


def apply_harmony(host: Path) -> list[Path]:
    written = []
    for relative in (
        'AppScope/resources/base/media/foreground.png',
        'entry/src/main/resources/base/media/foreground.png',
    ):
        path = host / relative
        with Image.open(path) as current:
            transparent(current.size, HARMONY_FOREGROUND_RATIO).save(path)
        written.append(path)
    start_icon = host / 'entry/src/main/resources/base/media/startIcon.png'
    with Image.open(start_icon) as current:
        tile(current, TILE_RATIO).save(start_icon)
    written.append(start_icon)
    return written


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument('--platform', choices=('android', 'harmony'), required=True)
    parser.add_argument('--host', type=Path, required=True,
                        help='exported host directory (dist/android, dist/harmony, ...)')
    args = parser.parse_args()

    host = args.host.resolve()
    apply = apply_android if args.platform == 'android' else apply_harmony
    try:
        written = apply(host)
    except FileNotFoundError as exc:
        parser.error(f'{exc.filename} not found: export the {args.platform} host first')
    print(f'Applied the game launcher icon to {len(written)} {args.platform} resources in {host}')


if __name__ == '__main__':
    main()
