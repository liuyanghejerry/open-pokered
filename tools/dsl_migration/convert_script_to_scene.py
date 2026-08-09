#!/usr/bin/env python3
"""Convert a single map's script.js + script_config.json to a .scene file.

Usage: convert_script_to_scene.py <map_dir> [--out <out_dir>]

The .scene file uses the dotzuki-engine-dsl Game DSL, which compiles to the
same JS that script.js would have produced. The DSL compiler picks up
.scene files via build.rs.
"""

import argparse
import esprima
import json
import os
import re
import sys
from pathlib import Path
from typing import Any


class ConvertError(Exception):
    pass


def js_string_lit(value) -> str:
    """Encode a Python value as a JS string literal (DSL-compatible)."""
    if not isinstance(value, str):
        value = str(value)
    escaped = (
        value.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
    )
    return f'"{escaped}"'


def expr_to_dsl(node, const_table=None) -> str:
    """Recursively convert a JS expression AST node to a DSL expression string."""
    if const_table is None:
        const_table = {}

    if node.type == "Literal":
        if isinstance(node.value, str):
            return js_string_lit(node.value)
        if isinstance(node.value, bool):
            return "true" if node.value else "false"
        if node.value is None:
            return "null"
        n = node.value
        if isinstance(n, float) and n.is_integer():
            return str(int(n))
        return str(n)

    if node.type == "Identifier":
        return node.name

    if node.type == "MemberExpression":
        obj = expr_to_dsl(node.object, const_table)
        if node.computed:
            prop = expr_to_dsl(node.property, const_table)
            return f"{obj}[{prop}]"
        if node.property.type == "Identifier":
            return f"{obj}.{node.property.name}"
        return f"{obj}.{expr_to_dsl(node.property, const_table)}"

    if node.type == "AwaitExpression":
        return expr_to_dsl(node.argument, const_table)

    if node.type == "CallExpression":
        callee = node.callee
        if callee.type == "MemberExpression" and not callee.computed:
            obj, prop = callee.object, callee.property
            if obj.type == "Identifier" and obj.name == "game" and prop.type == "Identifier":
                method = prop.name
                args = [expr_to_dsl(a, const_table) for a in node.arguments]
                return f"{method}({', '.join(args)})"
        callee_str = expr_to_dsl(callee, const_table)
        args = [expr_to_dsl(a, const_table) for a in node.arguments]
        return f"{callee_str}({', '.join(args)})"

    if node.type == "BinaryExpression":
        op = node.operator
        if op == "===":
            op = "=="
        elif op == "!==":
            op = "!="
        lhs = _resolve_member(node.left, const_table, expr_to_dsl)
        rhs = _resolve_member(node.right, const_table, expr_to_dsl)
        return f"({lhs} {op} {rhs})"

    if node.type == "LogicalExpression":
        op = "&&" if node.operator == "&&" else "||"
        lhs = _resolve_member(node.left, const_table, expr_to_dsl)
        rhs = _resolve_member(node.right, const_table, expr_to_dsl)
        return f"({lhs} {op} {rhs})"

    if node.type == "UnaryExpression":
        op = node.operator
        if op == "!":
            arg = _resolve_member(node.argument, const_table, expr_to_dsl)
            return f"(!{arg})"
        if op == "-":
            return f"-{expr_to_dsl(node.argument, const_table)}"
        if op == "typeof":
            return f"typeof {expr_to_dsl(node.argument, const_table)}"

    if node.type == "MemberExpression":
        return _resolve_member(node, const_table, expr_to_dsl)

    if node.type == "ArrayExpression":
        elements = [expr_to_dsl(e, const_table) for e in node.elements if e is not None]
        return f"[{', '.join(elements)}]"

    if node.type == "ObjectExpression":
        parts = []
        for prop in node.properties:
            if prop.type != "Property":
                continue
            if prop.key.type == "Identifier":
                key = prop.key.name
            elif prop.key.type == "Literal":
                key = js_string_lit(prop.key.value)
            else:
                key = expr_to_dsl(prop.key, const_table)
            val = expr_to_dsl(prop.value, const_table)
            parts.append(f"{key} = {val}")
        return "{" + ", ".join(parts) + "}"

    if node.type == "TemplateLiteral":
        parts = []
        for q in node.quasis:
            parts.append(q.value.cooked)
        for i, e in enumerate(node.expressions):
            parts.append(f"{{{{{expr_to_dsl(e, const_table)}}}}}")
        return js_string_lit("".join(parts))

    if node.type == "ConditionalExpression":
        c = expr_to_dsl(node.test, const_table)
        t = expr_to_dsl(node.consequent, const_table)
        a = expr_to_dsl(node.alternate, const_table)
        return f"({c} ? {t} : {a})"

    raise ConvertError(f"unsupported expression node: {node.type}")


