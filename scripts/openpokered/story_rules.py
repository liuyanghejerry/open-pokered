"""Condition-preserving scene rules for the two-level story experiment.

The game exports its statement AST. We retain branch polarity, early returns,
choice labels and preceding writes instead of treating every read as a required
true flag. This is a bounded planner, not a second game interpreter: unsupported
queries remain unknown, and predicted effects are always verified in the game.
"""
import copy
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path

MAPS_DIR = Path(__file__).resolve().parents[2] / 'crates/pokered-data/maps'
UNKNOWN = None
MAX_PATHS = 256


def default_hidden_toggles():
    """Resolve public SRAM defaults, including all engine toggle aliases."""
    source = (MAPS_DIR.parent / 'src/toggleable_objects.rs').read_text()
    source = re.sub(r'//[^\n]*', '', source)
    constants = {name: int(value, 0) for name, value in re.findall(
        r'pub const (\w+): ToggleableObject = ToggleableObject\((0x[0-9A-Fa-f]+)\)', source)}
    defaults = source.split('pub const DEFAULT_HIDDEN_TOGGLES:', 1)[1].split('];', 1)[0]
    hidden = {int(v, 0) for v in re.findall(r'0x[0-9A-Fa-f]+', defaults)}
    result = set()
    for labels, number, constant in re.findall(
            r'((?:"\w+"\s*\|\s*)*"\w+")\s*=>\s*\{?\s*Some\((?:(0x[0-9A-Fa-f]+)|ToggleableObject::(\w+)\.bit_index\(\))\)', source):
        bit = int(number, 0) if number else constants[constant]
        if bit in hidden:
            result.update(re.findall(r'"(\w+)"', labels))
    return result


DEFAULT_HIDDEN = default_hidden_toggles()


def trainer_victory_rules(maps_dir, configs, selected_maps):
    """Expose engine-owned trainer outcomes using its canonical header order.

    Dialogue scripts often only read these flags; the NPC battle engine is
    their producer. This supplies that missing dependency without a route.
    """
    source = (MAPS_DIR.parent / 'src/trainer_headers.rs').read_text()
    tables = {name: re.findall(r'event_flag:\s*EventFlag::(\w+)', body)
              for name, body in re.findall(
                  r'pub static (\w+):\s*\[TrainerHeaderData;\s*\d+\]\s*=\s*\[(.*?)\];', source, re.S)}
    rules = []
    for name, table in re.findall(r'MapId::(\w+)\s*=>\s*&(TRAINERS_\w+)', source):
        path = Path(maps_dir) / name / 'map.json'
        if name not in selected_maps or not path.exists():
            continue
        trainers = [npc for npc in json.loads(path.read_text()).get('npcs', []) if npc.get('isTrainer')]
        talks = {npc['id']: npc.get('talk') for npc in configs.get(name, {}).get('npcs', [])}
        for npc, flag in zip(trainers, tables.get(table, [])):
            effect = ('flag', flag, True)
            guard = {'Call': {'callee': 'getFlag', 'args': [literal(flag)]}}
            rules.append(Rule('trainer:' + flag, name,
                              f"{name}:{talks.get(npc['textId']) or 'engine_trainer'}",
                              [f"npc:{npc['textId']}"], [(guard, False)], [], effect,
                              [('battle', f"{npc.get('trainerClass')}:{npc.get('trainerSet')}", True)]))
    return rules


def literal(value):
    return {'BoolLit' if isinstance(value, bool) else
            'NumberLit' if isinstance(value, (int, float)) else 'StringLit': value}


