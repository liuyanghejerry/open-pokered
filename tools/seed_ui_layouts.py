#!/usr/bin/env python3
"""
Seed UI layout JSON files by extracting layout data from Rust menu source files.

Stage 1.1 of the UI Layout Editor plan. Parses every
crates/pokered-ui/src/menus/*.rs (excluding mod.rs and mart.rs), extracts
layout constants/coordinates from draw_* functions, and emits one JSON file
per screen module into crates/pokered-data/ui_layouts/.

NOTE: This is a one-shot bootstrap tool. After Stage 1.1, JSON files become
the source of truth. The script is NOT part of the build.

Expectations (per plan):
  - ~50-60% of variants extract cleanly (simple menus like start, save)
  - ~30-40% extract with _TODO markers needing manual fill-in
  - ~10-20% require full hand-authoring (naming.rs, stats.rs, party.rs)
"""

import re
import json
import sys
from pathlib import Path
from typing import Optional

# ── Paths ──────────────────────────────────────────────────────────
MENUS_DIR = Path("crates/pokered-ui/src/menus")
OUT_DIR = Path("crates/pokered-data/ui_layouts")
SKIP_FILES = {"mod.rs", "mart.rs"}

# ── Regex patterns ─────────────────────────────────────────────────
# TileRect literal
RE_TILE_RECT = re.compile(
    r'TileRect::new\(\s*(-?\d+)\s*,\s*(-?\d+)\s*,\s*(-?\d+)\s*,\s*(-?\d+)\s*\)'
)

# Variable assignment of a TileRect
RE_RECT_VAR = re.compile(
    r'let\s+(\w+)\s*[:=]\s*TileRect::new\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)'
)

# Static-height box variable: let th = <number>; ... TileRect::new(..,th)
RE_STATIC_TH_VAR = re.compile(
    r'let\s+(\w+)\s*=\s*(\d+)\s*(?:_u32)?\s*;'
)

# ui.text_box(TileRect::new(...), InkColor::C, |frame| {  -- or with variable
RE_TEXT_BOX_LITERAL = re.compile(
    r'ui\.text_box\(\s*TileRect::new\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*,\s*InkColor::(\w+)\s*,'
)
RE_TEXT_BOX_VAR = re.compile(
    r'ui\.text_box\(\s*(\w+)\s*,\s*InkColor::(\w+)\s*,'
)

# ui.region(TileRect::new(...), ...)
RE_REGION_LITERAL = re.compile(
    r'ui\.region\(\s*TileRect::new\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*,'
)
RE_REGION_VAR = re.compile(
    r'ui\.region\(\s*(\w+)\s*,'
)

# Static label: frame.label(tx, ty, "TEXT", InkColor::C)
RE_LABEL_STATIC = re.compile(
    r'frame\.label\(\s*(\d+)\s*,\s*(\d+)\s*,\s*"((?:[^"\\]|\\.)*)"\s*,\s*InkColor::(\w+)\s*\)'
)

# Label with variable/expression (e.g., &format!("..."), &var, var.as_str())
RE_LABEL_DYNAMIC = re.compile(
    r'frame\.label\(\s*(\d+)\s*,\s*(\d+)\s*,\s*[&]?(?!")([\w.:<>()&!+\-*/ ]+)\s*,\s*InkColor::(\w+)\s*\)'
)

# frame.cursor_at(tx, cursor_var, color)
RE_CURSOR_AT = re.compile(
    r'frame\.cursor_at\(\s*(\d+)\s*,\s*(\w+)\s*,\s*InkColor::(\w+)\s*\)'
)

# frame.cursor_glyph_at(tx, cursor_var, 'glyph', color) or (tx, ty, "glyph", color) or (tx, ty, STATIC_REF, color)
RE_CURSOR_GLYPH_AT = re.compile(
    r'frame\.cursor_glyph_at\(\s*(\d+)\s*,\s*(\w+)\s*,\s*'
    r'(?:[\'\"](\S+?)[\'\"]|(?:\'\\))?\s*,\s*InkColor::(\w+)\s*\)|'
    r'[\'\"]([^\'\"]+)[\'\"]\s*,\s*InkColor::(\w+)\s*\)'
)

# Cursor formula: let <var> = <base> + (<expr> as u32 * <step>);
RE_CURSOR_FORMULA = re.compile(
    r'let\s+(\w+)\s*=\s*(\d+)\s*\+\s*\(\s*([\w.()]+)\s*as\s*u32\s*\*\s*(\d+)\s*\)'
)
# Cursor formula: let <var> = <base> + <expr> as u32; (no step, step=1)
RE_CURSOR_SIMPLE = re.compile(
    r'let\s+(\w+)\s*=\s*(\d+)\s*\+\s*(\w+)\s*as\s*u32\s*;'
)
# Cursor formula: let <var> = <expr> as u32 * <step>; (no base, base=0)
RE_CURSOR_NO_BASE = re.compile(
    r'let\s+(\w+)\s*=\s*(\w+)\s*as\s*u32\s*\*\s*(\d+)\s*;'
)

# bracket_box
RE_BRACKET_BOX_LITERAL = re.compile(
    r'frame\.bracket_box\(\s*TileRect::new\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*,\s*BracketSides::(\w+)\s*,\s*(true|false)\s*,\s*InkColor::(\w+)\s*\)'
)
RE_BRACKET_BOX_VAR = re.compile(
    r'frame\.bracket_box\(\s*(\w+)\s*,\s*BracketSides::(\w+)\s*,\s*(true|false)\s*,\s*InkColor::(\w+)\s*\)'
)

