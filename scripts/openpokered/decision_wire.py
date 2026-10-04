"""Reversible request-only field dictionary; never ranks or omits evidence."""
from collections import Counter
import json
import re

FIELD_DICTIONARY = 'decision_field_dictionary'
FIELD_DICTIONARY_INSTRUCTION = (
    ' Wire field dictionary: state.decision_field_dictionary maps each $fN key '
    'to its full original field name. Read every such key in state and JSON '
    'candidate descriptions with that original meaning, including nested '
    'records. Values, record membership, candidate IDs, evidence references '
    'and tables are unchanged; canonical paths in these instructions retain '
    'their original meanings. This is a reversible field-name spelling only.')
STRING_REFERENCE_PREFIX = 'decision_string_reference_prefix'
STRING_REFERENCE_INSTRUCTION = (
    ' Wire string references: when state.decision_string_reference_prefix is @, '
    'an entire string matching @e followed by decimal digits denotes the complete '
    'entry with that eN ID in state.shared_strategy_evidence, exactly like a singleton '
    '$e object. Resolve references transitively, including library entries and JSON '
    'candidate descriptions. Restore original value types and record membership; '
    'all other strings keep their literal meanings. No facts or candidates are omitted.')
_STRING_REFERENCE = re.compile(r'@e[0-9]+\Z')
MAPPING_TABLE_SCHEMA = 'decision_mapping_table_schema'
SEQUENCE_TABLE_SCHEMA = 'decision_sequence_table_schema'
SEQUENCE_TABLE_INSTRUCTION = (
    ' Wire segmented record lists: when state.decision_sequence_table_schema is 1, '
    'each singleton $s is an ordered list of segments. Expand each segment as a '
    'complete list (including strategy_table and transitive evidence references), '
    'then concatenate the segments in order to restore the exact original list. '
    'Record fields stay descriptive; missing fields are not filled with null. '
    'Every record, value, list position and candidate is retained.')
MAPPING_TABLE_INSTRUCTION = (
    ' Wire mapped record tables: when state.decision_mapping_table_schema is 1, '
    'each singleton $m is [keys,columns,rows]. Row i is the complete record for keys[i]; '
    'zip columns with every row to restore the exact original ordered mapping. '
    'Resolve evidence references transitively in keys, columns and cells. No facts or options are omitted.')


def compact_decision_sequence_tables(state, candidates):
    """Pack contiguous same-shape runs in mixed lists, without a missing-value sentinel.

    Uniform lists already have the historical strategy_table encoding. Mixed
    party/PC records cannot share its columns, but their ordered runs can. Only
    profitable exact encodings are emitted; scalar/plain criteria stay literal.
    """
    tables = 0

    def encode(value):
        nonlocal tables
        if isinstance(value, dict):
            if '$s' in value or SEQUENCE_TABLE_SCHEMA in value:
                raise ValueError('Reserved segmented record list collision')
            return {key: encode(child) for key, child in value.items()}
        if not isinstance(value, list):
            return value
        children = [encode(child) for child in value]

        def shape(row):
            # Protocol wrappers encode other value types, not ordinary records.
            return (tuple(row) if isinstance(row, dict) and row and not
                set(row).intersection(('$s', '$m', '$e', 'shared_strategy_evidence_ref', 'strategy_table')) else None)

        segments, packed_runs, index = [], 0, 0
        while index < len(children):
            columns = shape(children[index])
            end = index + 1
            while end < len(children) and shape(children[end]) == columns:
                end += 1
            ordinary = children[index:end]
            packed = ({'strategy_table': {'columns': list(columns),
                'rows': [[row[column] for column in columns] for row in ordinary]}}
                if columns and len(ordinary) >= 3 else None)
            if packed and len(json.dumps(packed).encode()) < len(json.dumps(ordinary).encode()):
                segments.append(packed)
                packed_runs += 1
            elif segments and isinstance(segments[-1], list):
                segments[-1].extend(ordinary)
            else:
                segments.append(ordinary)
            index = end
        tagged = {'$s': segments}
        if (packed_runs and len(segments) > 1
                and len(json.dumps(tagged).encode()) < len(json.dumps(children).encode())):
            tables += 1
            return tagged
        return children

    wire, offered = encode(state), {}
    for key, value in candidates.items():
        try:
            payload = json.loads(value)
        except (TypeError, ValueError):
            payload = value
        encoded = encode(payload)
        offered[key] = (json.dumps(encoded, separators=(',', ':'), ensure_ascii=False)
                        if encoded != payload else value)
    if not tables:
        return state, candidates
    wire[SEQUENCE_TABLE_SCHEMA] = 1
    before = len(json.dumps({'state': state, 'criteria': candidates}).encode())
    after = len(json.dumps({'state': wire, 'criteria': offered}).encode())
    if after + len(SEQUENCE_TABLE_INSTRUCTION.encode()) >= before:
        return state, candidates
    return wire, offered


