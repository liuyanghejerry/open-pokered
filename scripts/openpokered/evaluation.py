"""Measurement contracts for fixed-budget autonomous model comparisons."""
import collections
import json
import math
import time
from pathlib import Path


class EvaluationStopped(BaseException):
    """Deadline control flow must bypass skills' ordinary recovery handlers."""


class EvaluationBudget:
    def __init__(self, seconds=1200, max_rtt_credit=300, clock=time.monotonic):
        if not math.isfinite(seconds) or not math.isfinite(max_rtt_credit) or seconds<=0 or max_rtt_credit<0:
            raise ValueError('invalid evaluation budget')
        self.seconds=seconds;self.max_rtt_credit=max_rtt_credit;self.clock=clock
        self.started=None;self.stopped=None;self.credit=0.;self.rtt_samples=[]

    def start(self):
        if self.started is not None:raise RuntimeError('budget already started')
        self.started=self.clock()

    def raw(self):
        end = self.clock() if self.stopped is None else self.stopped
        return 0. if self.started is None else end-self.started
    def effective(self):return max(0.,self.raw()-self.credit)
    def add_rtt(self, seconds, request_duration):
        if self.started is None or self.stopped is not None:return 0.
        if seconds is None or not math.isfinite(seconds) or seconds<0:return 0.
        credited=min(seconds,max(0.,request_duration),max(0.,self.max_rtt_credit-self.credit))
        self.credit+=credited;self.rtt_samples.append({'measured_s':seconds,'credited_s':credited})
        return credited

    def check(self):
        if self.started is not None and self.effective()>=self.seconds:
            raise EvaluationStopped('effective_time_budget')
        if self.raw()>=self.seconds+self.max_rtt_credit:
            raise EvaluationStopped('absolute_wall_budget')

    def finish(self):
        if self.stopped is None:self.stopped=self.clock()

    def snapshot(self):
        raw=self.raw()
        return {'start_monotonic':self.started,'raw_s':raw,'rtt_credit_s':self.credit,
                'effective_s':max(0.,raw-self.credit),'limit_s':self.seconds,'max_rtt_credit_s':self.max_rtt_credit}


# Predeclared optional coverage; these checks are observations, never planner goals.
SIDEQUESTS={
    'bicycle':{'label':'取得自行车','flags':['EVENT_GOT_BICYCLE']},
    'old_rod':{'label':'取得旧钓竿','flags':['EVENT_GOT_OLD_ROD']},
    'good_rod':{'label':'取得好钓竿','flags':['EVENT_GOT_GOOD_ROD']},
    'super_rod':{'label':'取得超级钓竿','items':['SuperRod']},
    'fly':{'label':'取得飞翔秘传机','items':['Hm02']},
    'flash':{'label':'取得闪光秘传机','items':['Hm05']},
    'dojo_gift':{'label':'格斗道场获赠精灵','flags':['EVENT_GOT_HITMONLEE','EVENT_GOT_HITMONCHAN']},
    'zapdos':{'label':'捕获闪电鸟','owned':[145]},
    'articuno':{'label':'捕获急冻鸟','owned':[144]},
    'moltres':{'label':'捕获火焰鸟','owned':[146]},
    'fossil_revived':{'label':'拥有复活化石精灵','owned':[138,140,142]},
}


def sidequests(flags,bag,owned):
    items={x['item'] for x in bag if x.get('qty',0)>0}
    return [key for key,spec in SIDEQUESTS.items() if any(flags.get(f,False) for f in spec.get('flags',[]))
            or any(i in items for i in spec.get('items',[])) or set(spec.get('owned',[]))&set(owned)]


def atomic_json(path,value):
    path=Path(path);tmp=path.with_suffix(path.suffix+'.next')
    tmp.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n');tmp.replace(path)


def controller_response(response):
    """Measurement-only fields must not change the controller's model inputs."""
    data=response.get('data')
    if not isinstance(data,dict):return response
    if 'screen' in data and 'evaluation' in data:
        return {**response,'data':{k:v for k,v in data.items() if k!='evaluation'}}
    if isinstance(data.get('state'),dict) and 'screen' in data['state'] and 'evaluation' in data['state']:
        return {**response,'data':{**data,'state':{k:v for k,v in data['state'].items() if k!='evaluation'}}}
    return response