# hp_bar
RE_HP_BAR = re.compile(
    r'frame\.hp_bar\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,'
)

# vline / hline / pixel_rect
RE_VLINE = re.compile(
    r'frame\.vline\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*InkColor::(\w+)\s*\)'
)
RE_HLINE = re.compile(
    r'frame\.hline\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*InkColor::(\w+)\s*\)'
)
RE_PIXEL_RECT = re.compile(
    r'frame\.pixel_rect\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*InkColor::(\w+)\s*\)'
)

# .take(N) or for i in 0..N
RE_TAKE_N = re.compile(r'\.take\((\d+)\)')
RE_FOR_RANGE = re.compile(r'for\s+\w+\s+in\s+0\.\.(\d+)')

# Dynamic height: let th = base + (len_expr * extra);
RE_DYNAMIC_HEIGHT = re.compile(
    r'let\s+(\w+)\s*=\s*(\d+)\s*\+\s*\(.*?\.len\(\).*?\*\s*(\d+)'
)
# Simpler: let <var> = <base> + (<expr> as u32 * extra);
RE_DYNAMIC_H_FORMULA = re.compile(
    r'let\s+(\w+)\s*=\s*(\d+)\s*\+\s*\(([^)]*as\s*u32\s*\*\s*(\d+)\s*)\)'
)

# match state.phase { or if condition {
RE_MATCH_PHASE = re.compile(r'match\s+(?:&?self\.|state\.)?(\w+)')
RE_IF_STATE = re.compile(r'if\s+(\w+\([^)]*\)|[\w.]+)')

# menu_list helper
RE_MENU_LIST = re.compile(
    r'frame\.menu_list\(\s*(\d+)\s*,\s*(\d+)\s*,\s*&\[([^\]]+)\]'
)

# label_value_grid
RE_LABEL_VALUE_GRID = re.compile(
    r'frame\.label_value_grid\(\s*(\d+)\s*,\s*(\d+)\s*,\s*&\[([^\]]+)\]'
)

# gb_tile
RE_GB_TILE = re.compile(
    r'frame\.gb_tile\(\s*(\d+)\s*,\s*(\d+)\s*,\s*([\w:]+)\s*,'
)

# const definitions (e.g., const NAME_BOX_TX: u32 = 10;)
RE_CONST_U32 = re.compile(
    r'const\s+(\w+)\s*:\s*u32\s*=\s*(\d+)\s*;'
)

# fn draw_*  →  function declaration
RE_FN_DRAW = re.compile(
    r'(?:pub(?:\s*\(\s*(?:crate|super)\s*\))?\s+)?fn\s+(draw(?:_\w+)?)\s*[<(]'
)

# enum→position mapping (e.g., TextSpeed::Fast => 0 / Medium => 6 / Slow => 13)
RE_MATCH_ENUM_TO_X = re.compile(
    r'(\w+)::(\w+)\s*=>\s*(\d+)'
)

# ── Helpers ────────────────────────────────────────────────────────

def unescape(s: str) -> str:
    """Handle Rust string escape sequences in extracted text."""
    s = re.sub(r'\\u\{([0-9A-Fa-f]+)\}', lambda m: chr(int(m.group(1), 16)), s)
    return s.replace('\\n', '\n').replace('\\t', '\t').replace('\\r', '\r')\
            .replace('\\\\', '\\').replace('\\"', '"').replace("\\'", "'")


def find_matching_brace(text: str, start: int) -> int:
    """Find the position of the matching close brace for the open brace at `start`.
    Returns -1 if not found."""
    if start >= len(text) or text[start] != '{':
        return -1
    depth = 1
    i = start + 1
    while i < len(text) and depth > 0:
        ch = text[i]
        if ch == '{':
            depth += 1
        elif ch == '}':
            depth -= 1
        elif ch == '"':
            # Skip strings
            i += 1
            while i < len(text) and text[i] != '"':
                if text[i] == '\\':
                    i += 1
                i += 1
        elif ch == '/' and i + 1 < len(text):
            # Skip comments
            if text[i + 1] == '/':
                while i < len(text) and text[i] != '\n':
                    i += 1
            elif text[i + 1] == '*':
                i += 2
                while i + 1 < len(text) and not (text[i] == '*' and text[i + 1] == '/'):
                    i += 1
                i += 1
        i += 1
    return i - 1 if depth == 0 else -1


def find_open_paren(text: str, pos: int) -> int:
    """Find the matching '(' before pos that opens a call."""
    depth = 0
    i = pos
    while i >= 0:
        ch = text[i]
        if ch == ')':
            depth += 1
        elif ch == '(':
            depth -= 1
            if depth < 0:
                return i
        elif ch == '"':
            i -= 1
            while i >= 0 and text[i] != '"':
                if i > 0 and text[i - 1] == '\\':
                    i -= 1
                i -= 1
        i -= 1
    return -1


def find_function_bodies(src: str) -> list[dict]:
    """Find all draw-related functions and return their metadata."""
    functions = []
    for m in RE_FN_DRAW.finditer(src):
        name = m.group(1)
        # Find opening paren '(', skipping past generics like <P: Painter>
        paren_pos = src.find('(', m.end() - 1)
        if paren_pos == -1:
            continue
        # Find matching ')' for parameter list
        depth = 1
        j = paren_pos + 1
        while j < len(src) and depth > 0:
            if src[j] == '(':
                depth += 1
            elif src[j] == ')':
                depth -= 1
            j += 1
        # Now find '{' for body
        while j < len(src) and src[j] != '{':
            if src[j] == ';':
                # This is a forward declaration/trait method, skip
                j = -1
                break
            j += 1
        if j == -1 or j >= len(src):
            continue
        body_start = j
        body_end = find_matching_brace(src, body_start)
        if body_end == -1:
            continue
        body = src[body_start:body_end + 1]
        functions.append({
            'name': name,
            'body': body,
            'start': m.start(),
            'body_start': body_start,
            'body_end': body_end,
        })
    return functions