def stmt_to_dsl(node, indent=0, const_table=None) -> str:
    """Convert a JS statement AST node to one or more DSL statement strings.

    Returns a list of (indent_level, dsl_text) tuples or a single string.
    """
    if const_table is None:
        const_table = {}

    pad = "  " * indent

    if node.type == "ExpressionStatement":
        expr = node.expression
        if expr.type == "AwaitExpression":
            expr = expr.argument
        if expr.type == "CallExpression":
            callee = expr.callee
            args = [expr_to_dsl(a, const_table) for a in expr.arguments]

            if callee.type == "MemberExpression" and not callee.computed:
                obj, prop = callee.object, callee.property
                if obj.type == "Identifier" and obj.name == "game" and prop.type == "Identifier":
                    method = prop.name
                    if method == "showText" and args:
                        return _speaker_stmt(args[0], pad)
                    if method == "showChoice" and args:
                        return _choice_stmt(args, indent)
                    if method == "showObject" and args:
                        return [f"{pad}showObject({args[0]})\n"]
                    if method == "hideObject" and args:
                        return [f"{pad}hideObject({args[0]})\n"]
                    if method == "showEmotionBubble" and len(args) >= 2:
                        return [f"{pad}showEmotionBubble({args[0]}, {args[1]})\n"]
                    if method == "showPokedexEntry" and args:
                        return [f"{pad}showPokedexEntry({args[0]})\n"]
                    if method == "openNamingScreen" and args:
                        return [f"{pad}openNamingScreen({args[0]})\n"]
                    if method == "facePlayer" and args:
                        return [f"{pad}facePlayer({args[0]})\n"]
                    if method == "faceNpc" and len(args) >= 2:
                        return [f"{pad}faceNpc({args[0]}, {args[1]})\n"]
                    if method == "delay" and args:
                        return [f"{pad}delay({args[0]})\n"]
                    if method == "setJoyIgnore" and args:
                        arg = _resolve_member(expr.arguments[0], const_table, expr_to_dsl)
                        return [f"{pad}setJoyIgnore({arg})\n"]
                    if method == "clearJoyIgnore":
                        return [f"{pad}clearJoyIgnore()\n"]
                    if method == "setFlag" and args:
                        return [f"{pad}setFlag({args[0]})\n"]
                    if method == "resetFlag" and args:
                        return [f"{pad}resetFlag({args[0]})\n"]
                    if method == "getFlag" and args:
                        return [f"{pad}game.getFlag({args[0]})\n"]
                    if method == "moveNpc" and len(args) >= 2:
                        return [f"{pad}moveNpc({args[0]}, {args[1]})\n"]
                    if method == "moveNpcTo" and len(args) >= 3:
                        return [f"{pad}moveNpcTo({args[0]}, {args[1]}, {args[2]})\n"]
                    if method == "movePlayer" and args:
                        return [f"{pad}movePlayer({args[0]})\n"]
                    if method == "followNpc" and len(args) >= 3:
                        return [f"{pad}followNpc({args[0]}, {args[1]}, {args[2]})\n"]
                    if method == "setNpcPosition" and len(args) >= 3:
                        return [f"{pad}setNpcPosition({args[0]}, {args[1]}, {args[2]})\n"]
                    if method == "playMusic" and args:
                        return [f"{pad}playMusic({args[0]})\n"]
                    if method == "playSound" and args:
                        return [f"{pad}playSound({args[0]})\n"]
                    if method == "stopMusic":
                        return [f"{pad}stopMusic()\n"]
                    if method == "fadeOutMusic":
                        return [f"{pad}fadeOutMusic()\n"]
                    if method == "startBattle" and args:
                        return [f"{pad}startBattle({args[0]})\n"]
                    if method == "startGymBattle" and args:
                        return [f"{pad}startGymBattle({args[0]}, {args[1]})\n"]
                    if method == "startTrainerBattle" and args:
                        return [f"{pad}startTrainerBattle({args[0]})\n"]
                    if method == "heal":
                        return [f"{pad}heal()\n"]
                    if method == "giveItem" and len(args) >= 2:
                        return [f"{pad}giveItem({args[0]}, {args[1]})\n"]
                    if method == "takeItem" and len(args) >= 2:
                        return [f"{pad}takeItem({args[0]}, {args[1]})\n"]
                    if method == "givePokemon" and len(args) >= 2:
                        return [f"{pad}givePokemon({args[0]}, {args[1]})\n"]
                    if method == "removeItem" and len(args) >= 2:
                        return [f"{pad}removeItem({args[0]}, {args[1]})\n"]
                    if method == "giveBadge" and args:
                        return [f"{pad}giveBadge({args[0]})\n"]
                    if method == "takeMoney" and args:
                        return [f"{pad}takeMoney({args[0]})\n"]
                    if method == "fadeScreen" and args:
                        return [f"{pad}fadeScreen({args[0]})\n"]
                    if method == "warpTo" and len(args) >= 3:
                        return [f"{pad}warpTo({args[0]}, {args[1]}, {args[2]})\n"]
                    if method == "setMapScript" and args:
                        return [f"{pad}setMapScript({args[0]})\n"]
                    if method == "animateHealingMachine":
                        return [f"{pad}animateHealingMachine()\n"]
                    if method == "showTextChoice" and args:
                        return [f"{pad}showTextChoice({args[0]})\n"]
                    # Fallback: bare call
                    return [f"{pad}{method}({', '.join(args)})\n"]
            return [f"{pad}{expr_to_dsl(expr, const_table)}\n"]
        return [f"{pad}{expr_to_dsl(expr, const_table)}\n"]

    if node.type == "VariableDeclaration":
        out = []
        for decl in node.declarations:
            if decl.init is None:
                continue
            val = expr_to_dsl(decl.init, const_table)
            out.append(f"{pad}let {decl.id.name} = {val}\n")
        return out

    if node.type == "IfStatement":
        cond = expr_to_dsl(node.test, const_table)
        out = [f"{pad}@if ({cond}) {{\n"]
        cons = node.consequent
        if cons.type == "BlockStatement":
            for s in cons.body:
                out.extend(_ensure_list(stmt_to_dsl(s, indent + 1, const_table)))
        else:
            out.extend(_ensure_list(stmt_to_dsl(cons, indent + 1, const_table)))
        if node.alternate is None:
            out.append(f"{pad}}}\n")
            return out
        if node.alternate.type == "IfStatement":
            out.append(f"{pad}}} @else ")
            else_part = stmt_to_dsl(node.alternate, indent, const_table)
            inner = else_part[0] if isinstance(else_part, list) and len(else_part) == 1 else else_part
            if isinstance(inner, str):
                out[0] = out[0] + inner
            else:
                out.extend(else_part)
            return out
        out.append(f"{pad}}} @else {{\n")
        alt = node.alternate
        if alt.type == "BlockStatement":
            for s in alt.body:
                out.extend(_ensure_list(stmt_to_dsl(s, indent + 1, const_table)))
        else:
            out.extend(_ensure_list(stmt_to_dsl(alt, indent + 1, const_table)))
        out.append(f"{pad}}}\n")
        return out

    if node.type == "BlockStatement":
        out = []
        for s in node.body:
            out.extend(_ensure_list(stmt_to_dsl(s, indent, const_table)))
        return out

    if node.type == "ReturnStatement":
        if node.argument is None:
            return [f"{pad}return\n"]
        return [f"{pad}return {expr_to_dsl(node.argument, const_table)}\n"]

    raise ConvertError(f"unsupported statement node: {node.type}")