def evaluate(expr, facts):
    if not isinstance(expr, dict) or len(expr) != 1:
        return UNKNOWN
    kind, value = next(iter(expr.items()))
    if kind == 'Visible':
        name, default_hidden = value
        flags = facts.get('flags', {})
        if flags.get('__OBJ_HIDDEN_' + name):
            return False
        if flags.get('__OBJ_SHOWN_' + name):
            return True
        if name in facts.get('object_visibility', {}):
            return facts['object_visibility'][name]
        return not default_hidden
    if kind in ('StringLit', 'NumberLit', 'BoolLit'):
        return value
    if kind == 'ArrayLit':
        items = [evaluate(item, facts) for item in value]
        return items if all(item is not None for item in items) else UNKNOWN
    if kind == 'Localized':
        return dict(value).get('en', next(iter(dict(value).values()), ''))
    if kind == 'UnaryOp':
        v = evaluate(value['operand'], facts)
        if v is None:
            return UNKNOWN
        return not v if value['op'] == 'Not' else -v if value['op'] == 'Neg' else UNKNOWN
    if kind == 'BinaryOp':
        a, b = evaluate(value['left'], facts), evaluate(value['right'], facts)
        op = value['op']
        if op == 'And':
            return False if a is False or b is False else UNKNOWN if a is None or b is None else bool(a and b)
        if op == 'Or':
            return True if a is True or b is True else UNKNOWN if a is None or b is None else bool(a or b)
        if a is None or b is None:
            return UNKNOWN
        funcs = {'Eq': lambda: a == b, 'Ne': lambda: a != b,
                 'Lt': lambda: a < b, 'Lte': lambda: a <= b,
                 'Gt': lambda: a > b, 'Gte': lambda: a >= b,
                 'Add': lambda: a + b, 'Sub': lambda: a - b,
                 'BitOr': lambda: int(a) | int(b)}
        return funcs[op]() if op in funcs else UNKNOWN
    if kind == 'Call':
        name = value['callee'].removeprefix('game.')
        args = [evaluate(a, facts) for a in value['args']]
        if any(a is None for a in args):
            return UNKNOWN
        if name == 'getFlag':
            return bool(facts.get('flags', {}).get(args[0]))
        if name == 'hasItem':
            return facts.get('bag', {}).get(args[0].replace('_', '').upper(), 0) > 0
        if name == 'getBadgeCount':
            return facts.get('badges', 0)
        if name in ('hasMoney', 'hasCoins'):
            amount = facts.get('money' if name == 'hasMoney' else 'coins')
            return None if amount is None else amount >= args[0]
        if name == 'getPlayerX':
            return facts.get('x')
        if name == 'getPlayerY':
            return facts.get('y')
        if name == 'getPlayerFacing':
            facing = facts.get('facing')
            return facing.lower() if isinstance(facing, str) else UNKNOWN
        if name == 'lang':
            return 'en'
    return UNKNOWN


def substitute(expr, context):
    if isinstance(expr, list):
        return [substitute(x, context) for x in expr]
    if not isinstance(expr, dict):
        return expr
    if 'Variable' in expr:
        return copy.deepcopy(context['variables'].get(expr['Variable'], expr))
    call = expr.get('Call')
    if call and call['callee'].removeprefix('game.') == 'getFlag':
        flag = evaluate(call['args'][0], {})
        if flag in context['writes']:
            return literal(context['writes'][flag])
    return {k: substitute(v, context) for k, v in expr.items()}


def requirements(expr, wanted, facts):
    """Alternative sets of missing predicates; None denotes an unknown guard."""
    value = evaluate(expr, facts)
    if value is not None and bool(value) == wanted:
        return [[]]
    if 'Visible' in expr:
        return [[('visibility', expr['Visible'][0], wanted)]]
    if 'UnaryOp' in expr and expr['UnaryOp']['op'] == 'Not':
        return requirements(expr['UnaryOp']['operand'], not wanted, facts)
    binary = expr.get('BinaryOp')
    if binary and binary['op'] in ('And', 'Or'):
        a = requirements(binary['left'], wanted, facts)
        b = requirements(binary['right'], wanted, facts)
        conjunctive = (binary['op'] == 'And') == wanted
        if conjunctive:
            return [x + y for x in a for y in b][:MAX_PATHS]
        return a + b
    # Only defer the spatial predicate, preserving any AND/OR-linked
    # flag or item guard above. The trigger's actual landing verifies it.
    if any(name in json.dumps(expr) for name in ('getPlayerX', 'getPlayerY', 'getPlayerFacing')):
        return [[]]
    call = expr.get('Call')
    if call and call['callee'].removeprefix('game.') in ('getFlag', 'hasItem'):
        key = evaluate(call['args'][0], facts)
        if key is not None:
            return [[('flag' if call['callee'].endswith('getFlag') else 'item', key, wanted)]]
    # Outcomes of an action earlier in this same script are postconditions,
    # not facts the player must arrange before triggering the script.
    if '"Result"' in json.dumps(expr):
        return [[]]
    return [[('unknown', json.dumps(expr, sort_keys=True), wanted)]]