def compact_decision_mapping_tables(state, candidates):
    """Transpose homogeneous keyed records without losing keys, order or cells.

    No candidate selection or domain-specific threshold: only profitable exact
    representation changes. Reserved tags fail closed even in unselected data.
    """
    tables = 0

    def encode(value):
        nonlocal tables
        if isinstance(value, list):
            return [encode(child) for child in value]
        if not isinstance(value, dict):
            return value
        if '$m' in value or MAPPING_TABLE_SCHEMA in value:
            raise ValueError('Reserved mapped record table collision')
        children = {key: encode(child) for key, child in value.items()}
        records = list(children.values())
        if len(records) < 3 or not all(isinstance(row, dict) and row for row in records):
            return children
        columns = list(records[0])
        if not all(list(row) == columns for row in records):
            return children
        packed = {'$m': [list(children), columns,
            [[row[column] for column in columns] for row in records]]}
        if len(json.dumps(packed).encode()) >= len(json.dumps(children).encode()):
            return children
        tables += 1
        return packed

    wire = encode(state)
    offered = {}
    for key, value in candidates.items():
        try:
            payload = json.loads(value)
        except (TypeError, ValueError):
            payload = value
        encoded = encode(payload)
        offered[key] = (json.dumps(encoded, separators=(',', ':'), ensure_ascii=False)
                        if encoded != payload else value)
    if not tables:
        return state, candidates
    wire[MAPPING_TABLE_SCHEMA] = 1
    before = len(json.dumps({'state': state, 'criteria': candidates}).encode())
    after = len(json.dumps({'state': wire, 'criteria': offered}).encode())
    if after + len(MAPPING_TABLE_INSTRUCTION.encode()) >= before:
        return state, candidates
    return wire, offered


def compact_decision_string_references(state, candidates):
    """Reversibly spell exact reference objects as strings, after collisions fail.

    Only profitable request representations change. This does not shorten source
    text, rank candidates or reinterpret an ordinary literal as evidence.
    """
    library = state.get('shared_strategy_evidence')
    if not isinstance(library, dict):
        return state, candidates
    references = 0

    def encode(value):
        nonlocal references
        if isinstance(value, str) and _STRING_REFERENCE.fullmatch(value):
            raise ValueError('Reserved string reference collision')
        if isinstance(value, dict):
            if STRING_REFERENCE_PREFIX in value:
                raise ValueError('Reserved string reference metadata collision')
            if set(value) == {'$e'}:
                key = value['$e']
                if not isinstance(key, str) or not re.fullmatch(r'e[0-9]+', key) or key not in library:
                    raise ValueError('Invalid string reference target')
                references += 1
                return '@' + key
            return {key: encode(child) for key, child in value.items()}
        if isinstance(value, list):
            return [encode(child) for child in value]
        return value

    wire = encode(state)
    offered = {}
    for key, value in candidates.items():
        try:
            payload = json.loads(value)
        except (ValueError, TypeError):
            payload = value
        encoded = encode(payload)
        offered[key] = (json.dumps(encoded, separators=(',', ':'), ensure_ascii=False)
                        if encoded != payload else value)
    if not references:
        return state, candidates
    wire[STRING_REFERENCE_PREFIX] = '@'
    before = len(json.dumps({'state': state, 'criteria': candidates}).encode())
    after = len(json.dumps({'state': wire, 'criteria': offered}).encode())
    if after + len(STRING_REFERENCE_INSTRUCTION.encode()) >= before:
        return state, candidates
    return wire, offered