def _ensure_list(x):
    return x if isinstance(x, list) else [x]


def _speaker_stmt(arg: str, pad: str):
    """Convert `await game.showText("Name: text")` to a @speaker block.

    Heuristic: if the string starts with `Name: `, split speaker from body.
    Otherwise emit a system @speaker.
    """
    try:
        parsed = json.loads(arg)
    except Exception:
        return [f"{pad}{arg}\n"]
    if not isinstance(parsed, str):
        return [f"{pad}{arg}\n"]
    text = parsed
    if ": " in text and "\n" not in text.split(": ", 1)[0]:
        speaker, _, body = text.partition(": ")
        lines = body.split("\\n")
        lines_js = "\n".join(js_string_lit(l) for l in lines)
        return [f"{pad}@speaker({js_string_lit(speaker)}) {{\n", f"{pad}  {lines_js}\n", f"{pad}}}\n"]
    if ": " in text:
        # Multi-line with embedded \n; split on first newline after the speaker
        m = re.match(r"^([^:\n]+): (.*)$", text, re.DOTALL)
        if m:
            speaker, body = m.group(1), m.group(2)
            lines = body.split("\\n")
            lines_js = "\n".join(js_string_lit(l) for l in lines)
            return [f"{pad}@speaker({js_string_lit(speaker)}) {{\n", f"{pad}  {lines_js}\n", f"{pad}}}\n"]
    # No speaker prefix; use generic "System"
    lines = text.split("\\n")
    lines_js = "\n".join(js_string_lit(l) for l in lines)
    return [f"{pad}@speaker({js_string_lit('System')}) {{\n", f"{pad}  {lines_js}\n", f"{pad}}}\n"]


