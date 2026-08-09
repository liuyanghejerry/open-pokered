#!/usr/bin/env python3
"""Re-flow the line breaks in every DSL dialogue (`@t("en", "zh")`) to match
the Fusion Pixel 10px font's line capacity.

The dialogue data was authored for the original 8px GB font (18 Latin / 13
CJK characters per line). The renderer now wraps by pixel width — an 18-tile
box interior is 144px, i.e. ~28 Latin (5px) or 14 CJK (10px) characters per
line — and treats a single `\n` as a soft break (short lines merge). This
script re-flows the authored `\n` positions so the data itself matches the
new capacity: EN word-wraps at 28 chars, ZH char-wraps at 14 full-width
units with kinsoku rules (ASCII words and <PLACEHOLDERS> stay intact,
closing punctuation never opens a line — it pulls one unit down to the next
line so character order is preserved, consecutive closing runs like ……
never split, opening brackets never close a line), mirroring
`dotzuki-ui/src/widgets/dialog.rs::wrap_lines`.

Invariant: the re-flow only *moves* `\n` — the character sequence of every
`@t` argument is preserved modulo whitespace (a space at a wrap point
becomes the `\n` and vice versa; a ZH `\n` between two CJK characters
vanishes on merge, next to Latin it becomes a space). `\n\n` paragraphs are
kept; the pass is idempotent. `scripts/verify_scene_translations.py`
enforces the invariant.

Usage:
  python3 scripts/reflow_scene_dialogue.py [--dry-run] [file_or_dir ...]

Defaults to every maps/*/script.scene. With --dry-run only reports what
would change.
"""
import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MAPS = ROOT / "examples" / "pokered" / "crates" / "pokered-data" / "maps"

# Match a localized literal; captures the raw (escaped) EN and ZH strings.
T_RE = re.compile(r'@t\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)')

# `\n` is the only escape used in the scene dialogue strings (verified), so
# escaping is a simple two-way swap.
ESC_NL = "\\n"
REAL_NL = "\n"

EN_WIDTH = 28          # Latin chars per line (28 × 5px = 140px ≤ 144px)
ZH_CAP = 14.0          # full-width units per line (14 × 10px = 140px ≤ 144px)


def unescape_nl(s: str) -> str:
    return s.replace(ESC_NL, REAL_NL)


def reescape_nl(s: str) -> str:
    return s.replace(REAL_NL, ESC_NL)


# ── English: greedy word wrap at `width` chars ─────────────────────
# Tokens are split on single spaces (empty tokens keep double spaces and
# leading/trailing padding byte-identical). A line that fits is rejoined to
# the exact original; at a wrap point the inter-word space becomes `\n`.

def wrap_en(paragraph: str, width: int) -> str:
    tokens = paragraph.split(" ")
    lines = []
    cur = []
    cur_chars = 0
    for t in tokens:
        t_len = len(t)
        if not cur:
            cur.append(t)
            cur_chars = t_len
            continue
        # Appending t adds one separator plus the token.
        if cur_chars + t_len + len(cur) <= width:
            cur.append(t)
            cur_chars += t_len
        else:
            lines.append(" ".join(cur))
            cur = [t]
            cur_chars = t_len
    if cur:
        lines.append(" ".join(cur))
    return "\n".join(lines)


# ── CJK: greedy char wrap at `cap` full-width units ────────────────
# ASCII is half-width (0.5 unit); ASCII alnum runs are unbreakable.

def zh_units(ch: str) -> float:
    return 0.5 if ord(ch) < 0x80 else 1.0


CLOSING = set("，。！？；：、」』）】〉》”’…—～")
OPENING = set("「『（【〈《“‘")


def zh_tokens(text: str):
    tokens = []
    word = ""
    i = 0
    chars = list(text)
    while i < len(chars):
        ch = chars[i]
        i += 1
        if ch == " ":
            if word:
                tokens.append(("word", word))
                word = ""
            tokens.append(("ch", " "))
        elif ch == "<":
            # <PLAYER> / <RIVAL> / … placeholders are atomic: a line break
            # inside one would defeat the literal `replace("<PLAYER>", name)`
            # substitution at display time.
            if word:
                tokens.append(("word", word))
                word = ""
            ph = ch
            while i < len(chars) and chars[i - 1] != ">":
                ph += chars[i]
                i += 1
            tokens.append(("word", ph))
        elif ch.isascii() and ch.isalnum():
            word += ch
        else:
            # The pending ASCII word must be flushed BEFORE a closing run can
            # glue onto the previous token — otherwise the word is appended
            # after the run, reordering the text ("：Pi…！" → "：！Pi…").
            if word:
                tokens.append(("word", word))
                word = ""
            if (
                ch in CLOSING
                and tokens
                and tokens[-1][0] == "ch"
                and tokens[-1][1] != " "
                and all(c in CLOSING for c in tokens[-1][1])
            ):
                # Consecutive closing punctuation (……, ！！) is one
                # unbreakable run — never split across lines (禁则处理).
                tokens[-1] = ("ch", tokens[-1][1] + ch)
            else:
                tokens.append(("ch", ch))
    if word:
        tokens.append(("word", word))
    return tokens


def tok_width(tok) -> float:
    if tok[0] == "word":
        return len(tok[1]) * 0.5
    return sum(zh_units(c) for c in tok[1])


def is_closing_tok(tok) -> bool:
    return tok[0] == "ch" and all(c in CLOSING for c in tok[1])