def restore_decision_string_references(state, candidates):
    """Restore tagged string references to exact singleton reference objects."""
    if STRING_REFERENCE_PREFIX not in state:
        return state, candidates
    if state[STRING_REFERENCE_PREFIX] != '@' or not isinstance(state.get('shared_strategy_evidence'), dict):
        raise ValueError('Invalid string reference metadata')
    library = state['shared_strategy_evidence']

    def decode(value):
        if isinstance(value, str) and _STRING_REFERENCE.fullmatch(value):
            key = value[1:]
            if key not in library:
                raise ValueError('Missing string reference target')
            return {'$e': key}
        if isinstance(value, dict):
            return {key: decode(child) for key, child in value.items()}
        if isinstance(value, list):
            return [decode(child) for child in value]
        return value

    restored = decode({key: value for key, value in state.items() if key != STRING_REFERENCE_PREFIX})
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


def compact_decision_field_wire(state, candidates):
    """Transmit profitable repeated field names once, retaining their meanings.

    Keep top-level state names and evidence/table protocol names descriptive.
    Original inputs are never mutated, plain criteria and unaffected JSON
    strings retain their exact bytes, and reserved-key collisions fail closed.
    Bytes measure representation size, not provider tokens or capacity.
    """
    protected = set(state) | {
        'shared_strategy_evidence', 'shared_strategy_evidence_ref', '$e',
        'strategy_table', 'columns', 'rows', FIELD_DICTIONARY, STRING_REFERENCE_PREFIX,
        MAPPING_TABLE_SCHEMA, '$m', SEQUENCE_TABLE_SCHEMA, '$s'}
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
    state, candidates = restore_decision_string_references(state, candidates)
    state, candidates = restore_decision_field_wire(state, candidates)
    library = state.get('shared_strategy_evidence') or {}
    mapped_tables = MAPPING_TABLE_SCHEMA in state
    if mapped_tables and (type(state[MAPPING_TABLE_SCHEMA]) is not int or state[MAPPING_TABLE_SCHEMA] != 1):
        raise ValueError('Invalid mapped record table schema')
    sequence_tables = SEQUENCE_TABLE_SCHEMA in state
    if sequence_tables and (type(state[SEQUENCE_TABLE_SCHEMA]) is not int or state[SEQUENCE_TABLE_SCHEMA] != 1):
        raise ValueError('Invalid segmented record list schema')

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
        if sequence_tables and '$s' in value:
            if set(value) != {'$s'}:
                raise ValueError('Malformed segmented record list tag')
            segments = expand(value['$s'], visiting)
            if (not isinstance(segments, list) or not segments
                    or any(not isinstance(segment, list) for segment in segments)):
                raise ValueError('Malformed segmented record list segment')
            return [child for segment in segments for child in segment]
        if mapped_tables and '$m' in value:
            if set(value) != {'$m'}:
                raise ValueError('Malformed mapped record table tag')
            table = expand(value['$m'], visiting)
            if (not isinstance(table, list) or len(table) != 3
                    or any(not isinstance(axis, list) for axis in table)):
                raise ValueError('Malformed mapped record table axes')
            keys, columns, rows = table
            if (not keys or not columns
                    or any(not isinstance(key, str) for key in keys + columns)
                    or len(set(keys)) != len(keys) or len(set(columns)) != len(columns)
                    or len(rows) != len(keys)
                    or any(not isinstance(row, list) or len(row) != len(columns) for row in rows)):
                raise ValueError('Malformed mapped record table records')
            return {key: expand(dict(zip(columns, row)), visiting) for key, row in zip(keys, rows)}
        if set(value) == {'strategy_table'}:
            table = expand(value['strategy_table'], visiting)
            if (not isinstance(table, dict) or set(table) != {'columns', 'rows'}
                    or not isinstance(table['columns'], list) or not isinstance(table['rows'], list)
                    or any(not isinstance(key, str) for key in table['columns'])
                    or len(set(table['columns'])) != len(table['columns'])
                    or any(not isinstance(row, list) or len(row) != len(table['columns'])
                           for row in table['rows'])):
                raise ValueError('Malformed shared evidence record table')
            return [expand(dict(zip(table['columns'], row)), visiting)
                    for row in table['rows']]
        return {key: expand(child, visiting) for key, child in value.items()}

    original_state = expand({key: value for key, value in state.items()
                            if key not in ('shared_strategy_evidence', MAPPING_TABLE_SCHEMA, SEQUENCE_TABLE_SCHEMA)})
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