def parse_consts(src: str) -> dict[str, int]:
    """Extract const NAME: u32 = VALUE; definitions."""
    consts = {}
    for m in RE_CONST_U32.finditer(src):
        consts[m.group(1)] = int(m.group(2))
    return consts


def resolve_var(val: str, consts: dict, local_vars: dict) -> Optional[int]:
    """Try to resolve a variable name or expression to an integer.
    Returns None if not resolvable."""
    val = val.strip()
    if val.isdigit():
        return int(val)
    if val in consts:
        return consts[val]
    if val in local_vars:
        return local_vars[val]
    # Try simple arithmetic like "NAME_BOX_TX + 1"
    parts = re.split(r'\s*([+\-*/])\s*', val)
    if len(parts) == 3 and parts[1] in ('+', '-', '*', '/'):
        left = resolve_var(parts[0], consts, local_vars)
        right = resolve_var(parts[2], consts, local_vars)
        if left is not None and right is not None:
            op = parts[1]
            if op == '+':
                return left + right
            elif op == '-':
                return left - right
            elif op == '*':
                return left * right
            elif op == '/':
                return left // right
    return None


def extract_dynamic_labels(body: str, consts: dict) -> dict:
    """Extract dynamic labels (positions for code-supplied text).
    Returns dict of label_id -> DynamicLabelDef."""
    result = {}
    idx = 0
    for m in RE_LABEL_STATIC.finditer(body):
        tid = f"label_{idx}"
        result[tid] = {
            "_TODO": "assign meaningful id - inspect source for what text this label holds",
            "tx": int(m.group(1)),
            "ty": int(m.group(2)),
            "text": unescape(m.group(3)),
            "color": m.group(4),
        }
        idx += 1
    return result


def parse_bracket_sides(sides_str: str) -> dict:
    """Parse BracketSides::ALL / LEFT_RIGHT / RIGHT_BOTTOM etc."""
    s = sides_str.upper()
    return {
        "top": "TOP" in s or "ALL" in s,
        "bottom": "BOTTOM" in s or "ALL" in s,
        "left": "LEFT" in s or "ALL" in s,
        "right": "RIGHT" in s or "ALL" in s,
    }


def extract_enum_positions(body: str) -> dict[str, int]:
    """Extract enum::variant => position mappings (e.g., TextSpeed::Fast => 0)."""
    result = {}
    for m in RE_MATCH_ENUM_TO_X.finditer(body):
        variant = m.group(2)
        pos = int(m.group(3))
        result[variant] = pos
    return result if result else None


def extract_cursor_formula(body: str, consts: dict) -> Optional[dict]:
    """Try to extract a cursor formula (base + cursor * step) from a function body."""
    candidates = []

    for m in RE_CURSOR_FORMULA.finditer(body):
        candidates.append({
            "base_ty": int(m.group(2)),
            "row_step": int(m.group(4)),
            "_cursor_var": m.group(1),
            "_cursor_expr": m.group(3),
            "score": 2 if 'cursor' in m.group(1).lower() or 'cursor' in m.group(3).lower() else 1,
        })
    for m in RE_CURSOR_SIMPLE.finditer(body):
        candidates.append({
            "base_ty": int(m.group(2)),
            "row_step": 1,
            "_cursor_var": m.group(1),
            "score": 2 if 'cursor' in m.group(1).lower() else 1,
        })
    for m in RE_CURSOR_NO_BASE.finditer(body):
        candidates.append({
            "base_ty": 0,
            "row_step": int(m.group(3)),
            "_cursor_var": m.group(1),
            "score": 2 if 'cursor' in m.group(1).lower() else 1,
        })

    if candidates:
        best = max(candidates, key=lambda c: c["score"])
        return best

    m = re.search(r'const\s+ROW_HEIGHT_TILES\s*:\s*u32\s*=\s*(\d+)\s*;', body)
    if m:
        return {
            "base_ty": 0,
            "row_step": int(m.group(1)),
            "_TODO": "verify cursor formula - using ROW_HEIGHT_TILES as row_step",
        }
    return None


def resolve_rect_in_body(body: str, varname: str, local_vars: dict, consts: dict) -> Optional[dict]:
    """Try to find a let <varname> = TileRect::new(...) assignment inside the body."""
    for m in RE_RECT_VAR.finditer(body):
        if m.group(1) == varname:
            return {
                "tx": int(m.group(2)),
                "ty": int(m.group(3)),
                "tw": int(m.group(4)),
                "th": int(m.group(5)),
            }
    # Look for const definitions
    for suffix in ['_TX', '_TY', '_TW', '_TH']:
        pass  # Not a clean pattern for rect variables from consts
    return None


def extract_rect_from_expr(expr: str, local_vars: dict, consts: dict) -> Optional[dict]:
    """Try to extract a TileRect from an expression that may be a literal,
    a variable, or a TileRect::new() call."""
    # Direct TileRect::new(...)
    m = RE_TILE_RECT.search(expr)
    if m:
        return {
            "tx": int(m.group(1)),
            "ty": int(m.group(2)),
            "tw": int(m.group(3)),
            "th": int(m.group(4)),
        }
    # Variable name
    if expr.strip() in local_vars:
        return local_vars[expr.strip()]
    return None


