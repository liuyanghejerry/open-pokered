#!/usr/bin/env python3
"""Run autonomous two-layer Jev exploration from a real NEW GAME.

Only generic operation skills are reused from playthrough.py. No milestone
handler, ordered route, seeded party, scripted flag or warp is used.
"""
import argparse
import hashlib
import json
import os
import shutil
import signal
import sys
import tempfile
import time
import traceback
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from openpokered.autonomous_story import AutonomousStoryAgent, accumulate_capture_retreat, capture_preparation
from openpokered.autonomous_story import capture_blackout_evidence, accumulate_capture_blackout
from openpokered.playthrough_judgments import capture_species
from openpokered.collection_verification import require_collection_completion, verify_collection_continue
from openpokered.judgment_agent import load_objectives
from openpokered.playthrough_judgments import JevGame
from openpokered.typesafe import TypeSafeClient
import playthrough as pt

GOAL_OBJECTIVES = {
    'collect-dex': {'id': 'collect-dex', 'agent_verified': True,
                    'name': 'Register every species reachable in one Pokémon Red save without external '
                            'link trades; clear the story to open every acquisition path'},
    'max-coverage': {'id': 'max-coverage', 'agent_verified': True,
                     'name': 'Leave no bordering area unexplored and finish the first playthrough'},
    'fast-clear': {'id': 'fast-clear',
                   'satisfied_when': {'flag': 'EVENT_BEAT_CHAMPION_RIVAL'},
                   'name': 'Reach the Hall of Fame in as few operations and frames as possible'},
}
DEX_OBJECTIVE = GOAL_OBJECTIVES['collect-dex']


def recording_assets(root):
    """Fail early on an incomplete gfx checkout; fingerprint the PNG inputs.

    These essential files guard the empty-map/sprite failure, not pixel-level
    correctness of every asset. Native frame inspection is still required.
    """
    root = Path(root).resolve()
    required = ('sprites/red.png', 'tilesets/overworld.png', 'font/font.png',
                'pokemon/front/charizard.png', 'pokemon/back/charizardb.png')
    for name in required:
        path = root / name
        if not path.is_file() or not path.read_bytes().startswith(b'\x89PNG\r\n\x1a\n'):
            raise ValueError(f'Recording needs PNG asset {path}; run scripts/fetch-gfx.sh')
    files = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(root.rglob('*.png'))}
    return {'root': str(root), 'png_count': len(files),
            'sha256': hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()}


def boot_new_game(game):
    assert not game.save_path.exists(), 'fresh run must not offer CONTINUE'
    for _ in range(600):
        state = game.st()
        if state['screen'] == 'overworld':
            assert state['map_name'] == 'RedsHouse2F', state
            assert state['player_name'] == 'RED', state
            assert not state['party'], state
            return state
        game.tap('a', 24)
    raise RuntimeError('NEW GAME initialization did not finish')


def observations_valid(observations):
    """Reject shifted JSON replies before treating a checkpoint as reusable."""
    try:
        values = {cmd: response['data'] for cmd, response in observations.items() if response['ok']}
        return (isinstance(values['get_state'], dict) and 'screen' in values['get_state']
                and isinstance(values['get_flags'], dict)
                and all(isinstance(v, bool) for v in values['get_flags'].values())
                and all(isinstance(values[cmd], list) and all(isinstance(row, dict) and key in row for row in values[cmd])
                        for cmd, key in [('get_party', 'species'), ('get_bag', 'item'), ('get_npcs', 'text_id')]))
    except (KeyError, TypeError):
        return False


def native_checkpoint_safe(observations):
    """SRAM resumes a settled overworld, not a battle or modal runtime.

    In battle, persistent party HP can still be the pre-battle snapshot.
    Well-formed protocol observations therefore do not prove resumability.
    Missing control-state fields are unknown, not evidence of idle control.
    """
    if not isinstance(observations, dict) or not observations_valid(observations):
        return False
    state = observations['get_state']['data']
    return (state.get('screen') == 'overworld'
            and state.get('warp_fade') == 'Idle'
            and state.get('player_movement_state') == 'Idle'
            and all(state.get(key) is False for key in
                    ('script_running', 'script_awaiting_battle', 'door_exit_pending', 'fishing_active'))
            and all(key in state and state[key] is None for key in
                    ('active_script_effect', 'dialogue', 'choice', 'field_menu')))


