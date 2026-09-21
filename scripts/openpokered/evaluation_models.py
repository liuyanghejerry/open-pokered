"""Native Laya / Jev adapters with explicit token, truncation and RTT evidence."""
import hashlib
import json
import math
import socket
import statistics
import struct
import sys
import time
import urllib.error
import urllib.request

from .typesafe import SystemOneResult, TypeSafeClient, _parse_answer

LAYA_MODEL='aac6fef/laya-mlx'
LAYA_REVISION='20aed815fc6acde75733882e7ec0e3f28aeb9717'


def tcp_rtt_seconds(response, platform=None):
    """Darwin SDK tcp_connection_info.tcpi_rttcur: offset 40, uint32 ms.

    Read the actual HTTPS socket, not request duration (which includes inference).
    Unsupported or inaccessible transports receive zero credit, not an estimate.
    """
    if (platform or sys.platform)!='darwin':return None
    try:
        sock=response.fp.raw._sock
        info=sock.getsockopt(socket.IPPROTO_TCP,0x106,104)
        if len(info)<44:return None
        return struct.unpack_from('=I',info,40)[0]/1000
    except (AttributeError,OSError,ValueError):return None


class RttOpener:
    def __init__(self):self.samples=[]
    def __call__(self,request,timeout):
        start=time.monotonic()
        try:response=urllib.request.urlopen(request,timeout=timeout)
        except urllib.error.HTTPError as error:
            self.samples.append({'rtt_s':tcp_rtt_seconds(error),'headers_wall_s':time.monotonic()-start,'http_status':error.code})
            raise
        except (urllib.error.URLError,OSError) as error:
            self.samples.append({'rtt_s':None,'headers_wall_s':time.monotonic()-start,
                                 'http_status':None,'transport_error':type(error).__name__})
            raise
        self.samples.append({'rtt_s':tcp_rtt_seconds(response),'headers_wall_s':time.monotonic()-start,'http_status':response.status})
        return response


class LayaClient:
    def __init__(self,checkpoint=LAYA_MODEL,revision=LAYA_REVISION, *, dtype='float16', compile=True,
                 pad_to_multiple=16, cache_prompts=True):
        import importlib.metadata
        import laya_mlx
        self.agent=laya_mlx.load(checkpoint,revision=revision,dtype=dtype,compile=compile,
                                pad_to_multiple=pad_to_multiple,cache_prompts=cache_prompts)
        self.model=checkpoint+'@'+revision
        self.metadata={'package':'laya-mlx','version':importlib.metadata.version('laya-mlx'),
                       'checkpoint':checkpoint,'revision':revision,'dtype':dtype,'compile':compile,
                       'pad_to_multiple':pad_to_multiple,'cache_prompts':cache_prompts,'max_len':self.agent.cfg['max_len'],
                       'head_max_len':self.agent.cfg['head_max_len'],'token_scope':'Actual encoded tokens after native truncation; output_tokens=0 (no text generation).'}
        self.last_audit=[]

    def system_one(self,state,questions,model=None):
        from laya_mlx.common import build_prefix, render_options, serialize_state
        defs={k:q.to_json() for k,q in questions.items()}
        prepared,internal=self.agent.prepare(state,defs)
        state_ids=self.agent.tok(serialize_state(state).replace(self.agent.tok.mask_token,' '))['input_ids']
        self.last_audit=[]
        for qid,item,q in zip(defs,prepared,internal):
            prefix,markers=build_prefix(self.agent.tok,q,self.agent.cfg['head_max_len'])
            used=max(0,len(item['ids'])-len(prefix)-1)
            spans=[len(prefix[markers[i]:markers[i+1]])-1 if i+1<len(markers) else len(prefix[markers[i]:])-2 for i in range(len(markers))]
            options=render_options(q)
            option_full=[len(self.agent.tok(' '+o.replace(self.agent.tok.mask_token,' '))['input_ids']) for o in options]
            head_full=len(self.agent.tok('%s question: %s' % (q['t'],str(q['ins']).replace(self.agent.tok.mask_token,' ')))['input_ids'])
            head_used=markers[0]-2
            self.last_audit.append({'question':qid,'state_tokens_before':len(state_ids),'state_tokens_used':used,
                                   'state_tokens_dropped':max(0,len(state_ids)-used),'encoded_tokens':len(item['ids']),
                                   'candidate_tokens_before':option_full,'candidate_tokens_used':spans,
                                   'candidate_tokens_dropped':[max(0,a-b) for a,b in zip(option_full,spans)],
                                   'instruction_tokens_before':head_full,'instruction_tokens_used':head_used,
                                   'instruction_tokens_dropped':max(0,head_full-head_used),
                                   'effective_input_text':self.agent.tok.backend.decode(item['ids'],skip_special_tokens=False)})
        data=self.agent.predict(state,defs)
        expected=sum(len(i['ids']) for i in prepared)
        if data['usage']['input_tokens']!=expected:raise RuntimeError('Laya prepared/actual token count mismatch')
        return SystemOneResult(self.model,{k:_parse_answer(v) for k,v in data['answers'].items()},expected,0)


