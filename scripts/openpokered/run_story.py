#!/usr/bin/env python3
"""Run two-layer Jev story exploration with real inputs and verified milestones.

python3 scripts/openpokered/run_story.py --until get-pokedex
python3 scripts/openpokered/run_story.py --until beat-brock --assisted

Default starts a fresh overworld in Pallet Town with no party or seeded flags.
--assisted adds a level-100 Charizard at setup to isolate story/navigation from
combat difficulty. Neither mode is a complete power-on/new-game benchmark.
"""
import argparse
import hashlib
import json
import subprocess
import shutil
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from openpokered.env import OpenPokeredEnv
from openpokered.judgment_agent import load_objectives
from openpokered.story_agent import DualStoryAgent
from openpokered.typesafe import TypeSafeClient


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--until', default='get-pokedex', choices=[o['id'] for o in load_objectives()])
    p.add_argument('--seed', type=int, default=42)
    p.add_argument('--runs', type=int, default=1)
    p.add_argument('--binary')
    p.add_argument('--maps-dir')
    p.add_argument('--model', default='jev-1.13.0')
    p.add_argument('--assisted', action='store_true')
    p.add_argument('--strategy', choices=['jev','code'], default='jev')
    p.add_argument('--action', choices=['jev','code'], default='jev')
    p.add_argument('--max-calls', type=int, default=160)
    p.add_argument('--max-actions', type=int, default=180)
    p.add_argument('--frame-budget', type=int, default=80000)
    p.add_argument('--wall-budget', type=float, default=600)
    p.add_argument('--output', type=Path, default=Path('target/agent/runs/story'))
    args = p.parse_args(argv)
    if args.maps_dir:
        p.error('alternate maps are not supported yet: planning ASTs come from the compiled game')
    if args.runs < 1 or min(args.max_calls,args.max_actions,args.frame_budget,args.wall_budget) <= 0:
        p.error('runs and budgets must be positive')
    objectives = load_objectives()
    objectives = objectives[:next(i for i,o in enumerate(objectives) if o['id']==args.until)+1]
    client = TypeSafeClient.from_env(timeout=10,max_retries=1) if 'jev' in (args.strategy,args.action) else None
    policy_files = sorted(Path(__file__).parent.glob('*.py'))
    policy_hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in policy_files}
    policy_sha256 = hashlib.sha256(json.dumps(policy_hashes, sort_keys=True).encode()).hexdigest()
    stamp = time.strftime('%Y%m%d-%H%M%S')
    folder = args.output / f'{stamp}-{args.strategy}-{args.action}-seed{args.seed}'
    folder.mkdir(parents=True,exist_ok=False)
    all_ok=True
    for repeat in range(args.runs):
        result = None
        env = OpenPokeredEnv(binary=args.binary, maps_dir=args.maps_dir,speed=0)
        # Native dynamic script flags live beside the executable even when
        # --save points elsewhere. Give every repeat its own executable dir
        # so NPC visibility cannot leak from another run or a user's game.
        private_binary_dir = Path(tempfile.mkdtemp(prefix='pokered-story-bin-'))
        shutil.copy2(env.binary, private_binary_dir / 'pokered-app')
        env.binary = private_binary_dir / 'pokered-app'
        task = {'id':'dual-story','name':'Explore the early story','seed':args.seed,
                'initial_state':{'warp':'PalletTown,10,6'},
                'setup':{'party':[{'species':'Charizard','level':100}]} if args.assisted else {},
                'goal':{'type':'flag','id':objectives[-1]['satisfied_when']['flag']},
                'max_steps':args.max_actions}
        with (folder/f'run-{repeat}.jsonl').open('w') as trace:
            try:
                env.reset(task)
                agent = DualStoryAgent(env.client,client,objectives,model=args.model,
                                       strategy_jev=args.strategy=='jev',action_jev=args.action=='jev',
                                       max_calls=args.max_calls,max_actions=args.max_actions,
                                       frame_budget=args.frame_budget,wall_budget=args.wall_budget,
                                       maps_dir=args.maps_dir,trace=trace)
                result = agent.run()
                result.update({'task':task,'repeat':repeat,'requested_model':args.model,
                               'strategy':args.strategy,'action':args.action,'assisted':args.assisted,
                               'policy_sha256':policy_sha256,'policy_files':policy_hashes,
                               'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
                               'binary_sha256':hashlib.sha256(env.binary.read_bytes()).hexdigest()})
                env.client.cmd(cmd='save')
                for saved in env.run_dir.glob('run.sav*'):
                    shutil.copy2(saved,folder/(f'run-{repeat}' + saved.name[3:]))
                flags_file = private_binary_dir / 'pokered.script_flags.json'
                if flags_file.exists():
                    shutil.copy2(flags_file, folder / f'run-{repeat}.script_flags.json')
                env.client.cmd(cmd='capture_frame',path=str((folder/f'run-{repeat}.png').resolve()))
            except Exception as e:
                # Keep measured progress if artifact capture failed, and
                # finish the remaining requested repetitions after failures.
                result = result or {'task': task}
                result.update(success=False, reason=f'{type(e).__name__}: {e}')
            finally:
                env.close()
                shutil.rmtree(private_binary_dir)
                if result is not None:
                    (folder/f'run-{repeat}-summary.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
        all_ok &= result['success']
        print(json.dumps({k:v for k,v in result.items() if k not in ('final_facts','task','policy_files')},ensure_ascii=False),flush=True)
    print('Artifacts:',folder,flush=True)
    return 0 if all_ok else 1


if __name__=='__main__':
    raise SystemExit(main())