def find_closure_body(text: str, pos: int) -> Optional[str]:
    """Given the position right after '|frame| {' or similar, find the full closure body
    by matching braces."""
    brace_pos = text.find('{', pos)
    if brace_pos == -1:
        return None
    end = find_matching_brace(text, brace_pos)
    if end == -1:
        return None
    return text[brace_pos:end + 1]


def extract_list_params(body: str) -> Optional[dict]:
    """Try to extract list parameters: item_start_ty, row_step, max_visible_rows."""
    result = {}
    # Look for row = BASE + (i as u32 * STEP) pattern
    m = re.search(r'row\s*=\s*(\d+)\s*\+\s*\(.*?as\s*u32\s*\*\s*(\d+)\s*\)', body)
    if m:
        result["item_start_ty"] = int(m.group(1))
        result["row_step"] = int(m.group(2))

    # .take(N)
    m = RE_TAKE_N.search(body)
    if m:
        result["max_visible_rows"] = int(m.group(1))

    # for i in 0..N
    if "max_visible_rows" not in result:
        m = RE_FOR_RANGE.search(body)
        if m:
            result["max_visible_rows"] = int(m.group(1))

    # for slot in 0..N
    if "max_visible_rows" not in result:
        m = re.search(r'for\s+\w+\s+in\s+0\.\.(\d+)', body)
        if m:
            result["max_visible_rows"] = int(m.group(1))

    return result if result else None


def detect_conditional(body: str) -> list[str]:
    """Detect conditional branches (match/if) in the function body.
    Returns list of warnings."""
    warnings = []
    if 'match ' in body and ('phase' in body or 'state.' in body.lower()):
        warnings.append("conditional layout: function branches on phase/state - may need split variants")
    if re.search(r'if\s+state\.\w+\(\)', body):
        warnings.append("conditional layout: function has state-dependent if-branches")
    if re.search(r'if\s+party\.is_empty\(\)', body):
        warnings.append("conditional layout: empty-party vs populated-party")
    return warnings


def detect_grid_construction(body: str) -> bool:
    """Detect if the function builds a grid programmatically (e.g., naming.rs keyboard)."""
    for_loops = len(re.findall(r'for\s+\(\s*\w+\s*,\s*\w+\s*\)', body))
    gb_tiles = len(re.findall(r'frame\.gb_tile\(', body))
    return for_loops >= 2 or gb_tiles >= 5


def clean_variant(variant: dict) -> dict:
    """Remove internal tracking fields (_-prefixed keys) from the variant
    and its nested structures. Preserves _TODO keys. Separates dynamic labels."""
    def clean_dict(d):
        if not isinstance(d, dict):
            return d
        cleaned = {}
        for k, v in d.items():
            if k.startswith("_") and k != "_TODO":
                continue
            if isinstance(v, dict):
                cleaned[k] = clean_dict(v)
            elif isinstance(v, list):
                cleaned[k] = [clean_dict(item) if isinstance(item, dict) else item for item in v]
            else:
                cleaned[k] = v
        return cleaned

    result = clean_dict(variant)

    dynamic_labels = {}
    def extract_dynamic(boxes_or_regions, prefix):
        nonlocal dynamic_labels
        for i, item in enumerate(boxes_or_regions):
            labels = item.get("labels", [])
            static_labels = []
            for label in labels:
                if label.get("text") == "_TODO: dynamic label" or "_TODO" in label:
                    dl_id = f"{prefix}_{i}_l{len(dynamic_labels)}"
                    dynamic_labels[dl_id] = {
                        "parent": item.get("id", f"{prefix}_{i}"),
                        "tx": label.get("tx"),
                        "ty": label.get("ty"),
                        "color": label.get("color", "Black"),
                        "_TODO": label.get("_TODO", "dynamic label from source"),
                    }
                else:
                    static_labels.append(label)
            if static_labels or labels == []:
                if static_labels:
                    item["labels"] = static_labels
                else:
                    item.pop("labels", None)
            else:
                item.pop("labels", None)

    if "boxes" in result:
        extract_dynamic(result["boxes"], "box")
    if "regions" in result:
        extract_dynamic(result["regions"], "region")

    if dynamic_labels:
        result["dynamic_labels"] = dynamic_labels

    return result


# ── Main extraction logic ──────────────────────────────────────────

