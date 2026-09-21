#!/usr/bin/env python3
"""Run one supervised, fresh 20-minute autonomous model evaluation."""
import argparse
import hashlib
import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
from openpokered.autonomous_story import AutonomousStoryAgent
from openpokered.evaluation import EvaluationBudget, EvaluationMetrics, EvaluationStopped, SIDEQUESTS, atomic_json, controller_response
from openpokered.evaluation_models import MeasuredModel
from openpokered.judgment_agent import load_objectives
from openpokered.playthrough_judgments import JevGame
from openpokered.run_autonomous import boot_new_game
from openpokered.typesafe import Choice, load_env_file
import playthrough as pt


def sha(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda:f.read(1048576),b''):digest.update(block)
    return digest.hexdigest()


class Observations:
    def __init__(self,game,budget,metrics,stream,commands_stream,status):
        self.game=game;self.raw=game.d.raw;self.budget=budget;self.metrics=metrics
        self.stream=stream;self.commands_stream=commands_stream;self.status=status;self.last=-1.;self.commands={}
        self.original=game.d.cmd
        game.d.cmd=self.command

    def command(self,**request):
        self.budget.check()
        name=request['cmd'];self.commands[name]=self.commands.get(name,0)+1
        record={'request':request,'start_clock':self.budget.snapshot()}
        try:
            response=self.original(**request);record['ok']=response.get('ok')
        except BaseException as error:
            record['error']=f'{type(error).__name__}: {error}';raise
        finally:
            record['end_clock']=self.budget.snapshot()
            self.commands_stream.write(json.dumps(record,ensure_ascii=False)+'\n');self.commands_stream.flush()
        if not response.get('ok') and self.budget.effective()<=self.budget.seconds:
            self.metrics.failure_counts['protocol_errors']+=1
        if self.budget.raw()-self.last>=.5:
            payload=response.get('data')
            state=payload if isinstance(payload,dict) and 'screen' in payload else None
            if state is None and isinstance(payload,dict):state=payload.get('state')
            self.capture(state)
        self.budget.check()
        return controller_response(response)

    def capture(self,state=None):
        state=state or self.raw.cmd(cmd='get_state')['data']
        flags=self.raw.cmd(cmd='get_flags')['data'];bag=self.raw.cmd(cmd='get_bag')['data']
        if 'evaluation' not in state:raise RuntimeError('Evaluation requires a binary with read-only evaluation telemetry')
        clock=self.budget.snapshot();row=self.metrics.observe(state,flags,bag,clock)
        if row is not None:self.stream.write(json.dumps(row,ensure_ascii=False)+'\n');self.stream.flush()
        self.last=self.budget.raw();self.status()
        return {'state':state,'flags':flags,'bag':bag,'clock':clock,'scored':row is not None}


class Trace:
    def __init__(self,stream,budget,metrics):self.stream=stream;self.budget=budget;self.metrics=metrics
    def write(self,text):
        result=self.stream.write(text)
        if self.budget.effective()<=self.budget.seconds:
            row=json.loads(text);self.metrics.event(row['kind'],row)
        return result
    def flush(self):self.stream.flush()