def _choice_stmt(args, indent):
    """Convert `await game.showChoice(["A", "B"])` to a @choice block."""
    pad = "  " * indent
    try:
        parsed = json.loads("[" + ", ".join(args) + "]")
    except Exception:
        return [f"{pad}showChoice({', '.join(args)})\n"]
    if not isinstance(parsed, list) or not parsed:
        return [f"{pad}showChoice({', '.join(args)})\n"]
    out = [f"{pad}@choice {{\n"]
    for label in parsed:
        out.append(f"{pad}  @option({js_string_lit(str(label))}) {{\n")
        out.append(f"{pad}  }}\n")
    out.append(f"{pad}}}\n")
    return out


def split_speaker_and_body(text: str):
    """Split 'Name: body' into (name, body) when Name has no whitespace/newline."""
    if ": " not in text:
        return None, text
    head, _, rest = text.partition(": ")
    if any(c in head for c in (" ", "\n", "\t")):
        return None, text
    return head, rest


def build_const_table(ast) -> dict:
    """Collect top-level `const X = { ... }` definitions so we can resolve `X.Y` later.

    Currently we just return the raw object literal value; downstream the converter
    may still emit `X.Y` literally since both DSL and JS accept that.
    """
    table = {}
    for stmt in ast.body:
        if stmt.type != "VariableDeclaration":
            continue
        for decl in stmt.declarations:
            if decl.init is None or decl.init.type != "ObjectExpression":
                continue
            if decl.id.type != "Identifier":
                continue
            try:
                value = _object_to_python(decl.init)
                table[decl.id.name] = value
            except Exception:
                pass
    return table


def _resolve_member(node, const_table, recurse):
    """If node is `EVENT.X` and `EVENT` is a known const table, return the
    resolved string value. Otherwise recurse normally.
    """
    if node.type == "MemberExpression" and not node.computed:
        obj = node.object
        prop = node.property
        if obj.type == "Identifier" and prop.type == "Identifier":
            table = const_table.get(obj.name)
            if isinstance(table, dict):
                val = table.get(prop.name)
                if isinstance(val, str):
                    return js_string_lit(val)
                if isinstance(val, (int, float, bool)):
                    return str(val).lower() if isinstance(val, bool) else str(val)
        return recurse(node, const_table)
    return recurse(node, const_table)


def _object_to_python(node):
    out = {}
    for prop in node.properties:
        if prop.type != "Property":
            continue
        key = prop.key.name if prop.key.type == "Identifier" else prop.key.value
        if prop.value.type == "Literal":
            out[key] = prop.value.value
        elif prop.value.type == "ObjectExpression":
            out[key] = _object_to_python(prop.value)
    return out


def convert_storyline(name: str, fn_body, indent: int, const_table: dict) -> str:
    pad = "  " * indent
    out = [f"{pad}@storyline({js_string_lit(name)}) {{\n"]
    for s in fn_body.body:
        try:
            converted = stmt_to_dsl(s, indent + 1, const_table)
            out.extend(_ensure_list(converted))
        except ConvertError as e:
            out.append(f"{pad}  // TODO: {e}\n")
    out.append(f"{pad}}}\n")
    return "".join(out)


def convert_onload(name: str, fn_body, indent: int, const_table: dict) -> str:
    pad = "  " * indent
    out = [f"{pad}@onLoad {{\n"]
    for s in fn_body.body:
        try:
            converted = stmt_to_dsl(s, indent + 1, const_table)
            out.extend(_ensure_list(converted))
        except ConvertError as e:
            out.append(f"{pad}  // TODO: {e}\n")
    out.append(f"{pad}}}\n")
    return "".join(out)