@dataclass
class Rule:
    id: str
    map: str
    storyline: str
    triggers: list
    guards: list
    choices: list
    effect: tuple
    preceding: list

    def alternatives(self, facts):
        alternatives = [[]]
        for expr, wanted in self.guards:
            alternatives = [a + b for a in alternatives
                            for b in requirements(expr, wanted, facts)][:MAX_PATHS]
        return alternatives

    def missing(self, facts):
        return min(self.alternatives(facts),
                   key=lambda x: (sum(p[0] == 'unknown' for p in x), len(x)))

    def description(self):
        return {'map': self.map, 'script': self.storyline, 'produces': self.effect,
                'confirmation_options': self.choices,
                'preceding_effects': self.preceding[-5:]}


def compile_story(story):
    rules = []
    def command(name, args, ctx):
        name = name.removeprefix('game.')
        values = [evaluate(substitute(a, ctx), {}) for a in args]
        effect = None
        if name in ('setFlag', 'resetFlag') and values and isinstance(values[0], str):
            effect = ('flag', values[0], name == 'setFlag')
        elif name in ('giveItem', 'takeItem') and values and isinstance(values[0], str):
            effect = ('item', values[0], name == 'giveItem')
        elif name == 'givePokemon' and values:
            effect = ('pokemon', values[0], values[1] if len(values) > 1 else None)
        elif name.startswith('startBattle') or name == 'startWildBattle':
            effect = ('battle', values[0] if values else name, True)
        elif name == 'heal':
            effect = ('heal', 'party', True)
        elif name == 'openShop' and values and isinstance(values[0], list):
            effect = ('shop', tuple(values[0]), True)
        elif (name == 'warpTo' and len(values) == 3 and isinstance(values[0], str)
              and all(isinstance(v, (int, float)) for v in values[1:])):
            effect = ('transport', (values[0], int(values[1]), int(values[2])), True)
        elif name in ('movePlayerRelative', 'movePlayer'):
            effect = ('movement', name, True)
        elif name == 'replaceTileBlock' and len(values) == 3 and all(isinstance(v, (int, float)) for v in values):
            effect = ('block', f"{story['map']},{int(values[0])},{int(values[1])}", int(values[2]))
        elif name in ('hideObject', 'hideObjectByName', 'showObject', 'showObjectByName') and values:
            effect = ('visibility', str(values[0]), name.startswith('show'))
        if effect:
            key = json.dumps([story['id'], ctx['guards'], ctx['choices'], effect], sort_keys=True)
            rules.append(Rule(hashlib.sha256(key.encode()).hexdigest()[:16], story['map'],
                              story['id'], story['triggers'], copy.deepcopy(ctx['guards']),
                              list(ctx['choices']), effect, list(ctx['effects'])))
            ctx['effects'].append(effect)
            if effect[0] == 'flag':
                ctx['writes'][effect[1]] = effect[2]

    def walk(statements, paths):
        for statement in statements:
            kind, data = next(iter(statement.items()))
            following = []
            for ctx in paths:
                if kind == 'Return':
                    continue
                if kind == 'If':
                    expr = substitute(data['condition'], ctx)
                    arms = []
                    for wanted, branch in [(True, 'then_branch'), (False, 'else_branch')]:
                        known = evaluate(expr, {}) if not ('"Call"' in json.dumps(expr)) else None
                        if known is not None and bool(known) != wanted:
                            continue
                        arm = copy.deepcopy(ctx)
                        arm['guards'].append((expr, wanted))
                        original_arm = copy.deepcopy(arm)
                        outcomes = walk(data[branch], [arm])
                        arms.append((original_arm, outcomes))
                    def rejoinable(original, outcomes):
                        if len(outcomes) != 1:
                            return False
                        outcome = outcomes[0]
                        if any(outcome[k] != original[k] for k in ('guards', 'variables', 'writes', 'choices')):
                            return False
                        return all(effect[0] in ('visibility', 'block')
                                   for effect in outcome['effects'][len(original['effects']):])
                    if len(arms) == 2 and all(rejoinable(original, outcomes)
                                             for original, outcomes in arms):
                        # Optional visual/geometry effects already have their
                        # guarded rules. They are not prerequisites of later
                        # independent effects; avoid exponential path growth.
                        following.append(ctx)
                    else:
                        for _, outcomes in arms:
                            following.extend(outcomes)
                elif kind == 'Choice':
                    for option in data['options']:
                        arm = copy.deepcopy(ctx)
                        arm['choices'].append(evaluate(option['label'], {}))
                        following.extend(walk(option['body'], [arm]))
                elif kind in ('Each', 'Run'):
                    # Preserve uncertainty; never treat unsupported code as a
                    # path that has been proven executable.
                    ctx['guards'].append(({'Unsupported': kind}, True))
                    following.append(ctx)
                else:
                    if kind == 'Command':
                        command(data['name'], data['args'], ctx)
                    elif kind == 'Assign':
                        expr = substitute(data['value'], ctx)
                        if 'Call' in expr:
                            call = expr['Call']
                            if call['callee'].removeprefix('game.') == 'elevatorMenu':
                                options = evaluate(call['args'][0], {}) if call['args'] else None
                                if isinstance(options, list) and all(isinstance(v, str) for v in options):
                                    for index, label in enumerate(options):
                                        arm = copy.deepcopy(ctx)
                                        arm['variables'][data['name']] = literal(index)
                                        arm['choices'].append(label)
                                        following.append(arm)
                                    continue
                            command(call['callee'], call['args'], ctx)
                            if call['callee'].removeprefix('game.') in ('startBattle', 'startBattleSet', 'startWildBattle', 'giveItem'):
                                expr = {'Result': expr}
                        ctx['variables'][data['name']] = expr
                    following.append(ctx)
            paths = following
            if len(paths) > MAX_PATHS:
                raise ValueError(f"too many branch paths: {story['id']}")
        return paths
    if not story.get('program') and not story.get('effects'):
        return []
    if not story.get('program'):
        raise ValueError(f"missing planning AST: {story['id']}; rebuild the debug binary")
    walk(story['program'], [{'guards': [], 'choices': [], 'variables': {}, 'writes': {}, 'effects': []}])
    return list({r.id: r for r in rules}.values())