def parse_menu_file(filepath: Path) -> dict:
    """Parse a menu .rs file and return { variant_name: VariantDef }."""
    src = filepath.read_text()
    screen_name = filepath.stem
    consts = parse_consts(src)
    functions = find_function_bodies(src)

    warnings: list[str] = []
    variants: dict = {}

    # Build a local-var map for each function
    for func in functions:
        body = func["body"]
        name = func["name"]
        variant_key = name
        if name.startswith("draw_"):
            variant_key = name[5:]  # strip "draw_" prefix
        if not variant_key or variant_key in ("draw",):
            variant_key = "default"

        # Skip if this is a helper that just delegates (has very short body)
        # This handles cases like stats.rs draw() which dispatches
        if len(body.strip()) < 100 and ('match' in body or 'if' in body):
            # Check if it's just a dispatcher
            if re.search(r'StatsPage::(Stats|Moves)\s*=>\s*draw_page', body):
                warnings.append(f"  [{screen_name}] {name}: dispatcher function, extracting dispatched pages instead")
                # For stats.rs, we'll handle dispatched pages separately
                continue

        local_vars = {}
        # Extract rect variable assignments
        for m in RE_RECT_VAR.finditer(body):
            var = m.group(1)
            local_vars[var] = {
                "tx": int(m.group(2)),
                "ty": int(m.group(3)),
                "tw": int(m.group(4)),
                "th": int(m.group(5)),
            }
        # Extract static u32 let bindings
        for m in re.finditer(r'let\s+(\w+)\s*=\s*(\d+)\s*(?:_u32)?\s*;', body):
            local_vars[m.group(1)] = int(m.group(2))

        variant = {}
        boxes = []
        regions = []
        primitives = []

        # ── Extract ui.text_box() boxes ──────────────────────────
        # Flexible regex: captures raw strings (digit or variable) for each rect param
        for m in re.finditer(
            r'ui\.text_box\(\s*TileRect::new\(\s*([^,]+)\s*,\s*([^,]+)\s*,\s*([^,]+)\s*,\s*([^)]+)\s*\)\s*,\s*InkColor::(\w+)\s*,',
            body
        ):
            raw_tx, raw_ty, raw_tw, raw_th = m.group(1), m.group(2), m.group(3), m.group(4)
            rect = {}
            unresolved = []
            for key, raw in [("tx", raw_tx), ("ty", raw_ty), ("tw", raw_tw), ("th", raw_th)]:
                val = resolve_var(raw.strip(), consts, local_vars)
                if val is not None:
                    rect[key] = val
                else:
                    rect[key] = None
                    unresolved.append(f"{key}={raw.strip()}")
            box = {
                "id": f"box_{len(boxes)}",
                "rect": rect,
                "color": m.group(5),
            }
            if unresolved:
                box["_TODO"] = f"unresolved rect params: {', '.join(unresolved)}; inspect source"
            closure_start = m.end()
            closure_body = find_closure_body(body, closure_start)
            if closure_body:
                labels = []
                for lm in RE_LABEL_STATIC.finditer(closure_body):
                    labels.append({
                        "tx": int(lm.group(1)),
                        "ty": int(lm.group(2)),
                        "text": unescape(lm.group(3)),
                        "color": lm.group(4),
                    })
                if labels:
                    box["labels"] = labels
            boxes.append(box)

        # text_box with variable name
        for m in RE_TEXT_BOX_VAR.finditer(body):
            varname = m.group(1)
            color = m.group(2)
            rect = local_vars.get(varname)
            if isinstance(rect, dict):
                box = {
                    "id": f"box_{len(boxes)}",
                    "rect": rect,
                    "color": color,
                }
                closure_start = m.end()
                closure_body = find_closure_body(body, closure_start)
                if closure_body:
                    labels = []
                    for lm in RE_LABEL_STATIC.finditer(closure_body):
                        labels.append({
                            "tx": int(lm.group(1)),
                            "ty": int(lm.group(2)),
                            "text": unescape(lm.group(3)),
                            "color": lm.group(4),
                        })
                    if labels:
                        box["labels"] = labels
                boxes.append(box)
            else:
                boxes.append({
                    "id": f"box_{len(boxes)}",
                    "_TODO": f"unresolved rect variable '{varname}' for text_box",
                    "color": color,
                })

        # ── Extract ui.region() regions ──────────────────────────
        for m in RE_REGION_LITERAL.finditer(body):
            region = {
                "id": f"region_{len(regions)}",
                "rect": {
                    "tx": int(m.group(1)),
                    "ty": int(m.group(2)),
                    "tw": int(m.group(3)),
                    "th": int(m.group(4)),
                },
            }
            # Extract labels from region closure
            closure_start = m.end()
            closure_body = find_closure_body(body, closure_start)
            if closure_body:
                labels = []
                for lm in RE_LABEL_STATIC.finditer(closure_body):
                    # Check if this label is inside a nested text_box — skip if so
                    labels.append({
                        "tx": int(lm.group(1)),
                        "ty": int(lm.group(2)),
                        "text": unescape(lm.group(3)),
                        "color": lm.group(4),
                    })
                if labels:
                    region["labels"] = labels
                # TODO: extract cursors from regions too
            regions.append(region)

        for m in RE_REGION_VAR.finditer(body):
            varname = m.group(1)
            rect = local_vars.get(varname)
            if isinstance(rect, dict):
                region = {
                    "id": f"region_{len(regions)}",
                    "rect": rect,
                }
                closure_start = m.end()
                closure_body = find_closure_body(body, closure_start)
                if closure_body:
                    labels = []
                    for lm in RE_LABEL_STATIC.finditer(closure_body):
                        labels.append({
                            "tx": int(lm.group(1)),
                            "ty": int(lm.group(2)),
                            "text": unescape(lm.group(3)),
                            "color": lm.group(4),
                        })
                    if labels:
                        region["labels"] = labels
                regions.append(region)
            else:
                regions.append({
                    "id": f"region_{len(regions)}",
                    "_TODO": f"unresolved rect variable '{varname}' for region",
                })

        # ── Extract primitives ───────────────────────────────────
        # bracket_box with literal rect
        for m in RE_BRACKET_BOX_LITERAL.finditer(body):
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "bracket_box",
                "color": m.group(7),
                "rect": {
                    "tx": int(m.group(1)),
                    "ty": int(m.group(2)),
                    "tw": int(m.group(3)),
                    "th": int(m.group(4)),
                },
                "sides": parse_bracket_sides(m.group(5)),
                "with_arrow": m.group(6) == "true",
            })
        for m in RE_BRACKET_BOX_VAR.finditer(body):
            varname = m.group(1)
            rect = local_vars.get(varname)
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "bracket_box",
                "color": m.group(4),
                "rect": rect if isinstance(rect, dict) else {"_TODO": f"unresolved rect '{varname}'"},
                "sides": parse_bracket_sides(m.group(2)),
                "with_arrow": m.group(3) == "true",
            })

        # hp_bar
        for m in RE_HP_BAR.finditer(body):
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "hp_bar",
                "color": "Black",  # will be refined
                "tx": int(m.group(1)),
                "ty": int(m.group(2)),
                "width_tiles": int(m.group(3)),
                "_TODO": "verify hp_bar color - extracted from InkColor in full call",
            })

        # vline
        for m in RE_VLINE.finditer(body):
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "vline",
                "color": m.group(4),
                "tx": int(m.group(1)),
                "ty": int(m.group(2)),
                "length_tiles": int(m.group(3)),
            })

        # hline
        for m in RE_HLINE.finditer(body):
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "hline",
                "color": m.group(4),
                "tx": int(m.group(1)),
                "ty": int(m.group(2)),
                "length_tiles": int(m.group(3)),
            })

        # pixel_rect
        for m in RE_PIXEL_RECT.finditer(body):
            primitives.append({
                "id": f"prim_{len(primitives)}",
                "parent_id": None,
                "kind": "pixel_rect",
                "color": m.group(5),
                "px": int(m.group(1)),
                "py": int(m.group(2)),
                "pw": int(m.group(3)),
                "ph": int(m.group(4)),
            })

        # ── Extract cursor ───────────────────────────────────────
        cursor = None
        cursor_formula = extract_cursor_formula(body, consts)
        if cursor_formula:
            cursor_tx = 1
            cursor_color = "Black"
            cursor_glyph = "▶"

            # Search for cursor_at with full color
            cm = re.search(
                r'frame\.cursor_at\(\s*(\d+)\s*,\s*(\w+)\s*,\s*InkColor::(\w+)\s*\)',
                body
            )
            if cm:
                cursor_tx = int(cm.group(1))
                cursor_color = cm.group(3)
            else:
                # cursor_glyph_at with char/string + color
                cgm = re.search(
                    r'frame\.cursor_glyph_at\(\s*(\d+)\s*,\s*(\w+)\s*,\s*'
                    r'[\'\"]((?:[^\'\"\\]|\\.)*)[\'\"]\s*,\s*InkColor::(\w+)\s*\)',
                    body
                )
                if cgm:
                    cursor_tx = int(cgm.group(1))
                    cursor_glyph = unescape(cgm.group(3))
                    cursor_color = cgm.group(4)
            cursor = {
                "tx": cursor_tx,
                "base_ty": cursor_formula.get("base_ty", 0),
                "row_step": cursor_formula.get("row_step", 1),
                "glyph": cursor_glyph,
                "color": cursor_color,
            }
            if "_TODO" in cursor_formula:
                cursor["_TODO"] = cursor_formula["_TODO"]
        elif re.search(r'frame\.cursor_glyph_at|frame\.cursor_at', body):
            cursor = {
                "_TODO": "cursor formula not detected - inspect source for cursor_row/ty computation",
            }

        # ── Extract list params ──────────────────────────────────
        list_params = extract_list_params(body)
        if list_params and cursor:
            list_params["cursor"] = cursor
            cursor = None  # moved to list

        # ── Check for dynamic height ─────────────────────────────
        m_dh = RE_DYNAMIC_HEIGHT.search(body)
        if m_dh:
            base_h = int(m_dh.group(2))
            for box in boxes:
                th_val = box.get("rect", {}).get("th")
                if th_val is not None and th_val == base_h:
                    box["dynamic_height"] = {"extra_per_row": int(m_dh.group(3))}
                    break

        m_dh2 = RE_DYNAMIC_H_FORMULA.search(body)
        if m_dh2:
            base = int(m_dh2.group(2))
            extra = int(m_dh2.group(4))
            for box in boxes:
                th_val = box.get("rect", {}).get("th")
                if th_val is not None and th_val == base:
                    box["dynamic_height"] = {"extra_per_row": extra}
                    break

        # ── Check for conditional layouts ────────────────────────
        cond_warnings = detect_conditional(body)
        for w in cond_warnings:
            warnings.append(f"  [{screen_name}] {name}: {w}")

        # ── Check for programmatic grid ──────────────────────────
        if detect_grid_construction(body):
            warnings.append(f"  [{screen_name}] {name}: programmatic grid detected - likely needs hand-authoring")

        # ── Extract enum position maps ───────────────────────────
        enum_map = extract_enum_positions(body)

        # ── Assemble and clean variant ────────────────────────────
        if boxes:
            variant["boxes"] = boxes
        if regions:
            variant["regions"] = regions
        if primitives:
            variant["primitives"] = primitives
        if cursor:
            variant["cursor"] = cursor
        if list_params:
            variant["list"] = list_params
        if enum_map:
            variant["enum_position_map"] = enum_map

        if not variant:
            variant["_TODO"] = f"no layout data extracted for {name}"
        else:
            has_real_data = False
            for box in boxes:
                if "rect" in box:
                    vals = [v for v in box["rect"].values() if v is not None]
                    if vals:
                        has_real_data = True
                        break
            if not has_real_data:
                for region in regions:
                    if "rect" in region:
                        vals = [v for v in region["rect"].values() if v is not None]
                        if vals:
                            has_real_data = True
                            break
            if not has_real_data:
                variant["_TODO"] = f"no concrete layout data extracted for {name} - all rects are unresolved"

        variant = clean_variant(variant)
        variants[variant_key] = variant

    # ── Handle cases where variants are empty ────────────────────
    if not variants:
        # Check if all draw_ functions were dispatchers
        # For stats.rs: we need to look for private fn draw_page1/draw_page2
        all_funcs = find_function_bodies(src)
        # Re-run with private functions included
        for func in all_funcs:
            body = func["body"]
            name = func["name"]
            if name.startswith("draw_") and len(name) > 5:
                variant_key = name[5:]
                local_vars = {}
                for m in RE_RECT_VAR.finditer(body):
                    local_vars[m.group(1)] = {
                        "tx": int(m.group(2)),
                        "ty": int(m.group(3)),
                        "tw": int(m.group(4)),
                        "th": int(m.group(5)),
                    }

                variant = {}
                boxes = []
                regions = []
                primitives = []

                # Extract boxes
                for m in RE_TEXT_BOX_LITERAL.finditer(body):
                    box = {
                        "id": f"box_{len(boxes)}",
                        "rect": {"tx": int(m.group(1)), "ty": int(m.group(2)), "tw": int(m.group(3)), "th": int(m.group(4))},
                        "color": m.group(5),
                    }
                    closure_body = find_closure_body(body, m.end())
                    if closure_body:
                        labels = []
                        for lm in RE_LABEL_STATIC.finditer(closure_body):
                            labels.append({"tx": int(lm.group(1)), "ty": int(lm.group(2)), "text": unescape(lm.group(3)), "color": lm.group(4)})
                        if labels:
                            box["labels"] = labels
                    boxes.append(box)

                # Extract regions
                for m in RE_REGION_LITERAL.finditer(body):
                    region = {
                        "id": f"region_{len(regions)}",
                        "rect": {"tx": int(m.group(1)), "ty": int(m.group(2)), "tw": int(m.group(3)), "th": int(m.group(4))},
                    }
                    closure_body = find_closure_body(body, m.end())
                    if closure_body:
                        labels = []
                        for lm in RE_LABEL_STATIC.finditer(closure_body):
                            labels.append({"tx": int(lm.group(1)), "ty": int(lm.group(2)), "text": unescape(lm.group(3)), "color": lm.group(4)})
                        if labels:
                            region["labels"] = labels
                    regions.append(region)

                # Extract primitives
                for m in RE_BRACKET_BOX_LITERAL.finditer(body):
                    primitives.append({
                        "id": f"prim_{len(primitives)}", "parent_id": None, "kind": "bracket_box",
                        "color": m.group(7),
                        "rect": {"tx": int(m.group(1)), "ty": int(m.group(2)), "tw": int(m.group(3)), "th": int(m.group(4))},
                        "sides": parse_bracket_sides(m.group(5)),
                        "with_arrow": m.group(6) == "true",
                    })
                for m in RE_HP_BAR.finditer(body):
                    primitives.append({
                        "id": f"prim_{len(primitives)}", "parent_id": None, "kind": "hp_bar",
                        "color": "Black", "tx": int(m.group(1)), "ty": int(m.group(2)), "width_tiles": int(m.group(3)),
                        "_TODO": "verify hp_bar color",
                    })
                for m in RE_PIXEL_RECT.finditer(body):
                    primitives.append({
                        "id": f"prim_{len(primitives)}", "parent_id": None, "kind": "pixel_rect",
                        "color": m.group(5),
                        "px": int(m.group(1)), "py": int(m.group(2)), "pw": int(m.group(3)), "ph": int(m.group(4)),
                    })

                if boxes:
                    variant["boxes"] = boxes
                if regions:
                    variant["regions"] = regions
                if primitives:
                    variant["primitives"] = primitives
                if variant:
                    variants[variant_key] = variant

    # ── Clean up _TODO on individual boxes if variant has overall _TODO ──
    # (remove duplicate info)

    return {"variants": variants, "warnings": warnings}