class MeasuredModel:
    def __init__(self,backend,budget,journal,status_callback=lambda:None, *, config=None):
        self.backend=backend;self.budget=budget;self.journal=journal;self.status_callback=status_callback
        self.records=[];self.opener=None
        self.validate_output=config is not None
        if config is not None:
            from .benchmark_models import create_adapter
            self.inner,self.opener=create_adapter(config)
            self.model=self.inner.model;self.metadata=self.inner.metadata
        elif backend=='jev':
            self.opener=RttOpener();self.inner=TypeSafeClient.from_env(timeout=10,max_retries=1,opener=self.opener)
            self.model='jev-1.13.0';self.metadata={'model':self.model,'token_scope':'TypeSafe service-reported usage; tokenizer is provider-specific.'}
        elif backend=='laya':
            self.inner=LayaClient();self.model=self.inner.model;self.metadata=self.inner.metadata
        else:raise ValueError(f'Unknown backend {backend!r}; provide a model configuration')

    def system_one(self,state,questions,model=None):
        self.budget.check();start=time.monotonic();sample_start=len(self.opener.samples) if self.opener else 0
        record={'number':len(self.records)+1,'backend':self.backend,'warmup':self.budget.started is None,'success':False,
                'started_clock':self.budget.snapshot(),'state':state,'questions':{k:v.to_json() for k,v in questions.items()},
                'question_ids':list(questions),'request_sha256':hashlib.sha256(json.dumps({'state':state,'questions':{k:v.to_json() for k,v in questions.items()}},ensure_ascii=False).encode()).hexdigest(),
                'input_json_bytes':len(json.dumps({'state':state,'questions':{k:v.to_json() for k,v in questions.items()}},ensure_ascii=False).encode())}
        try:
            result=self.inner.system_one(state,questions,model=self.model)
            usage=getattr(self.inner,'last_usage',None)
            record.update(model=result.model,input_tokens=usage.get('input_tokens') if usage is not None else result.input_tokens,
                          output_tokens=usage.get('output_tokens') if usage is not None else result.output_tokens,
                          answers={k:vars(v) for k,v in result.answers.items()},success=True)
            if hasattr(self.inner,'last_audit'):record['encoding']=self.inner.last_audit
            if getattr(self,'validate_output',False):
                from .benchmark_models import validate_answers
                from .typesafe import ChoiceAnswer, NoulAnswer, ScoreAnswer
                kinds={ChoiceAnswer:'choice',NoulAnswer:'noul',ScoreAnswer:'score'}
                validate_answers({k:{'type':kinds.get(type(v)),**vars(v)} for k,v in result.answers.items()},questions)
        except BaseException as error:
            record.update(success=False,error=f'{type(error).__name__}: {error}')
            usage=getattr(self.inner,'last_usage',None)
            if usage is not None:record.update(usage)
            raise
        finally:
            duration=time.monotonic()-start;record['latency_s']=duration
            samples=self.opener.samples[sample_start:] if self.opener else []
            record['transport_samples']=samples;record['rtt_credit_s']=sum(self.budget.add_rtt(s['rtt_s'],s['headers_wall_s']) for s in samples)
            record['clock']=self.budget.snapshot();self.records.append(record)
            self.journal.write(json.dumps(record,ensure_ascii=False)+'\n');self.journal.flush();self.status_callback()
        # A completed late judgment is cost evidence but cannot drive a late action.
        self.budget.check()
        return result

    def close(self):
        close=getattr(self.inner,'close',None)
        if close:close()

    def summary(self):
        records=[r for r in self.records if not r['warmup']]
        encoding=[e for r in records for e in r.get('encoding',[])]
        latencies=sorted(r['latency_s'] for r in records)
        return {'backend':self.backend,'metadata':self.metadata,'calls':len(records),'successful_calls':sum(r['success'] for r in records),
                'actual_models':sorted({r['model'] for r in records if 'model' in r}),
                'usage_missing_calls':sum(r.get('input_tokens') is None or r.get('output_tokens') is None for r in records),
                'input_tokens':sum(r.get('input_tokens') or 0 for r in records),'output_tokens':sum(r.get('output_tokens') or 0 for r in records),
                'latency_sum_s':sum(latencies),'latency_median_s':statistics.median(latencies) if latencies else None,
                'latency_p95_s':latencies[min(len(latencies)-1,math.ceil(len(latencies)*.95)-1)] if latencies else None,
                'rtt_measurements':sum(s['rtt_s'] is not None for r in records for s in r['transport_samples']),
                'encoding':{'questions':len(encoding),'state_tokens_before':sum(e['state_tokens_before'] for e in encoding),
                            'state_tokens_dropped':sum(e['state_tokens_dropped'] for e in encoding),
                            'questions_with_state_truncation':sum(e['state_tokens_dropped']>0 for e in encoding),
                            'candidate_tokens_dropped':sum(sum(e['candidate_tokens_dropped']) for e in encoding),
                            'instruction_tokens_dropped':sum(e['instruction_tokens_dropped'] for e in encoding)},
                'warmup':{ 'calls':sum(r['warmup'] for r in self.records),'latency_s':sum(r['latency_s'] for r in self.records if r['warmup']),
                           'input_tokens':sum(r.get('input_tokens') or 0 for r in self.records if r['warmup'])}}