def checkpoint_collection_audit(run):
    """Keep historical invalid acquisitions pending without rewriting native SRAM."""
    chain, seen = [], set()
    while run:
        run = Path(run).resolve()
        if run in seen:
            raise ValueError('collection audit checkpoint cycle')
        seen.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        if summary.get('collection_audit_schema') == 1:
            pending = dict(summary.get('collection_audit_pending', {}))
            break
        chain.append(run)
        parent = summary.get('resumed_from')
        run = pt.ROOT / parent if parent else None
    else:
        pending = {}
    for folder in reversed(chain):
        previous_owned = None
        with (folder / 'trace.jsonl').open() as stream:
            for line in stream:
                if '"dex_progress"' not in line:
                    continue
                event = json.loads(line)
                # Native RESTLESS_SOUL: Tower6F has no wild Marowak slot.
                # A new registration here is the old engine's illegal spirit catch.
                if (previous_owned is not None and 'Marowak' not in previous_owned
                        and event.get('map') == 'PokemonTower6F'
                        and 'Marowak' in event.get('acquired', [])):
                    pending['Marowak'] = {'reason': 'uncatchable_restless_soul',
                        'source_trace': str(folder / 'trace.jsonl'),
                        'elapsed_s': event['elapsed_s'], 'map': event['map']}
                previous_owned = set(event.get('owned_species', []))
    return pending


def checkpoint_capture_retreats(run):
    """Recover omitted capacity only from the matching recorded native battle.

    Older summaries retained balls and party but dropped the box observation.
    Do not infer historic capacity from today's box, an older attempt, or an
    unrelated/sibling run. Missing observations stay unknown.
    """
    run = Path(run).resolve()
    retreats = json.loads((run / 'summary.json').read_text()).get('capture_retreats', {})
    pending = {key for key, row in retreats.items() if 'storage_full' not in row['preparation']}
    seen = set()
    while run and pending:
        if run in seen:
            raise ValueError('capture retreat checkpoint cycle')
        seen.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        trace = run / 'trace.jsonl'
        latest, battle = {}, None
        if trace.is_file():
            with trace.open() as stream:
                for line in stream:
                    if not any('"' + kind + '"' in line for kind in
                               ('battle_started', 'battle_resolved', 'capture_retreat')):
                        continue
                    event = json.loads(line)
                    if event.get('kind') == 'battle_started':
                        battle = event['state']
                    elif event.get('kind') == 'battle_resolved':
                        battle = None
                    elif event.get('kind') == 'capture_retreat':
                        key = event['map'] + ':' + event['species']
                        latest[key] = (event, battle)
        for key in pending & latest.keys():
            pending.remove(key)  # Never substitute an older attempt if this one lacks evidence.
            event, battle = latest[key]
            saved = retreats[key]
            enemy = ((battle or {}).get('battle_live') or {}).get('enemy') or {}
            if (not battle or not isinstance(battle.get('party'), list)
                    or not (enemy.get('capture_species') or enemy.get('species'))
                    or not all(event.get(field) == value for field, value in saved.items())
                    or battle.get('map_name') != event['map']
                    or capture_species(enemy) != event['species']):
                continue
            observed = capture_preparation(battle.get('party', []), {}, battle)
            if 'storage_full' in observed:
                saved['preparation']['storage_full'] = observed['storage_full']
                saved['capacity_evidence_source'] = {'trace': str(trace), 'elapsed_s': event['elapsed_s']}
        parent = summary.get('resumed_from')
        run = (pt.ROOT / parent).resolve() if parent else None
    return retreats


def checkpoint_capture_retreat_totals(run):
    """Replay only ancestors of this save; never count discarded sibling runs."""
    chain, seen, totals = [], set(), {}
    while run:
        run = Path(run).resolve()
        if run in seen:
            raise ValueError('capture retreat checkpoint cycle')
        seen.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        if summary.get('capture_retreat_totals_schema') == 1:
            totals = summary['capture_retreat_totals']
            break
        chain.append(run)
        parent = summary.get('resumed_from')
        run = pt.ROOT / parent if parent else None
    for folder in reversed(chain):
        trace = folder / 'trace.jsonl'
        if not trace.is_file():
            continue  # Counts describe recorded events, not an invented total.
        with trace.open() as stream:
            for line in stream:
                if '"capture_retreat"' not in line:
                    continue
                event = json.loads(line)
                if event.get('kind') == 'capture_retreat':
                    accumulate_capture_retreat(totals, event)
    return totals


