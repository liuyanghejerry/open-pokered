"""Reversible request-only field dictionary; never ranks or omits evidence."""
from collections import Counter
import json

FIELD_DICTIONARY = 'decision_field_dictionary'
FIELD_DICTIONARY_INSTRUCTION = (
    ' Wire field dictionary: state.decision_field_dictionary maps each $fN key '
    'to its full original field name. Read every such key in state and JSON '
    'candidate descriptions with that original meaning, including nested '
    'records. Values, record membership, candidate IDs, evidence references '
    'and tables are unchanged; canonical paths in these instructions retain '
    'their original meanings. This is a reversible field-name spelling only.')


def compact_decision_field_wire(state, candidates):
    """Transmit profitable repeated field names once, retaining their meanings.

    Keep top-level state names and evidence/table protocol names descriptive.
    Original inputs are never mutated, plain criteria and unaffected JSON
    strings retain their exact bytes, and reserved-key collisions fail closed.
    Bytes measure representation size, not provider tokens or capacity.
    """
    protected = set(state) | {
        'shared_strategy_evidence', 'shared_strategy_evidence_ref', '$e',
        'strategy_table', 'columns', 'rows', FIELD_DICTIONARY}
    decoded = {}
    for key, value in candidates.items():
        try:
            decoded[key] = json.loads(value)
        except (TypeError, ValueError):
            decoded[key] = value
    counts = Counter()

    def collect(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == FIELD_DICTIONARY or key.startswith('$f'):
                    raise ValueError('Reserved decision field dictionary key collision')
                counts[key] += 1
                collect(child)
        elif isinstance(value, list):
            for child in value:
                collect(child)

    collect(state)
    for value in decoded.values():
        collect(value)
    aliases = {}
    for key, count in counts.items():
        alias = f'$f{len(aliases)}'
        saving = (count * (len(json.dumps(key).encode()) - len(json.dumps(alias).encode()))
                  - len(json.dumps({alias: key}).encode()))
        if key not in protected and count > 1 and saving > 16:
            aliases[key] = alias
    if not aliases:
        return state, candidates

    def encode(value):
        if isinstance(value, dict):
            return {aliases.get(key, key): encode(child) for key, child in value.items()}
        if isinstance(value, list):
            return [encode(child) for child in value]
        return value

    wire_state = encode(state)
    wire_state[FIELD_DICTIONARY] = {alias: key for key, alias in aliases.items()}
    wire_candidates = {}
    for key, value in candidates.items():
        encoded = encode(decoded[key])
        wire_candidates[key] = (json.dumps(encoded, separators=(',', ':'), ensure_ascii=False)
                                if encoded != decoded[key] else value)
    before = len(json.dumps({'state': state, 'criteria': candidates}).encode())
    after = len(json.dumps({'state': wire_state, 'criteria': wire_candidates}).encode())
    # The dictionary and its complete decoding instruction must pay for
    # themselves. No format change for an unprofitable small request.
    if after + len(FIELD_DICTIONARY_INSTRUCTION.encode()) >= before:
        return state, candidates
    return wire_state, wire_candidates


def restore_decision_field_wire(state, candidates):
    """Invert the dictionary for exact state/candidate semantic audits."""
    dictionary = state.get(FIELD_DICTIONARY)
    if dictionary is None:
        return state, candidates
    if (not isinstance(dictionary, dict) or not dictionary
            or any(not isinstance(alias, str) or not alias.startswith('$f')
                   or not isinstance(key, str) or key.startswith('$f')
                   or key == FIELD_DICTIONARY for alias, key in dictionary.items())
            or len(set(dictionary.values())) != len(dictionary)):
        raise ValueError('Invalid decision field dictionary')

    def decode(value):
        if isinstance(value, dict):
            result = {}
            for key, child in value.items():
                if key.startswith('$f') and key not in dictionary:
                    raise ValueError('Missing decision field dictionary key')
                original = dictionary.get(key, key)
                if original in result:
                    raise ValueError('Conflicting decision field dictionary key')
                result[original] = decode(child)
            return result
        if isinstance(value, list):
            return [decode(child) for child in value]
        return value

    restored = decode({key: value for key, value in state.items() if key != FIELD_DICTIONARY})
    options = {}
    for key, value in candidates.items():
        try:
            payload = json.loads(value)
        except (TypeError, ValueError):
            options[key] = value
            continue
        original = decode(payload)
        options[key] = (json.dumps(original, separators=(',', ':'), ensure_ascii=False)
                        if original != payload else value)
    return restored, options


def expand_decision_evidence(state, candidates):
    """Expand evidence formats and aliases, rejecting malformed references.

    The root library is encoding metadata. Expand every reference reachable
    from the complete world and current candidates, without selecting facts.
    """
    state, candidates = restore_decision_field_wire(state, candidates)
    library = state.get('shared_strategy_evidence') or {}

    def expand(value, visiting=()):
        if isinstance(value, list):
            return [expand(child, visiting) for child in value]
        if not isinstance(value, dict):
            return value
        if set(value) in ({'shared_strategy_evidence_ref'}, {'$e'}):
            key = next(iter(value.values()))
            if not isinstance(key, str) or key not in library or key in visiting:
                raise ValueError('Missing or cyclic shared evidence reference')
            return expand(library[key], (*visiting, key))
        if set(value) == {'strategy_table'}:
            table = value['strategy_table']
            if (not isinstance(table, dict) or set(table) != {'columns', 'rows'}
                    or not isinstance(table['columns'], list) or not isinstance(table['rows'], list)
                    or any(not isinstance(key, str) for key in table['columns'])
                    or len(set(table['columns'])) != len(table['columns'])
                    or any(not isinstance(row, list) or len(row) != len(table['columns'])
                           for row in table['rows'])):
                raise ValueError('Malformed shared evidence record table')
            return [{key: expand(child, visiting) for key, child in zip(table['columns'], row)}
                    for row in table['rows']]
        return {key: expand(child, visiting) for key, child in value.items()}

    original_state = {key: expand(value) for key, value in state.items() if key != 'shared_strategy_evidence'}
    original_options = {}
    for key, value in candidates.items():
        try:
            payload = json.loads(value)
        except (ValueError, TypeError):
            original_options[key] = value
            continue
        original = expand(payload)
        original_options[key] = (json.dumps(original, separators=(',', ':'), ensure_ascii=False)
                                 if original != payload else value)
    return original_state, original_options