# ── Validation ─────────────────────────────────────────────────────

def validate_output(filepath: Path) -> list[str]:
    """Validate a generated JSON file. Returns list of error messages."""
    errors = []
    try:
        data = json.loads(filepath.read_text())
    except json.JSONDecodeError as e:
        return [f"{filepath.name}: invalid JSON: {e}"]

    # Check schema_version
    if data.get("schema_version") != 1:
        errors.append(f"{filepath.name}: schema_version must be 1, got {data.get('schema_version')}")

    # Check screen
    if "screen" not in data:
        errors.append(f"{filepath.name}: missing 'screen' field")

    # Check variants
    variants = data.get("variants", {})
    if not isinstance(variants, dict):
        errors.append(f"{filepath.name}: 'variants' must be a dict")
        return errors

    for vname, variant in variants.items():
        for box in variant.get("boxes", []):
            if "rect" in box:
                r = box["rect"]
                for key in ("tx", "ty", "tw", "th"):
                    val = r.get(key)
                    if val is None:
                        continue  # unresolved — expected for dynamic values
                    if not isinstance(val, int) or val < 0:
                        errors.append(f"{filepath.name}: {vname}.boxes.{box.get('id','?')}.rect.{key} = {val} (must be non-negative int)")
            if "color" in box:
                if not isinstance(box["color"], str):
                    errors.append(f"{filepath.name}: {vname}.boxes.{box.get('id','?')}.color must be string")
            # Check labels
            for li, label in enumerate(box.get("labels", [])):
                for key in ("tx", "ty"):
                    val = label.get(key)
                    if val is not None and (not isinstance(val, int) or val < 0):
                        errors.append(f"{filepath.name}: {vname}.boxes.{box.get('id','?')}.labels[{li}].{key} = {val}")

        # Check regions
        for region in variant.get("regions", []):
            if "rect" in region:
                r = region["rect"]
                for key in ("tx", "ty", "tw", "th"):
                    val = r.get(key)
                    if val is not None and (not isinstance(val, int) or val < 0):
                        errors.append(f"{filepath.name}: {vname}.regions.{region.get('id','?')}.rect.{key} = {val}")

        # Check cursor
        cursor = variant.get("cursor", {})
        if cursor and isinstance(cursor, dict):
            for key in ("tx", "base_ty", "row_step"):
                val = cursor.get(key)
                if val is not None and (not isinstance(val, int) or val < 0):
                    errors.append(f"{filepath.name}: {vname}.cursor.{key} = {val}")

        # Check list
        lst = variant.get("list", {})
        if lst and isinstance(lst, dict):
            for key in ("item_start_ty", "row_step", "max_visible_rows"):
                val = lst.get(key)
                if val is not None and (not isinstance(val, int) or val < 0):
                    errors.append(f"{filepath.name}: {vname}.list.{key} = {val}")

        # Check primitives
        for prim in variant.get("primitives", []):
            if "color" in prim and not isinstance(prim["color"], str):
                errors.append(f"{filepath.name}: {vname}.primitives.{prim.get('id','?')}.color must be string")
            if "rect" in prim:
                r = prim["rect"]
                if isinstance(r, dict):
                    for key in ("tx", "ty", "tw", "th"):
                        val = r.get(key)
                        if val is not None and (not isinstance(val, int) or val < 0):
                            errors.append(f"{filepath.name}: {vname}.primitives.{prim.get('id','?')}.rect.{key} = {val}")

        # Check enum_position_map
        epm = variant.get("enum_position_map", {})
        if epm and isinstance(epm, dict):
            for k, val in epm.items():
                if not isinstance(val, int) or val < 0:
                    errors.append(f"{filepath.name}: {vname}.enum_position_map.{k} = {val}")

    return errors