def checkpoint_capture_blackouts(run):
    """Recover losses from matched native battle pairs in this save's lineage.

    Generic battle_defeat rows cannot establish a capture loss or its costs.
    A persisted schema (including empty) is authoritative; replay only its
    descendants, never siblings or an attempt spanning two recording segments.
    """
    chain, seen, blackouts, totals = [], set(), {}, {}
    while run:
        run = Path(run).resolve()
        if run in seen:
            raise ValueError('capture blackout checkpoint cycle')
        seen.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        if summary.get('capture_blackouts_schema') == 1:
            blackouts, totals = summary.get('capture_blackouts'), summary.get('capture_blackout_totals')
            if not isinstance(blackouts, dict) or not isinstance(totals, dict):
                raise ValueError('invalid capture blackout checkpoint schema')
            break
        chain.append(run)
        parent = summary.get('resumed_from')
        run = pt.ROOT / parent if parent else None
    for folder in reversed(chain):
        trace, started = folder / 'trace.jsonl', None
        if not trace.is_file():
            continue  # Missing records stay unknown, not synthetic failures.
        with trace.open() as stream:
            for line in stream:
                if not any('"' + kind + '"' in line for kind in
                           ('battle_started', 'battle_resolved', 'capture_retreat')):
                    continue
                event = json.loads(line)
                kind = event.get('kind')
                if kind == 'battle_started':
                    started = event
                elif kind == 'capture_retreat':
                    blackouts.pop(event['map'] + ':' + event['species'], None)
                elif kind == 'battle_resolved':
                    before = (started or {}).get('state')
                    after = event.get('state')
                    row = capture_blackout_evidence(before, after) if isinstance(before, dict) and isinstance(after, dict) else None
                    if row is not None:
                        row['recorded_battle_pair'] = {'trace': str(trace),
                            'battle_started_elapsed_s': started.get('elapsed_s'),
                            'battle_resolved_elapsed_s': event.get('elapsed_s')}
                        blackouts[row['map'] + ':' + row['species']] = row
                        accumulate_capture_blackout(totals, row)
                    started = None  # Never reuse one start for a later result.
    return blackouts, totals


def checkpoint_first_clear_verification(run, restored):
    """Carry a verified ending along this save's lineage, not transient flags.

    Legacy collection checkpoints forgot this proof and re-offered the Elite
    Four after every resume. A live Hall of Fame record corroborates an actual
    ancestor proof; neither a victory flag nor the count alone creates one.
    """
    seen = set()
    while run:
        run = Path(run).resolve()
        if run in seen:
            raise ValueError('first-clear checkpoint cycle')
        seen.add(run)
        summary = json.loads((run / 'summary.json').read_text())
        proof = summary.get('first_clear_verification')
        if proof is not None:
            continued = proof.get('separate_process_continue', {}) if isinstance(proof, dict) else {}
            phases = proof.get('phases', []) if isinstance(proof, dict) else []
            signatures = [row.get('phase') for row in phases if isinstance(row, dict)]
            count = continued.get('hall_of_fame_count')
            digest = proof.get('autosave_sha256', '') if isinstance(proof, dict) else ''
            valid = (type(count) is int and count > 0 and continued.get('map_name') == 'PalletTown'
                and continued.get('badges') == 255 and isinstance(digest, str) and len(digest) == 64
                and all(c in '0123456789abcdef' for c in digest)
                and signatures and all(isinstance(p, list) and len(p) == 3 for p in signatures)
                and any(p[1] for p in signatures) and any(p[2] == 'TheEnd' for p in signatures)
                and signatures[-1][0] == 'title' and signatures[-1][1:] == [None, None])
            if not valid:
                raise ValueError('incomplete inherited first-clear verification')
            if (restored.get('badges') != 255 or type(restored.get('hall_of_fame_count')) is not int
                    or restored['hall_of_fame_count'] < count):
                raise ValueError('restored save contradicts inherited first-clear verification')
            return {**proof, 'inherited_from': proof.get('inherited_from', str(run))}
        parent = summary.get('resumed_from')
        run = pt.ROOT / parent if parent else None
    return None