def make_trigger_line(trigger: dict, scene_name: str, pad: str) -> str:
    """Build a @trigger(...) line from script_config.json entry."""
    parts = []
    if "map" in trigger:
        parts.append(f'map = {js_string_lit(trigger["map"])}')
    else:
        parts.append(f'map = {js_string_lit(scene_name)}')
    if trigger.get("npc"):
        parts.append(f'npc = {js_string_lit(trigger["npc"])}')
    if trigger.get("onEnter"):
        parts.append("onEnter = true")
    if trigger.get("after"):
        parts.append(f'after = {js_string_lit(trigger["after"])}')
    return f"{pad}@trigger({', '.join(parts)})\n"


def make_coord_trigger(coord_event: dict, scene_name: str, pad: str) -> str:
    """Build a @trigger(...) line for a coord event (map + position)."""
    pos = coord_event.get("position", [0, 0])
    parts = [
        f'map = {js_string_lit(scene_name)}',
        f'coord = [{pos[0]}, {pos[1]}]',
    ]
    if coord_event.get("onEnter"):
        parts.append("onEnter = true")
    return f"{pad}@trigger({', '.join(parts)})\n"


def convert_file(js_source: str, config: dict, scene_name: str) -> str:
    """Convert one script.js + script_config.json pair to a .scene string."""
    ast = esprima.parseModule(js_source, options={"loc": True, "range": True})
    const_table = build_const_table(ast)

    out = []
    out.append(f"// Auto-converted from {scene_name}/script.js by convert_script_to_scene.py\n")
    out.append(f"// Manual review recommended: state machines, local vars, complex control flow.\n\n")
    out.append(f"game_scene {scene_name} {{\n")

    for stmt in ast.body:
        if stmt.type != "FunctionDeclaration" and stmt.type != "ExportNamedDeclaration":
            continue
        if stmt.type == "ExportNamedDeclaration" and stmt.declaration is not None:
            decl = stmt.declaration
        else:
            decl = stmt
        if decl.type != "FunctionDeclaration":
            continue
        fn = decl
        if fn.id is None:
            continue
        fn_name = fn.id.name
        if fn_name == config.get("onLoad") or fn_name.endswith("OnLoad"):
            out.append(convert_onload(fn_name, fn.body, 1, const_table))
            continue
        trigger_line = ""
        for npc_entry in config.get("npcs", []):
            if npc_entry.get("talk") == fn_name:
                trigger_line = make_trigger_line(
                    {
                        "map": scene_name,
                        "npc": npc_entry.get("toggleId") or npc_entry.get("id"),
                    },
                    scene_name,
                    "    ",
                )
                break
        if not trigger_line:
            for sign_entry in config.get("signs", []):
                if sign_entry.get("talk") == fn_name:
                    sign_id = str(sign_entry.get("id"))
                    npc = js_string_lit(fn_name)
                    trigger_line = (
                        "    @trigger(map = " + js_string_lit(scene_name)
                        + ", sign = " + js_string_lit(sign_id)
                        + ", npc = " + npc + ")\n"
                    )
                    break
        if not trigger_line:
            for coord_entry in config.get("coordEvents", []):
                if coord_entry.get("trigger") == fn_name:
                    trigger_line = make_coord_trigger(coord_entry, scene_name, "    ")
                    break
        out.append(f"  @storyline({js_string_lit(fn_name)}) {{\n")
        if trigger_line:
            out.append(trigger_line)
        for s in fn.body.body:
            try:
                converted = stmt_to_dsl(s, 2, const_table)
                out.extend(_ensure_list(converted))
            except ConvertError as e:
                out.append(f"    // TODO: {e}\n")
        out.append(f"  }}\n\n")

    out.append("}\n")
    return "".join(out)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("map_dir", type=Path)
    p.add_argument("--out", type=Path, default=None,
                   help="Output dir (defaults to map_dir)")
    p.add_argument("--keep-original", action="store_true",
                   help="Do not rename script.js")
    args = p.parse_args()

    map_dir: Path = args.map_dir
    out_dir: Path = args.out or map_dir
    scene_name = map_dir.name

    script_js = map_dir / "script.js"
    config_json = map_dir / "script_config.json"
    if not script_js.exists():
        sys.exit(f"missing {script_js}")
    if not config_json.exists():
        sys.exit(f"missing {config_json}")

    js_source = script_js.read_text(encoding="utf-8")
    config = json.loads(config_json.read_text(encoding="utf-8"))

    try:
        scene = convert_file(js_source, config, scene_name)
    except Exception as e:
        sys.exit(f"conversion error in {scene_name}: {e}")

    out_path = out_dir / "script.scene"
    out_path.write_text(scene, encoding="utf-8")
    print(f"wrote {out_path} ({len(scene)} bytes)")


if __name__ == "__main__":
    main()