def worker(args):
    out=args.output;out.mkdir(parents=True,exist_ok=True)
    temporary=out/'temporary';temporary.mkdir(exist_ok=True);tempfile.tempdir=str(temporary)
    if args.env_file:load_env_file(args.env_file)
    objectives=load_objectives();metrics=EvaluationMetrics(objectives)
    config=json.loads(args.model_config.read_text()) if args.model_config else None
    network=config['provider']=='typesafe' if config else args.backend=='jev'
    budget=EvaluationBudget(args.seconds,args.max_rtt_credit if network else 0)
    result={'backend':args.backend,'seed':args.seed,'profile':'native-backend-replacement',
            'layers':{'strategy':args.backend,'action':args.backend},
            'limits':{'calls':50000,'actions':50000,'frames':40000000},
            'target':'become-champion','boot':'fresh NEW GAME','clock_mode':'driven-only','seconds':args.seconds,
            'record_video':False,'sidequest_catalog':SIDEQUESTS,'controller_uses_milestones':False,
            'telemetry_visibility':'Evaluator only; stripped from controller responses to preserve model inputs.',
            'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=pt.ROOT,text=True).strip(),
            'source_branch':subprocess.check_output(['git','branch','--show-current'],cwd=pt.ROOT,text=True).strip(),
            'binary_sha256':sha(args.binary),'python':sys.version,'platform':platform.platform(),
            'sampling_interval_s':.5,'setup_started_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())}
    result['host_load_at_setup']=os.getloadavg()
    if args.run_metadata:result['benchmark']=json.loads(args.run_metadata.read_text())
    if sys.platform=='darwin':
        result['hardware']={key:subprocess.check_output(['sysctl','-n',key],text=True).strip()
                            for key in ('machdep.cpu.brand_string','hw.memsize')}
    policies=[*Path(__file__).parent.glob('*.py'),pt.ROOT/'scripts/playthrough.py',pt.ROOT/'scripts/playthrough_late.py',
              pt.ROOT/'scripts/debug_drive.py']
    result['source_files']={str(p.relative_to(pt.ROOT)):sha(p) for p in sorted(policies)}
    result['source_sha256']=hashlib.sha256(json.dumps(result['source_files'],sort_keys=True).encode()).hexdigest()
    model=game=observer=agent=None;reason='';success=False;deadline_signal=[False]
    def status():
        atomic_json(out/'progress.json',{'backend':args.backend,'pid':os.getpid(),'game_pid':game.proc.pid if game else None,
            'clock':budget.snapshot(),'model_calls':len([r for r in model.records if not r['warmup']]) if model else 0,
            'last':metrics.timeline[-1] if metrics.timeline else None})
        checkpoint={**result,'completed':False,'reason':'running','clock':budget.snapshot(),
                    'metrics':metrics.summary(),'model':model.summary() if model else None}
        if agent:checkpoint.update(actions=agent.actions,action_cache_hits=game.move_cache_hits)
        atomic_json(out/'checkpoint.json',checkpoint)
    status();setup=time.monotonic()
    with (out/'requests.jsonl').open('w') as requests,(out/'trace.jsonl').open('w') as trace_file,(out/'observations.jsonl').open('w') as states,(out/'commands.jsonl').open('w') as commands:
        try:
            model=MeasuredModel(args.backend,budget,requests,status,config=config)
            model.system_one({'purpose':'Warm up the decision adapter before the timed game.'},
                             {'warmup':Choice('Which label describes this preparation?',{'warmup':'Prepare for a timed evaluation','gameplay':'Already playing the game'})})
            runtime=temporary/'runtime';runtime.mkdir();binary=runtime/'pokered-app';shutil.copy2(args.binary,binary)
            game=JevGame(binary=binary,seed=args.seed,speed=0)
            raw_timeout=min(10.,args.seconds+1);game.d.sock.settimeout(raw_timeout)
            trace=Trace(trace_file,budget,metrics)
            game.attach_judgments(model,model=model.model,trace=trace,max_calls=50000,
                                  wall_budget=args.seconds+args.max_rtt_credit+60,frame_budget=40000000)
            budget.start();result['setup_s']=time.monotonic()-setup
            result['timed_started_utc']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())
            observer=Observations(game,budget,metrics,states,commands,status);observer.capture()
            def stop_at_boundary(*_):
                deadline_signal[0]=True;game.d.stop_requested=True
            signal.signal(signal.SIGINT,stop_at_boundary)
            initial=boot_new_game(game);game.judgments.record('new_game',state=initial)
            agent=AutonomousStoryAgent(game.judgments.client,model,objectives,game=game,model=model.model,
                max_calls=50000,max_actions=50000,wall_budget=args.seconds+args.max_rtt_credit+60,
                frame_budget=40000000,trace=trace)
            original_check=agent.check_budget
            def check():budget.check();original_check()
            agent.check_budget=check
            run=agent.run();success=run['success'];reason='completed' if success else run['reason']
        except EvaluationStopped as error:reason=str(error)
        except BaseException as error:
            reason=f'{type(error).__name__}: {error}'
            (out/'failure.txt').write_text(traceback.format_exc())
        finally:
            if observer and budget.effective()<args.seconds:
                try:observer.capture()
                except BaseException as error:result['last_scored_observation_error']=str(error)
            budget.finish()
            result['host_load_at_finish']=os.getloadavg()
            result['deadline_overrun_s']=max(0.,budget.effective()-args.seconds)
            if not success and budget.effective()>=args.seconds:
                result['exit_detail']=reason;reason='effective_time_budget'
            elif not success and deadline_signal[0]:
                # A pending response can grant its RTT after the supervisor's
                # deadline signal, leaving a small unused fraction of a second.
                result['exit_detail']=reason;reason='supervisor_deadline'
            result['deadline_signal_received']=deadline_signal[0]
            result.update(reason=reason,completed=success,clock=budget.snapshot(),metrics=metrics.summary())
            if model:result['model']=model.summary()
            if agent:
                result.update(actions=agent.actions,action_cache_hits=game.move_cache_hits,
                              first_clear_verification=agent.first_clear_verification)
            # Preserve scores even if final diagnostic reads or shutdown hang.
            atomic_json(out/'summary.json',result)
            if game:
                try:
                    raw=game.d.raw
                    # Final reads are diagnostic only; scoring uses the last in-budget observation.
                    final={cmd:raw.cmd(cmd=cmd) for cmd in ('get_state','get_flags','get_bag')}
                    atomic_json(out/'final-observations.json',final)
                    raw.cmd(cmd='capture_frame',path=str((out/'final.png').resolve()))
                    result['commands']=observer.commands if observer else {}
                    game.log.flush();shutil.copy2(game.run_dir/'game.log',out/'game.log')
                except Exception as error:result['final_observation_error']=str(error)
                finally:game.close()
            if model:model.close()
            atomic_json(out/'summary.json',result);status()
    shutil.rmtree(temporary)
    print(json.dumps({'backend':args.backend,'reason':reason,'clock':result['clock'],'completed_objectives':list(metrics.milestones)},ensure_ascii=False),flush=True)
    return 0 if reason in ('completed','effective_time_budget','absolute_wall_budget','supervisor_deadline') else 1