def checkpoint_field_requirements(run):
    """Restore observed HM blockers, including legacy checkpoints that lost them."""
    chain, seen, requirements = [], set(), {}
    while run:
        run = Path(run).resolve()
        if run in seen:
            break
        seen.add(run)
        summary_path = run / 'summary.json'
        if not summary_path.is_file():
            break
        summary = json.loads(summary_path.read_text())
        if summary.get('field_requirements_schema') == 1:
            requirements.update(summary.get('preparation_requirements', {}))
            break
        chain.append((run, summary))
        parent = summary.get('resumed_from')
        run = (pt.ROOT / parent) if parent else None
    for folder, summary in reversed(chain):
        requirements.update(summary.get('preparation_requirements', {}))
        trace = folder / 'trace.jsonl'
        if not trace.is_file():
            continue
        with trace.open() as stream:
            for line in stream:
                if '"field_obstruction"' not in line:
                    continue
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if event.get('kind') != 'operation':
                    continue
                obstacle = (event.get('result') or {}).get('field_obstruction')
                if isinstance(obstacle, dict) and obstacle.get('move') in ('Cut', 'Surf', 'Strength'):
                    requirements[obstacle['move']] = obstacle
    return requirements


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--until', choices=[o['id'] for o in load_objectives()], default='become-champion')
    parser.add_argument('--goal', choices=['story', 'collect-dex', 'max-coverage', 'fast-clear'],
                        default='story')
    # `code` picks candidates without a model call: a dry run of the skills,
    # navigation and candidate generation at zero service cost.
    parser.add_argument('--strategy', choices=['jev', 'code'], default='jev')
    parser.add_argument('--action', choices=['jev', 'code'], default='jev')
    # Playstyle biases, not goals: each one biases the instruction and the candidates.
    parser.add_argument('--preference', choices=['none', 'level', 'type', 'tactic'], default='none')
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--binary', type=Path, default=pt.BIN)
    parser.add_argument('--model', default='jev-1.13.0')
    parser.add_argument('--jev-provider', choices=['auto', 'openrouter', 'typesafe'], default='auto',
                        help='Jev billing/API provider; auto prefers OPENROUTER_API_KEY when present')
    parser.add_argument('--max-calls', type=int, default=2500)
    parser.add_argument('--max-actions', type=int, default=2500)
    parser.add_argument('--wall-budget', type=float, default=7200)
    parser.add_argument('--frame-budget', type=int, default=4000000)
    parser.add_argument('--checkpoint', action='store_true', help='save a development checkpoint after stopping')
    parser.add_argument('--resume', type=Path, help='continue a checkpoint earned by this runner')
    parser.add_argument('--output', type=Path, default=Path('.artifacts/jev-autonomous'))
    parser.add_argument('--record-video', nargs='?', const='auto', metavar='FILE',
                        help='record every simulated frame to H.264 MP4; without FILE, write '
                             'jev-dex-full.mp4 inside the run directory')
    parser.add_argument('--record-video-fps', type=int, default=240,
                        help='game frames per video second (60 = real-time, 240 = 4×; default: 240)')
    args = parser.parse_args(argv)
    folder = args.output / (time.strftime('%Y%m%d-%H%M%S') + f'-seed{args.seed}')
    folder.mkdir(parents=True, exist_ok=False)
    video_path = None
    if args.record_video:
        video_path = ((folder / 'jev-dex-full.mp4').resolve() if args.record_video == 'auto'
                      else Path(args.record_video).expanduser().resolve())
        video_path.parent.mkdir(parents=True, exist_ok=True)
    # A 10s read timeout with one retry aborted a whole 25-minute run on a
    # single transient network blip; long runs need more slack than that.
    model = TypeSafeClient.from_env(provider=args.jev_provider, timeout=30, max_retries=2)
    if args.goal == 'story':
        objectives = load_objectives()
        objectives = objectives[:next(i for i, obj in enumerate(objectives) if obj['id'] == args.until) + 1]
    else:
        # The story objectives stay as the instrumental frontier: their flags
        # are what carries the run into new areas and resources. The goal entry
        # is appended last so it is the terminal requirement rather than the
        # story prefix — fast-clear names the champion flag itself.
        objectives = load_objectives() + [GOAL_OBJECTIVES[args.goal]]
    result = {'success': False, 'goal': args.goal, 'preference': args.preference,
              'target': args.until if args.goal == 'story' else args.goal,
              'layers': {'strategy': args.strategy, 'action': args.action},
              'seed': args.seed, 'mode': 'autonomous-new-game', 'uses_milestone_handlers': False}
    resolved_provider = getattr(model, 'provider', args.jev_provider)
    result['jev_provider'] = (resolved_provider if isinstance(resolved_provider, str)
                              else args.jev_provider)
    if video_path:
        assets = recording_assets(os.environ.get('POKERED_GFX_DIR', pt.ROOT / 'gfx'))
        # Ensure the child uses the same root we checked, not a stale build's
        # baked path to a different worktree.
        os.environ['POKERED_GFX_DIR'] = assets['root']
        result['recording'] = {'path': str(video_path), 'simulated_fps': 60,
                               'game_frames_per_video_second': args.record_video_fps,
                               'container': 'mp4', 'assets': assets}
    policy_files = [*Path(__file__).parent.glob('*.py'),
                    pt.ROOT / 'scripts/playthrough.py', pt.ROOT / 'scripts/playthrough_late.py',
                    pt.ROOT / 'scripts/debug_drive.py']
    result['policy_files'] = {str(p.relative_to(pt.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in sorted(policy_files)}
    result['policy_sha256'] = hashlib.sha256(json.dumps(result['policy_files'], sort_keys=True).encode()).hexdigest()
    # Formal saves and native logs must survive OS /tmp cleanup while running.
    # On an abnormal process loss this private directory remains inside the
    # durable evidence folder; normal shutdown exports evidence then cleans it.
    with tempfile.TemporaryDirectory(prefix='.runtime-', dir=folder.resolve()) as private:
        binary = Path(private) / 'pokered-app'
        shutil.copy2(args.binary, binary)
        saved = None
        parent = None
        if args.resume:
            parent = json.loads((args.resume / 'summary.json').read_text())
            if parent.get('mode') != 'autonomous-new-game':
                parser.error('resume requires an autonomous-run checkpoint')
            evidence = args.resume / 'final-observations.json'
            if not parent.get('development_checkpoint') or not native_checkpoint_safe(json.loads(evidence.read_text())):
                parser.error('resume requires a settled overworld checkpoint; native SRAM does not restore battle or modal runtime')
            saved = Path(private) / 'continuation.sav'
            shutil.copy2(args.resume / 'game.sav', saved)
            extras = args.resume / 'game.script_flags.json'
            if extras.exists():
                shutil.copy2(extras, binary.parent / 'pokered.script_flags.json')
            result['resumed_from'] = str(args.resume)
        result['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        # Preserve provenance before any long-running controller work. A
        # process loss must not require inferring its policy/binary/parent
        # from the worktree present at recovery time. This is NOT a checkpoint.
        (folder / 'run-manifest.json').write_text(json.dumps({**result,
            'status': 'starting', 'model': args.model,
            'budgets': {'wall_seconds': args.wall_budget, 'calls': args.max_calls,
                        'actions': args.max_actions, 'frames': args.frame_budget}},
            ensure_ascii=False, indent=2) + '\n')
        game = agent = None
        with (folder / 'trace.jsonl').open('w') as trace:
            try:
                game = JevGame(binary=binary, save_path=saved, seed=args.seed, speed=0,
                               record_video=video_path, record_video_fps=args.record_video_fps,
                               runtime_root=binary.parent)
                game.attach_judgments(model, model=args.model, trace=trace,
                                      max_calls=args.max_calls, wall_budget=args.wall_budget,
                                      frame_budget=args.frame_budget)
                if args.resume:
                    pt.resume_reentry(game)
                    initial = game.st()
                else:
                    initial = boot_new_game(game)
                game.judgments.record('resume' if args.resume else 'new_game', state=initial)
                agent = AutonomousStoryAgent(
                    game.judgments.client, model, objectives, game=game, model=args.model,
                    strategy_jev=args.strategy == 'jev', action_jev=args.action == 'jev',
                    preference=args.preference,
                    max_calls=args.max_calls, max_actions=args.max_actions,
                    wall_budget=args.wall_budget, frame_budget=args.frame_budget, trace=trace)
                if parent:
                    agent.first_clear_verification = checkpoint_first_clear_verification(args.resume, initial)
                    if agent.first_clear_verification is not None:
                        agent.record('first_clear_inherited',
                            source=agent.first_clear_verification['inherited_from'],
                            autosave_sha256=agent.first_clear_verification['autosave_sha256'],
                            restored_hall_of_fame_count=initial['hall_of_fame_count'])
                    agent.visited.update(parent.get('visited_maps', []))
                    agent.observed_barrier_maps.update(parent.get('observed_barrier_maps', []))
                    agent.navigation_memory.update(parent.get('navigation_memory', {}))
                    agent.navigation_history.update(parent.get('navigation_history', {}))
                    agent.mechanism_goal = parent.get('mechanism_goal')
                    # Earlier checkpoints stored only the most recent wall
                    # per destination. Recover this agent's own observed
                    # obstacles from its trace, never from a playthrough route.
                    if not parent.get('navigation_history'):
                        prior_trace = args.resume / 'trace.jsonl'
                        for line in prior_trace.read_text().splitlines():
                            try:
                                event = json.loads(line)
                            except json.JSONDecodeError:
                                continue
                            for blockage in event.get('state', {}).get('known_navigation_failures', {}).values():
                                key = json.dumps([blockage['destination'], blockage['map']])
                                agent.navigation_history[key] = blockage
                    agent.field_requirements.update(checkpoint_field_requirements(args.resume))
                    agent.battle_requirements.update(parent.get('battle_requirements', {}))
                    agent.capture_retreats.update(checkpoint_capture_retreats(args.resume))
                    agent.capture_retreat_totals.update(checkpoint_capture_retreat_totals(args.resume))
                    blackouts, blackout_totals = checkpoint_capture_blackouts(args.resume)
                    agent.capture_blackouts.update(blackouts)
                    agent.capture_blackout_totals.update(blackout_totals)
                    agent.collection_audit_pending.update(checkpoint_collection_audit(args.resume))
                    game.stationary_npcs = {name: {int(k): tuple(v) for k, v in npcs.items()}
                                            for name, npcs in parent.get('stationary_npcs', {}).items()}
                    agent.battle_defeats.extend(parent.get('battle_defeats', []))
                    agent.defeat_preparation = parent.get('defeat_preparation', 0)
                # Finish the outstanding JSON round trip before stopping.
                # Raising KeyboardInterrupt inside readline would leave its
                # reply queued and misalign final observations / checkpoint.
                signal.signal(signal.SIGINT, lambda *_: setattr(game.d, 'stop_requested', True))
                signal.signal(signal.SIGTERM, lambda *_: setattr(game.d, 'stop_requested', True))
                result.update(agent.run())
                result['action_cache_hits'] = game.move_cache_hits
            except (Exception, KeyboardInterrupt) as error:
                result.update(success=False, reason=f'{type(error).__name__}: {error}')
                (folder / 'failure.txt').write_text(traceback.format_exc())
            finally:
                if game is not None:
                    try:
                        if agent is not None:
                            result['visited_maps'] = sorted(agent.visited)
                            result['observed_barrier_maps'] = sorted(agent.observed_barrier_maps)
                            result['navigation_memory'] = agent.navigation_memory
                            result['navigation_history'] = agent.navigation_history
                            result['mechanism_goal'] = agent.mechanism_goal
                            result['preparation_requirements'] = dict(agent.field_requirements)
                            result['field_requirements_schema'] = 1
                            result['battle_requirements'] = agent.battle_requirements
                            result['capture_retreats'] = agent.capture_retreats
                            result['capture_retreat_totals_schema'] = 1
                            result['capture_retreat_totals'] = agent.capture_retreat_totals
                            result['capture_blackouts_schema'] = 1
                            result['capture_blackouts'] = agent.capture_blackouts
                            result['capture_blackout_totals'] = agent.capture_blackout_totals
                            result['collection_audit_schema'] = 1
                            result['collection_audit_pending'] = agent.collection_audit_pending
                            result['stationary_npcs'] = getattr(game, 'stationary_npcs', {})
                            result['battle_defeats'] = agent.battle_defeats
                            result['defeat_preparation'] = agent.defeat_preparation
                            result.setdefault('calls', dict(agent.calls))
                            result.setdefault('tokens', dict(agent.tokens))
                            result.setdefault('completed', list(agent.completed))
                            result.setdefault('actions', agent.actions)
                            result.setdefault('models', sorted(agent.models))
                            result.setdefault('resolved_script_battles', agent.resolved_battles)
                            if agent.first_clear_verification is not None:
                                result['first_clear_verification'] = agent.first_clear_verification
                        observations = {cmd: game.d.raw.cmd(cmd=cmd) for cmd in
                                        ('get_state', 'get_flags', 'get_party', 'get_bag', 'get_npcs')}
                        (folder / 'final-observations.json').write_text(json.dumps(observations, indent=2))
                        valid = observations_valid(observations)
                        result['final_observations_valid'] = valid
                        safe = native_checkpoint_safe(observations)
                        result['native_checkpoint_safe'] = safe
                        result['development_checkpoint'] = False
                        reply = observations['get_state']
                        final_state = reply.get('data') if isinstance(reply, dict) else None
                        result['final_dex'] = final_state.get('pokedex') if isinstance(final_state, dict) else None
                        if not valid:
                            result.update(success=False, reason='invalid_final_protocol_observations')
                        if valid:
                            game.d.raw.cmd(cmd='capture_frame', path=str((folder / 'final.png').resolve()))
                        completing_dex = args.goal == 'collect-dex' and result.get('success') is True
                        if (args.checkpoint or completing_dex) and valid:
                            result['development_checkpoint_screen'] = observations['get_state']['data']['screen']
                            if not safe:
                                result['checkpoint_rejection_reason'] = 'native_runtime_not_serialized'
                                if completing_dex:
                                    result.update(success=False, reason='unsafe_collection_checkpoint')
                            else:
                                reply = game.d.raw.cmd(cmd='save')
                                valid = bool(reply.get('ok')) and reply.get('data') is None
                                result['development_checkpoint'] = valid
                                if not valid:
                                    result.update(success=False, reason='invalid_checkpoint_acknowledgement')
                        if game.save_path.exists():
                            # Preserve stale/native SRAM as failure evidence only.
                            name = 'game.sav' if valid and safe else 'nonresumable-native-save.sav'
                            shutil.copy2(game.save_path, folder / name)
                        flags = binary.parent / 'pokered.script_flags.json'
                        if flags.exists():
                            name = 'game.script_flags.json' if valid and safe else 'nonresumable-native-save.script_flags.json'
                            shutil.copy2(flags, folder / name)
                        if completing_dex and valid and safe:
                            try:
                                require_collection_completion(observations, result.get('collection_audit_pending', {}))
                                proof = verify_collection_continue(folder / 'game.sav', binary, observations,
                                                                   folder / 'game.script_flags.json')
                                result['collection_continue_verification'] = proof
                                if agent is not None:
                                    agent.record('collection_continue_verified', verification=proof)
                            except Exception as error:
                                result.update(success=False, reason=f'collection_continue_verification_failed: {error}')
                        result['commands'] = game.d.counts
                        result['battles_driven'] = game.battles_driven
                        result['action_cache_hits'] = game.move_cache_hits
                        game.log.flush()
                        shutil.copy2(game.run_dir / 'game.log', folder / 'game.log')
                    finally:
                        game.close()
                        if video_path:
                            result['recording']['exists'] = video_path.is_file()
                            result['recording']['bytes'] = video_path.stat().st_size if video_path.is_file() else 0
    (folder / 'summary.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ('final_facts', 'commands', 'policy_files', 'first_clear_verification')}, ensure_ascii=False))
    print('Artifacts:', folder)
    return 0 if result['success'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
