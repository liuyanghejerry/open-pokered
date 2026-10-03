"""Independent native CONTINUE proof, never a substitute for source auditing."""
import hashlib
import shutil
import tempfile
import time
from pathlib import Path

import playthrough as pt
from .playthrough_judgments import ObservedProtocol


def valid_safari_snapshot(safari):
    return (isinstance(safari, dict) and type(safari.get('active')) is bool
            and type(safari.get('balls_remaining')) is int and 0 <= safari['balls_remaining'] <= 30
            and type(safari.get('steps_remaining')) is int and 0 <= safari['steps_remaining'] <= 500)


def collection_snapshot(observations):
    state = observations['get_state']['data']
    dex = state.get('pokedex') or {}
    owned = dex.get('owned_species') or []
    seen = dex.get('seen_species') or []
    if (state.get('screen') != 'overworld' or len(set(owned)) != len(owned)
            or len(set(seen)) != len(seen) or dex.get('owned') != len(owned)
            or dex.get('seen') != len(seen) or not set(owned) <= set(seen)):
        raise ValueError('Invalid overworld collection snapshot')
    stored = state.get('stored_pokemon')
    counts = state.get('box_counts')
    fields = {'box', 'index', 'species', 'level', 'hp', 'max_hp', 'status', 'moves', 'pp'}
    if (not isinstance(counts, list) or len(counts) != 12
            or any(type(count) is not int or not 0 <= count <= 20 for count in counts)
            or not isinstance(stored, list)
            or any(not isinstance(mon, dict) or not fields <= mon.keys()
                   or type(mon['box']) is not int or type(mon['index']) is not int
                   for mon in stored)):
        raise ValueError('Incomplete stored Pokemon observation')
    slots = [(mon['box'], mon['index']) for mon in stored]
    expected_slots = {(box, index) for box, count in enumerate(counts) for index in range(count)}
    if len(slots) != len(expected_slots) or set(slots) != expected_slots:
        raise ValueError('Invalid stored Pokemon slots or box counts')
    if not valid_safari_snapshot(state.get('safari_game')):
        raise ValueError('Incomplete or invalid Safari session observation')
    return {
        'dex': {**dex, 'owned_species': sorted(owned), 'seen_species': sorted(seen)},
        'state': {key: state[key] for key in (
            'map_name', 'player_x', 'player_y', 'money', 'coins', 'badges',
            'current_box_index', 'box_counts', 'safari_game')},
        'party': observations['get_party']['data'],
        # Counts alone cannot detect a replaced species, altered moves/HP,
        # or a different occupied slot. Compare every exposed stored field.
        'stored_pokemon': sorted(stored, key=lambda mon: (mon['box'], mon['index'])),
        'bag': observations['get_bag']['data'],
        'flags': observations['get_flags']['data'],
    }


def require_collection_completion(observations, pending):
    snapshot = collection_snapshot(observations)
    if pending or snapshot['dex']['owned'] != 124:
        raise ValueError('Collection is not a source-validated 124-species completion')
    return snapshot


def verify_collection_continue(saved, binary, observations, script_flags=None):
    """Boot an isolated copy using real CONTINUE and compare persisted facts.

    Also usable on partial saves to test persistence; the final completion
    gate must separately call require_collection_completion. No save editing,
    warping, seeding or connection to the original native instance is used.
    """
    expected = collection_snapshot(observations)
    saved, binary = Path(saved), Path(binary)
    if saved.stat().st_size != 32768:
        raise ValueError('Native SRAM must contain 32768 bytes')
    digest = hashlib.sha256(saved.read_bytes()).hexdigest()
    # Keep even the isolated verifier under the durable evidence directory.
    # System temporary cleanup must not remove an in-flight native save/log.
    with tempfile.TemporaryDirectory(prefix='.continue-', dir=saved.resolve().parent) as folder:
        folder = Path(folder)
        copied_binary, copied_save = folder / 'pokered-app', folder / 'collection.sav'
        shutil.copy2(binary, copied_binary)
        shutil.copy2(saved, copied_save)
        if script_flags and Path(script_flags).is_file():
            shutil.copy2(script_flags, folder / 'pokered.script_flags.json')
        check = pt.Game(save_path=copied_save, binary=copied_binary, seed=42, speed=0,
                        runtime_root=folder)
        check.d = ObservedProtocol(check.d, lambda *args, **kwargs: None, time.monotonic() + 120)
        try:
            pt.resume_reentry(check)
            restored_observations = {cmd: check.d.cmd(cmd=cmd) for cmd in (
                'get_state', 'get_party', 'get_bag', 'get_flags')}
            restored = collection_snapshot(restored_observations)
            differences = [key for key in expected if expected[key] != restored[key]]
            if differences:
                raise ValueError('CONTINUE changed persisted collection facts: ' + ', '.join(differences))
            if hashlib.sha256(saved.read_bytes()).hexdigest() != digest:
                raise ValueError('Source SRAM changed during isolated verification')
            return {'schema': 3, 'verified': True, 'save_sha256': digest,
                    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                    'separate_process_pid': check.proc.pid,
                    'verification_commands': dict(check.d.counts),
                    'expected': expected, 'restored': restored}
        finally:
            check.close()