# ── Main ───────────────────────────────────────────────────────────

def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    all_warnings = []
    files_generated = []

    for menu_file in sorted(MENUS_DIR.glob("*.rs")):
        if menu_file.name in SKIP_FILES:
            if menu_file.name == "mart.rs":
                print(f"  SKIP {menu_file.name} — mart.json already hand-authored (Stage 0)", file=sys.stderr)
            continue

        screen = menu_file.stem
        print(f"Processing {screen}...", file=sys.stderr)

        result = parse_menu_file(menu_file)
        variants = result["variants"]
        warnings = result["warnings"]

        if warnings:
            all_warnings.extend(warnings)
            for w in warnings:
                print(w, file=sys.stderr)

        out = {
            "schema_version": 1,
            "screen": screen,
            "variants": variants,
        }

        out_path = OUT_DIR / f"{screen}.json"
        # Ensure consistent key ordering for idempotency
        out_json = json.dumps(out, indent=2, ensure_ascii=False, sort_keys=False)
        # Reorder: move schema_version, screen, variants to top for readability
        parsed = json.loads(out_json)
        ordered = {
            "schema_version": parsed["schema_version"],
            "screen": parsed["screen"],
            "variants": parsed["variants"],
        }
        out_json = json.dumps(ordered, indent=2, ensure_ascii=False) + "\n"
        out_path.write_text(out_json)
        files_generated.append(out_path.name)
        print(f"  -> {out_path}", file=sys.stderr)

    # ── Validation pass ───────────────────────────────────────────
    print("\nValidating outputs...", file=sys.stderr)
    all_errors = []
    for fname in files_generated:
        errors = validate_output(OUT_DIR / fname)
        all_errors.extend(errors)

    # also validate mart.json
    mart_path = OUT_DIR / "mart.json"
    if mart_path.exists():
        errors = validate_output(mart_path)
        all_errors.extend(errors)

    if all_errors:
        print(f"\nVALIDATION FAILED ({len(all_errors)} errors):", file=sys.stderr)
        for err in all_errors:
            print(f"  ERROR: {err}", file=sys.stderr)
        sys.exit(1)

    # ── Verify mart.json was NOT overwritten ──────────────────────
    if mart_path.exists():
        mart_data = json.loads(mart_path.read_text())
        if mart_data.get("screen") != "mart":
            print("ERROR: mart.json was overwritten!", file=sys.stderr)
            sys.exit(1)

    # ── Summary ──────────────────────────────────────────────────
    total_json = len(list(OUT_DIR.glob("*.json")))
    print(f"\nDone. {total_json} JSON files in {OUT_DIR}/", file=sys.stderr)
    print(f"  Files generated: {len(files_generated)}", file=sys.stderr)
    print(f"  Warnings: {len(all_warnings)}", file=sys.stderr)

    total_todos = 0
    for fname in sorted(OUT_DIR.glob("*.json")):
        content = fname.read_text()
        count = content.count("_TODO")
        if count > 0:
            total_todos += count
            print(f"  {fname.name}: {count} _TODO markers", file=sys.stderr)

    if total_todos > 0:
        print(f"\n  TOTAL _TODO markers across all files: {total_todos}", file=sys.stderr)
        print("  These must be manually filled in before Stage 1.6.", file=sys.stderr)

    print(f"\nAll outputs valid. schema_version=1 confirmed.", file=sys.stderr)


if __name__ == "__main__":
    main()
