#!/usr/bin/env python3
"""Run autonomous two-layer Jev exploration from a real NEW GAME.

Only generic operation skills are reused from playthrough.py. No milestone
handler, ordered route, seeded party, scripted flag or warp is used.
"""
import argparse
import hashlib
import json
import shutil
import signal
import sys
import tempfile
import time
import traceback
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.judgment_agent import load_objectives
from openpokered.playthrough_judgments import JevGame
from openpokered.typesafe import TypeSafeClient
import playthrough as pt


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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--until', choices=[o['id'] for o in load_objectives()], default='become-champion')
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--binary', type=Path, default=pt.BIN)
    parser.add_argument('--model', default='jev-1.13.0')
    parser.add_argument('--max-calls', type=int, default=2500)
    parser.add_argument('--max-actions', type=int, default=2500)
    parser.add_argument('--wall-budget', type=float, default=7200)
    parser.add_argument('--frame-budget', type=int, default=4000000)
    parser.add_argument('--checkpoint', action='store_true', help='save a development checkpoint after stopping')
    parser.add_argument('--resume', type=Path, help='continue a checkpoint earned by this runner')
    parser.add_argument('--output', type=Path, default=Path('.artifacts/jev-autonomous'))
    args = parser.parse_args(argv)
    folder = args.output / (time.strftime('%Y%m%d-%H%M%S') + f'-seed{args.seed}')
    folder.mkdir(parents=True, exist_ok=False)
    model = TypeSafeClient.from_env(timeout=10, max_retries=1)
    objectives = load_objectives()
    objectives = objectives[:next(i for i, obj in enumerate(objectives) if obj['id'] == args.until)+1]
    result = {'success': False, 'target': args.until, 'seed': args.seed,
              'mode': 'autonomous-new-game', 'uses_milestone_handlers': False}
    policy_files = [*Path(__file__).parent.glob('*.py'),
                    pt.ROOT / 'scripts/playthrough.py', pt.ROOT / 'scripts/playthrough_late.py',
                    pt.ROOT / 'scripts/debug_drive.py']
    result['policy_files'] = {str(p.relative_to(pt.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in sorted(policy_files)}
    result['policy_sha256'] = hashlib.sha256(json.dumps(result['policy_files'], sort_keys=True).encode()).hexdigest()
    with tempfile.TemporaryDirectory(prefix='jev-autonomous-') as private:
        binary = Path(private) / 'pokered-app'
        shutil.copy2(args.binary, binary)
        saved = None
        parent = None
        if args.resume:
            parent = json.loads((args.resume / 'summary.json').read_text())
            if parent.get('mode') != 'autonomous-new-game':
                parser.error('resume requires an autonomous-run checkpoint')
            evidence = args.resume / 'final-observations.json'
            if not parent.get('development_checkpoint') or not observations_valid(json.loads(evidence.read_text())):
                parser.error('resume requires a checkpoint with valid final protocol observations')
            saved = Path(private) / 'continuation.sav'
            shutil.copy2(args.resume / 'game.sav', saved)
            extras = args.resume / 'game.script_flags.json'
            if extras.exists():
                shutil.copy2(extras, binary.parent / 'pokered.script_flags.json')
            result['resumed_from'] = str(args.resume)
        result['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        game = agent = None
        with (folder / 'trace.jsonl').open('w') as trace:
            try:
                game = JevGame(binary=binary, save_path=saved, seed=args.seed, speed=0)
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
                    max_calls=args.max_calls, max_actions=args.max_actions,
                    wall_budget=args.wall_budget, frame_budget=args.frame_budget, trace=trace)
                if parent:
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
                    agent.field_requirements.update(parent.get('preparation_requirements', {}))
                    agent.battle_requirements.update(parent.get('battle_requirements', {}))
                    game.stationary_npcs = {name: {int(k): tuple(v) for k, v in npcs.items()}
                                            for name, npcs in parent.get('stationary_npcs', {}).items()}
                    agent.battle_defeats.extend(parent.get('battle_defeats', []))
                    agent.defeat_preparation = parent.get('defeat_preparation', 0)
                # Finish the outstanding JSON round trip before stopping.
                # Raising KeyboardInterrupt inside readline would leave its
                # reply queued and misalign final observations / checkpoint.
                signal.signal(signal.SIGINT, lambda *_: setattr(game.d, 'stop_requested', True))
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
                            result['preparation_requirements'] = {
                                move: context for move, context in agent.field_requirements.items()
                                if move == 'Strength'}
                            result['battle_requirements'] = agent.battle_requirements
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
                        if not valid:
                            result.update(success=False, reason='invalid_final_protocol_observations')
                        if valid:
                            game.d.raw.cmd(cmd='capture_frame', path=str((folder / 'final.png').resolve()))
                        if args.checkpoint and valid:
                            reply = game.d.raw.cmd(cmd='save')
                            valid = bool(reply.get('ok')) and reply.get('data') is None
                            result['development_checkpoint'] = valid
                            if not valid:
                                result.update(success=False, reason='invalid_checkpoint_acknowledgement')
                            result['development_checkpoint_screen'] = observations['get_state']['data']['screen']
                        if valid and game.save_path.exists():
                            shutil.copy2(game.save_path, folder / 'game.sav')
                        flags = binary.parent / 'pokered.script_flags.json'
                        if valid and flags.exists():
                            shutil.copy2(flags, folder / 'game.script_flags.json')
                        result['commands'] = game.d.counts
                        result['battles_driven'] = game.battles_driven
                        result['action_cache_hits'] = game.move_cache_hits
                        game.log.flush()
                        shutil.copy2(game.run_dir / 'game.log', folder / 'game.log')
                    finally:
                        game.close()
    (folder / 'summary.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ('final_facts', 'commands', 'policy_files', 'first_clear_verification')}, ensure_ascii=False))
    print('Artifacts:', folder)
    return 0 if result['success'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