def normalize_bag(bag):
    if isinstance(bag, dict):
        return {k.replace('_', '').upper(): v for k, v in bag.items()}
    return {str(b.get('item') or b.get('name')).replace('_', '').upper():
            b.get('qty', b.get('quantity', 1)) for b in bag or []}


class StoryIndex:
    def __init__(self, client, maps_dir=None):
        self.client = client
        self.maps_dir = Path(maps_dir) if maps_dir else MAPS_DIR
        self.stories = {}
        self.rules = []
        self.by_effect = {}
        self.errors = []
        self.configs = {p.parent.name: json.loads(p.read_text())
                        for p in self.maps_dir.glob('*/script_config.json')}
        self.npc_toggles = {(name, npc['id']): (npc['toggleId'], npc.get('defaultHidden', False)
                                               or npc['toggleId'] in DEFAULT_HIDDEN)
                            for name, config in self.configs.items()
                            for npc in config.get('npcs', []) if npc.get('toggleId')}
        self.visibility_defaults = dict(self.npc_toggles.values())
        for name in client.script_semantics()['maps']:
            data = client.script_semantics(name)
            for story in data['storylines']:
                self.stories[story['id']] = story
                try:
                    rules = compile_story(story)
                except ValueError as e:
                    self.errors.append(str(e))
                    continue
                for rule in rules:
                    for trigger in rule.triggers:
                        if trigger.startswith('npc:'):
                            visibility = self.npc_toggles.get((rule.map, int(trigger.split(':')[1])))
                            if visibility:
                                rule.guards.insert(0, ({'Visible': list(visibility)}, True))
                    self.rules.append(rule)
                    self.by_effect.setdefault(rule.effect[:3], []).append(rule)
        for rule in trainer_victory_rules(self.maps_dir, self.configs,
                                          {s['map'] for s in self.stories.values()}):
            for trigger in rule.triggers:
                visibility = self.npc_toggles.get((rule.map, int(trigger.split(':')[1])))
                if visibility:
                    rule.guards.insert(0, ({'Visible': list(visibility)}, True))
            self.rules.append(rule)
            self.by_effect.setdefault(rule.effect, []).append(rule)
        from .boulder_skills import BOULDER_TARGETS, boulder_sources
        for flag, target in BOULDER_TARGETS.items():
            effect = ('flag', flag, True)
            guards = []
            if any(name == target['map'] for name, _ in self.npc_toggles):
                sources = boulder_sources(target['map'], tuple(target['target']), str(self.maps_dir))
                visibility = [self.npc_toggles.get((target['map'], text_id)) for text_id in sources]
                if visibility and all(visibility):
                    available = {'Visible': list(visibility[0])}
                    for toggle in visibility[1:]:
                        available = {'BinaryOp': {'op': 'Or', 'left': available, 'right': {'Visible': list(toggle)}}}
                    guards = [(available, True)]
            rule = Rule('boulder:' + flag, target['map'], target['map'] + ':engine_boulder',
                        [], guards, [], effect, [('field_move', 'Strength', True)])
            self.rules.append(rule)
            self.by_effect.setdefault(effect, []).append(rule)
        configs = {p.parent.name: p.read_text()
                   for p in sorted(self.maps_dir.glob('*/script_config.json'))}
        self.sha256 = hashlib.sha256(json.dumps(
            {'stories': self.stories, 'coordinate_configs': configs,
             'engine_trainers': [(r.id, r.map, r.triggers, r.preceding)
                                 for r in self.rules if r.id.startswith('trainer:')],
             'engine_boulders': BOULDER_TARGETS,
             'engine_boulder_guards': {r.id: r.guards for r in self.rules if r.id.startswith('boulder:')}},
            sort_keys=True).encode()).hexdigest()

    def satisfied(self, target, facts):
        kind, name, wanted = target
        if kind == 'flag':
            return bool(facts['flags'].get(name)) == wanted
        if kind == 'item':
            return (facts['bag'].get(name.replace('_', '').upper(), 0) > 0) == wanted
        if kind == 'level':
            return bool(facts['party']) and facts['party'][0]['level'] >= wanted
        if kind == 'heal':
            return facts.get('fully_recovered', False)
        if kind == 'bag_space':
            return len(facts['bag']) < wanted
        if kind == 'supply':
            return facts['bag'].get(name.replace('_', '').upper(), 0) >= wanted
        if kind == 'sale':
            return not facts['bag'].get(name.replace('_', '').upper(), 0)
        if kind == 'health':
            return all(mon['hp'] == mon['max_hp'] and mon['status'] == 'None' for mon in facts['party'])
        if kind == 'pp_reserve':
            from playthrough_late import move_data
            return all(pp > move_data(move)['pp'] * .5
                       for mon in facts['party'] if mon['hp'] > 0
                       for move, pp in zip(mon['moves'], mon['pp'])
                       if move != 'None' and move_data(move)['power'] > 0)
        if kind == 'visibility':
            return evaluate({'Visible': [name, self.visibility_defaults.get(name, False)]}, facts) == wanted
        if kind == 'move':
            return any(name in mon.get('moves', []) for mon in facts['party']) == wanted
        if kind == 'pokemon':
            return any(mon['species'].replace('_', '').upper() == str(name).replace('_', '').upper()
                       and (wanted is None or mon['level'] >= wanted) for mon in facts['party'])
        if kind in ('location', 'transport'):
            return [facts['map'], facts['x'], facts['y']] == list(name)
        if kind == 'block':
            return facts.get('block_values', {}).get(name) == wanted
        if kind == 'terrain':
            return (name in facts.get('cleared_terrain', [])) == wanted
        return False

    def frontier(self, target, facts, seen=(), depth=0):
        """Backchain missing facts to actionable producers; retain alternatives."""
        target = tuple(target)
        if target in seen or depth > 10 or self.satisfied(target, facts):
            return []
        found = []
        for rule in self.by_effect.get(target, []):
            alternatives = rule.alternatives(facts)
            if [] in alternatives:
                found.append(rule)
            else:
                for missing in alternatives:
                    for prerequisite in missing:
                        if prerequisite[0] != 'unknown':
                            found.extend(self.frontier(prerequisite, facts, seen + (target,), depth + 1))
        return list({r.id: r for r in found}.values())

    def coordinates(self, rule):
        result = []
        for trigger in rule.triggers:
            if trigger.startswith('coord:'):
                x, y = trigger[7:-1].split(',')
                result.append((int(x), int(y)))
        # Named triggers can bind positions in the runtime script config.
        path = self.maps_dir / rule.map / 'script_config.json'
        if path.exists():
            config = json.loads(path.read_text())
            for event in config.get('coordEvents', []):
                if event.get('trigger') == rule.storyline.partition(':')[2]:
                    result.append(tuple(event['position']))
        return list(dict.fromkeys(result))
