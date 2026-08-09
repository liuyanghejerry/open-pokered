#!/usr/bin/env python3
"""Verify scene translations preserve the English text word-for-word.

For every @say/@speaker block in a translated .scene file, the English
text (the first arg of each @t(...)) must equal the original English text
(the plain string literals joined with \n, plus the "NAME: " prefix that
the DSL codegen added for a non-empty speaker name).

Whitespace (spaces and line breaks) is ignored: since the Fusion Pixel
font changed the line capacity, `scripts/reflow_scene_dialogue.py`
deliberately re-flows the `\n` breaks (a space at a wrap point becomes the
newline and vice versa), so the check guards word-level fidelity, not
formatting.

When the git baseline is already @t-form, the Chinese text (the second
arg) is also checked the same whitespace-insensitive way: a re-flow may
only move line breaks, never reorder or drop characters.

Usage:
  python3 scripts/verify_scene_translations.py [map_dir] [map_dir ...]

Defaults to checking every maps/*/script.scene against git HEAD.
Exit 0 = all EN/ZH text preserved; 1 = mismatches found.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAPS = ROOT / "examples" / "pokered" / "crates" / "pokered-data" / "maps"

# Match a @say("Name") { ... } or @speaker("Name") { ... } block.
BLOCK_RE = re.compile(
    r'@(?:say|speaker)\("([^"]*)"\)\s*\{([^}]*)\}', re.DOTALL
)
# Match a plain string literal inside a block (the original form).
PLAIN_STR_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')
# Match an @t("en", "zh") localized literal.
T_RE = re.compile(r'@t\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)')


def unescape(s: str) -> str:
    """Decode the DSL string escapes (\\n, \\t, \\r, \\", \\\\)."""
    return (
        s.replace(r'\"', '"')
        .replace(r"\\", "\x00")
        .replace(r"\n", "\n")
        .replace(r"\t", "\t")
        .replace(r"\r", "\r")
        .replace("\x00", "\\")
    )


def normalized(text: str) -> str:
    """Word-level fidelity key: all whitespace (incl. re-flowed line breaks)
    removed."""
    return re.sub(r"\s+", "", text)


def extract_blocks(src: str):
    return BLOCK_RE.findall(src)


def original_en(blocks):
    """Baseline EN body: the EN literal of each @t(...) when the baseline is
    @t-form (translated), else the plain string literals (pre-translation),
    both prefixed with the speaker name when present."""
    out = []
    for name, body in blocks:
        ts = T_RE.findall(body)
        if ts:
            en = "\n".join(unescape(e) for e, _z in ts)
        else:
            lines = PLAIN_STR_RE.findall(body)
            en = "\n".join(unescape(l) for l in lines)
        out.append(f"{name}: {en}" if name else en)
    return out


def translated_en(blocks):
    """Translated EN body: first args of @t(...) joined by \n."""
    out = []
    for _name, body in blocks:
        ts = T_RE.findall(body)
        en = "\n".join(unescape(e) for e, _z in ts)
        out.append(en)
    return out


def baseline_zh(blocks):
    """Baseline ZH body per block, or None when the baseline is not @t-form
    (pre-translation). Used to guard the ZH text against reflow accidents:
    only whitespace may change, never the character sequence."""
    out = []
    for _name, body in blocks:
        ts = T_RE.findall(body)
        out.append("\n".join(unescape(z) for _e, z in ts) if ts else None)
    return out


def translated_zh(blocks):
    out = []
    for _name, body in blocks:
        ts = T_RE.findall(body)
        out.append("\n".join(unescape(z) for _e, z in ts) if ts else None)
    return out


def check_file(path: Path) -> list:
    new_src = path.read_text(encoding="utf-8")
    rel = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)
    # Baseline = the pre-translation upstream text. `HEAD` is the (already
    # committed) translated version on this branch, so fall back to
    # origin/master when the branch is ahead of it.
    for ref in ("origin/master", "HEAD"):
        old_src = subprocess.run(
            ["git", "show", f"{ref}:{rel}"],
            capture_output=True, text=True, cwd=ROOT,
        ).stdout
        if old_src:
            break
    if not old_src:
        return [f"{path}: no git baseline (new file?)"]
    problems = []
    new_blocks = extract_blocks(new_src)
    old_blocks = extract_blocks(old_src)
    if len(new_blocks) != len(old_blocks):
        problems.append(
            f"{path}: block count changed {len(old_blocks)} -> {len(new_blocks)}"
        )
    for i, (old_en, new_en) in enumerate(zip(original_en(old_blocks), translated_en(new_blocks))):
        if normalized(old_en) != normalized(new_en):
            problems.append(
                f"{path}: block {i} EN mismatch\n"
                f"  HEAD: {old_en!r}\n"
                f"  NOW : {new_en!r}"
            )
    # ZH conservation: when the baseline is already @t-form, the re-flow may
    # only move whitespace — the character sequence must survive intact
    # (guards against kinsoku/merge bugs reordering or dropping characters).
    for i, (old_zh, new_zh) in enumerate(zip(baseline_zh(old_blocks), translated_zh(new_blocks))):
        if old_zh is not None and new_zh is not None and normalized(old_zh) != normalized(new_zh):
            problems.append(
                f"{path}: block {i} ZH character sequence changed\n"
                f"  HEAD: {old_zh!r}\n"
                f"  NOW : {new_zh!r}"
            )
    # Every block must be fully localized (no leftover plain strings) —
    # except blocks whose body contains no text at all.
    for i, (name, body) in enumerate(new_blocks):
        # Remove @t(...) constructs first, then any remaining "..." is a
        # leftover monolingual string literal.
        stripped = T_RE.sub("", body)
        plain = PLAIN_STR_RE.findall(stripped)
        if plain:
            problems.append(f"{path}: block {i} still has plain string(s): {plain}")
    return problems


def main():
    targets = sys.argv[1:] or [str(MAPS)]
    paths = []
    for t in targets:
        p = Path(t)
        if p.is_dir():
            direct = p / "script.scene"
            if direct.exists():
                paths.append(direct)
            else:
                paths.extend(sorted(p.glob("*/script.scene")))
        elif p.is_file():
            paths.append(p)
    all_problems = []
    for p in paths:
        all_problems.extend(check_file(p))
    if all_problems:
        print(f"FAIL: {len(all_problems)} problem(s)")
        for pr in all_problems:
            print(pr)
        sys.exit(1)
    print(f"OK: {len(paths)} scene(s) preserve English text word-for-word and Chinese character sequences")
    sys.exit(0)


if __name__ == "__main__":
    main()