def wrap_zh(paragraph: str, cap: float) -> str:
    toks = zh_tokens(paragraph)
    lines = []
    line = []
    line_w = 0.0

    def flush():
        nonlocal line, line_w
        while line and line[0] == ("ch", " "):
            line.pop(0)
        while line and line[-1] == ("ch", " "):
            line.pop()
        if line:
            lines.append("".join(t for _, t in line))
        line = []
        line_w = 0.0

    i = 0
    while i < len(toks):
        tok = toks[i]
        w = tok_width(tok)
        if not line:
            if tok == ("ch", " "):
                i += 1
                continue
            line.append(tok)
            line_w = w
            i += 1
            continue
        if line_w + w <= cap:
            line.append(tok)
            line_w += w
            i += 1
            continue

        # ── overflow: kinsoku (禁则处理) ──
        # Closing punctuation must not open a line: pull the last unit of the
        # current line down so the run follows it on the next line (追い込み)
        # — character order is preserved. When the line has only one unit to
        # give, the run overhangs the line end (ぶら下げ) rather than
        # dangling at the start of the next one.
        if is_closing_tok(tok):
            # Drop trailing spaces — they are trimmed at display anyway.
            while line and line[-1] == ("ch", " "):
                u = line.pop()
                line_w -= tok_width(u)
            if len(line) >= 2:
                u = line.pop()
                line_w -= tok_width(u)
                flush()
                line.append(u)
                line_w = tok_width(u)
                line.append(tok)
                line_w += w
            else:
                line.append(tok)
                line_w += w
            i += 1
            continue
        # Opening brackets must not end a line: the bracket rolls onto the
        # next line where it binds to the overflowing unit that follows.
        carried = None
        if line and line[-1][0] == "ch" and line[-1][1] in OPENING:
            carried = line.pop()
        flush()
        if carried:
            line.append(carried)
            line_w = tok_width(carried)
        # The overflowing unit starts the fresh line (a too-wide ASCII word
        # stays whole; the renderer hard-splits it at display time).
        if tok == ("ch", " "):
            i += 1
            continue
        line.append(tok)
        line_w += w
        i += 1

    if line:
        flush()
    return "\n".join(lines)


def merge_zh(p: str) -> str:
    """Merge soft `\\n` breaks in ZH text: a break between two CJK characters
    vanishes (CJK needs no inter-word space); next to Latin it stands for a
    space — otherwise "TM23 3300\\nTM15" would fuse into "3300TM15", and a
    space trimmed at an earlier pass's wrap point would be lost for good.
    Mirrors `cjk_units` in `dotzuki-ui/src/widgets/dialog.rs`."""
    out = []
    chars = list(p)
    for idx, ch in enumerate(chars):
        if ch != REAL_NL:
            out.append(ch)
            continue
        prev = chars[idx - 1] if idx > 0 else ""
        nxt = chars[idx + 1] if idx + 1 < len(chars) else ""
        if not prev or not nxt or prev == " " or nxt == " ":
            continue  # a space (or the paragraph edge) already separates
        if ord(prev) >= 0x80 and ord(nxt) >= 0x80:
            continue  # between CJK: disappears
        out.append(" ")
    return "".join(out)


def reflow(escaped: str, lang: str) -> str:
    """Re-flow one escaped @t argument; returns the new escaped form."""
    if ESC_NL not in escaped:
        return escaped
    text = unescape_nl(escaped)
    paragraphs = text.split(REAL_NL * 2)
    out = []
    for p in paragraphs:
        # Single `\n` is a soft break: EN keeps a space between the merged
        # words, ZH packs the characters (CJK needs no inter-word space).
        soft = p.replace(REAL_NL, " ") if lang == "en" else merge_zh(p)
        wrapped = wrap_en(soft, EN_WIDTH) if lang == "en" else wrap_zh(soft, ZH_CAP)
        out.append(reescape_nl(wrapped))
    return (ESC_NL * 2).join(out)


def reflow_file(path: pathlib.Path) -> tuple:
    """Rewrites @t literals in one scene file; returns (changed, total)."""
    src = path.read_text(encoding="utf-8")
    total = len(T_RE.findall(src))

    def sub(m: re.Match) -> str:
        en, zh = m.group(1), m.group(2)
        return f'@t("{reflow(en, "en")}", "{reflow(zh, "zh")}")'

    # Re-flow until stable: a ZH break replaces an inter-word space with `\n`,
    # which the next pass drops on merge, so a second pass can shift a break.
    for _ in range(4):
        new_src = T_RE.subn(sub, src)[0]
        if new_src == src:
            break
        src = new_src

    if src != path.read_text(encoding="utf-8"):
        path.write_text(src, encoding="utf-8")
        return True, total
    return False, total


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dry-run", action="store_true", help="report without writing")
    ap.add_argument("targets", nargs="*", help="files or dirs (default: all maps/*/script.scene)")
    args = ap.parse_args()

    targets = args.targets or [str(MAPS)]
    paths = []
    for t in targets:
        p = pathlib.Path(t)
        if p.is_dir():
            direct = p / "script.scene"
            if direct.exists():
                paths.append(direct)
            else:
                paths.extend(sorted(p.glob("*/script.scene")))
        elif p.is_file():
            paths.append(p)
        else:
            print(f"skip: no such path {t}", file=sys.stderr)

    changed = 0
    total_lits = 0
    for p in paths:
        if args.dry_run:
            src = p.read_text(encoding="utf-8")
            before = src
            new_src = T_RE.subn(
                lambda m: f'@t("{reflow(m.group(1), "en")}", "{reflow(m.group(2), "zh")}")',
                src,
            )[0]
            if new_src != before:
                changed += 1
            total_lits += len(T_RE.findall(before))
        else:
            c, n = reflow_file(p)
            changed += c
            total_lits += n

    print(f"{'would change' if args.dry_run else 'changed'}: {changed} / {len(paths)} file(s) "
          f"({total_lits} @t literal(s) examined)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