def supervise(args):
    args.output.mkdir(parents=True,exist_ok=False)
    command=[sys.executable,str(Path(__file__).resolve()),'--worker',*sys.argv[1:]]
    launched=time.monotonic();interrupted=None;terminated=None
    with (args.output/'console.log').open('w') as log:
        process=subprocess.Popen(command,cwd=pt.ROOT,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            while process.poll() is None:
                now=time.monotonic();file=args.output/'progress.json';progress=json.loads(file.read_text()) if file.exists() else {}
                clock=progress.get('clock',{});started=clock.get('start_monotonic')
                expired=(now-started-clock.get('rtt_credit_s',0)>=args.seconds) if started is not None else now-launched>=180
                if expired and interrupted is None:
                    # Keep the game alive so the worker can finish an in-flight RPC
                    # and read final evidence. Only forced termination kills the group.
                    try:os.kill(process.pid,signal.SIGINT)
                    except ProcessLookupError:break
                    interrupted=now
                if interrupted is not None and now-interrupted>=8 and terminated is None:
                    try:os.killpg(process.pid,signal.SIGTERM)
                    except ProcessLookupError:break
                    terminated=now
                if terminated is not None and now-terminated>=2:
                    try:os.killpg(process.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                    break
                time.sleep(.2)
            process.wait()
        finally:
            try:os.killpg(process.pid,signal.SIGTERM)
            except ProcessLookupError:pass
            try:process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                try:os.killpg(process.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                process.wait()
    atomic_json(args.output/'supervisor.json',{'exit_code':process.returncode,'interrupted':interrupted is not None,
        'forced_termination':terminated is not None,'wall_s':time.monotonic()-launched,
        'grace_policy':'SIGINT at the effective deadline; terminate entire private process group after 8s, kill after another 2s.'})
    if not (args.output/'summary.json').exists():
        file=args.output/'checkpoint.json';checkpoint=json.loads(file.read_text()) if file.exists() else {}
        if args.run_metadata:checkpoint['benchmark']=json.loads(args.run_metadata.read_text())
        checkpoint.update(backend=args.backend,completed=False,reason='watchdog_termination',seed=args.seed,
            warning='Worker did not finish. Scores/costs are from the last checkpoint; a pending request may have unreported usage. Shutdown grace is never scored.')
        atomic_json(args.output/'summary.json',checkpoint)
    print(str(args.output/'summary.json'))
    return process.returncode


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('backend');parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True);parser.add_argument('--seconds',type=float,default=1200)
    parser.add_argument('--max-rtt-credit',type=float,default=300);parser.add_argument('--seed',type=int,default=42)
    parser.add_argument('--env-file',type=Path);parser.add_argument('--worker',action='store_true',help=argparse.SUPPRESS)
    parser.add_argument('--model-config',type=Path,help='Versioned benchmark model configuration; otherwise use legacy jev/laya defaults')
    parser.add_argument('--run-metadata',type=Path,help='Frozen benchmark job metadata')
    args=parser.parse_args();args.output=args.output.resolve();args.binary=args.binary.resolve()
    EvaluationBudget(args.seconds,args.max_rtt_credit)
    if args.model_config:
        from openpokered.benchmark_models import validate_model
        args.model_config=args.model_config.resolve();validate_model(json.loads(args.model_config.read_text()))
    elif args.backend not in ('jev','laya'):parser.error('A custom backend requires --model-config')
    if args.run_metadata:args.run_metadata=args.run_metadata.resolve()
    return worker(args) if args.worker else supervise(args)

if __name__=='__main__':raise SystemExit(main())