class EvaluationMetrics:
    def __init__(self,objectives,stall_seconds=180):
        self.objectives=objectives;self.stall_seconds=stall_seconds
        self.timeline=[];self.milestones={};self.maps=set();self.side_completed={}
        self.money_initial=None;self.money_last=None;self.money_peak=0
        self.money_gained=0;self.money_lost=0;self.failure_counts=collections.Counter()
        self.last_progress_s=0.;self.progress=None;self.stall_active=False;self.stalls=[]
        self.current_battle_wipe=False
        self.ever_flags=set();self.ever_owned=set();self.max_seen=0;self.training_best={}

    def observe(self,state,flags,bag,clock):
        # Post-deadline final reads can diagnose the exit but cannot add score.
        if clock['effective_s']>clock['limit_s']:return None
        elapsed=clock['effective_s'];place=state['map_name']
        if state['screen'] in ('overworld','battle'):self.maps.add(place)
        telemetry=state.get('evaluation',{})
        party=[dict(m) for m in telemetry.get('party',state.get('party',[]))]
        money=state.get('money',0)
        if self.money_initial is None:self.money_initial=money
        if self.money_last is not None:
            delta=money-self.money_last;self.money_gained+=max(0,delta);self.money_lost+=max(0,-delta)
        self.money_last=money;self.money_peak=max(self.money_peak,money)
        dex=telemetry.get('pokedex',state.get('pokedex',{}));owned=dex.get('owned_numbers',[])
        for obj in self.objectives:
            if flags.get(obj['satisfied_when']['flag']) and obj['id'] not in self.milestones:
                self.milestones[obj['id']]={'effective_s':elapsed,'raw_s':clock['raw_s'],'frame':state['frame_count']}
        for name in sidequests(flags,bag,owned):self.side_completed.setdefault(name,elapsed)
        wiped=state['screen']=='battle' and bool(party) and all(m['hp']==0 for m in party)
        if wiped and not self.current_battle_wipe:self.failure_counts['observed_party_wipes']+=1
        self.current_battle_wipe=wiped
        # Healing, party reordering and toggling a transient flag must not hide a loop.
        self.ever_flags.update(k for k,v in flags.items() if v)
        self.ever_owned.update(owned);self.max_seen=max(self.max_seen,dex.get('seen',0))
        for mon in party:
            previous=self.training_best.get(mon['species'],(0,0))
            self.training_best[mon['species']]=max(previous,(mon['level'],mon.get('total_exp',0)))
        progress=(tuple(sorted(self.ever_flags)),tuple(sorted(self.maps)),tuple(sorted(self.ever_owned)),
                  self.max_seen,tuple(sorted(self.training_best.items())),tuple(sorted(self.side_completed)))
        if progress!=self.progress:
            if self.stall_active:self.stalls[-1]['ended_s']=elapsed
            self.progress=progress;self.last_progress_s=elapsed;self.stall_active=False
        elif elapsed-self.last_progress_s>=self.stall_seconds and not self.stall_active:
            self.stall_active=True;self.stalls.append({'started_s':self.last_progress_s,'detected_s':elapsed,'map':place})
        row={'effective_s':elapsed,'raw_s':clock['raw_s'],'rtt_credit_s':clock['rtt_credit_s'],
             'frame':state['frame_count'],'map':place,'position':[state['player_x'],state['player_y']],
             'screen':state['screen'],'badges':int(state.get('badges',0)).bit_count(),
             'completed_objectives':list(self.milestones),'sidequests':list(self.side_completed),
             'map_count':len(self.maps),'pokedex':dex,'party':party,'party_source':telemetry.get('party_source','fixture'),'money':money,
             'observed_money_gained':self.money_gained,'observed_money_lost':self.money_lost,
             'failures':dict(self.failure_counts),'suspected_stall':self.stall_active}
        self.timeline.append(row);return row

    def event(self,kind,payload):
        if kind=='battle_defeat':self.failure_counts['reported_battle_defeats']+=1
        if kind=='outcome' and payload.get('result') in ('blocked','failed','not_found','no_path'):
            self.failure_counts['unsuccessful_operations']+=1
            if payload.get('operation','').startswith('travel_to:'):
                self.failure_counts['blocked_travel_operations']+=1
        if kind=='action_rejected':self.failure_counts['rejected_actions']+=1
        if kind=='judgment_error':self.failure_counts['judgment_errors']+=1

    def summary(self):
        last=self.timeline[-1] if self.timeline else {}
        party=last.get('party',[])
        return {'final':last,'milestones':self.milestones,'objective_total':len(self.objectives),
                'maps':sorted(self.maps),'sidequests':self.side_completed,'sidequest_total':len(SIDEQUESTS),
                'party_levels':{'sum':sum(m['level'] for m in party),'max':max((m['level'] for m in party),default=0),
                                'mean':sum(m['level'] for m in party)/len(party) if party else 0},
                'money':{'initial':self.money_initial,'final':self.money_last,'peak':self.money_peak,
                         'observed_positive_deltas':self.money_gained,'observed_negative_deltas':self.money_lost,
                         'scope':'Sampled balance changes, not exact gross income/spending; multiple changes between observations can cancel.'},
                'failures':dict(self.failure_counts),'suspected_stalls':self.stalls,
                'stall_policy':'Report only; no backend-specific recovery. No new flags/maps/dex/experience/levels for 180 effective seconds.'}
