#!/usr/bin/env python3
"""Streaming CPU chat comparison. Synthetic data; no real tools, capture or DB."""
import argparse,hashlib,json,platform,re,statistics,subprocess,threading,time,urllib.request
from pathlib import Path

def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  while b:=f.read(1024*1024):h.update(b)
 return h.hexdigest()
def proc_status(pid):
 try:
  lines=Path(f'/proc/{pid}/status').read_text().splitlines()
  return {key:int(next(x for x in lines if x.startswith(key+':')).split()[1]) for key in ['VmRSS','VmHWM']}
 except (OSError,StopIteration):return {'VmRSS':0,'VmHWM':0}
def percentile(xs,q):
 xs=sorted(x for x in xs if x is not None)
 return xs[min(len(xs)-1,int((len(xs)-1)*q))] if xs else None

def main():
 p=argparse.ArgumentParser();p.add_argument('--server',required=True);p.add_argument('--models',required=True);p.add_argument('--out',required=True);p.add_argument('--extra-manifest');p.add_argument('--only',default='');a=p.parse_args()
 out=Path(a.out)
 if out.exists() and any(out.iterdir()):p.error('--out must be empty')
 fixtures=Path(__file__).with_name('chat-cases.json');data=json.loads(fixtures.read_text());root=Path(__file__).resolve().parents[2]
 manifest=json.loads((root/'docs/research/mobile-model-candidates-2026-10-10.json').read_text())['candidates']
 models=[m for m in manifest if m['repository'] in ['LiquidAI/LFM2.5-350M-GGUF','lmstudio-community/Qwen3-0.6B-GGUF']]
 if a.extra_manifest:models.extend(m for m in json.loads(Path(a.extra_manifest).read_text())['candidates'] if not m['name'].startswith('mmproj'))
 models=[m for m in models if a.only in m['name']]
 unique={}
 for m in models:
  if m['name'] in unique and (unique[m['name']]['sha256']!=m['sha256'] or unique[m['name']]['bytes']!=m['bytes']):p.error('conflicting candidate: '+m['name'])
  unique[m['name']]=m
 models=list(unique.values())
 if not models:p.error('no matching model')
 out.mkdir(parents=True);(out/'fixtures.json').write_bytes(fixtures.read_bytes())
 params={'stream':True,'stream_options':{'include_usage':True},'max_tokens':192,'temperature':0.6,'top_p':0.9,'top_k':40,'min_p':0.0,'repeat_penalty':1.05,'seed':20261010,'cache_prompt':True,'chat_template_kwargs':{'enable_thinking':False}}
 config={'runtime':subprocess.check_output([a.server,'--version'],text=True,stderr=subprocess.STDOUT),'platform':platform.platform(),'cpu':subprocess.check_output(['lscpu'],text=True),'cpu_quota':Path('/sys/fs/cgroup/cpu.max').read_text().strip(),'memory_limit_bytes':Path('/sys/fs/cgroup/memory.max').read_text().strip(),'threads':2,'context_tokens':4096,'request_parameters':params,'models':models,'note':'Warm filesystem cache; health startup is not disk-cold. Repeated system prefix cache. Static synthetic histories plus four actual generated three-turn chains per model. Host reviewer; no automated semantic pass.'}
 (out/'configuration.json').write_text(json.dumps(config,ensure_ascii=False,indent=2)+'\n')
 (out/'source-hashes.json').write_text(json.dumps({x.name:sha(x) for x in [Path(__file__),fixtures,Path(__file__).with_name('chat-cases.py')]},indent=2)+'\n')
 base='http://127.0.0.1:18891'
 def get(path):
  with urllib.request.urlopen(base+path,timeout=3) as r:return json.load(r)
 def request(case,messages):
  body={'messages':messages,**params};req=urllib.request.Request(base+'/v1/chat/completions',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  start=time.perf_counter();reply='';reasoning='';first=None;sentence=None;events=[];error=None;finish=[];usage={}
  try:
   with urllib.request.urlopen(req,timeout=90) as r:
    for line in r:
     if not line.startswith(b'data: '):continue
     payload=line[6:].strip()
     if payload==b'[DONE]':break
     e=json.loads(payload);events.append(e)
     if e.get('usage'):usage=e['usage']
     for c in e.get('choices',[]):
      delta=c.get('delta',{});chunk=delta.get('content') or '';reasoning+=delta.get('reasoning_content') or ''
      if c.get('finish_reason'):finish.append(c['finish_reason'])
      if chunk:
       if first is None:first=time.perf_counter()-start
       reply+=chunk
       if sentence is None and re.search(r'[。！？!?\n]',reply):sentence=time.perf_counter()-start
  except Exception as ex:error=type(ex).__name__+': '+str(ex)[:300]
  return {**case,'request_messages':messages,'reply':reply,'reasoning':reasoning,'first_content_seconds':first,'first_sentence_seconds':sentence,'total_seconds':time.perf_counter()-start,'finish_reasons':finish,'usage':usage,'error':error,'events':events}
 summaries=[]
 for model in models:
  file=Path(a.models)/model['name'];assert file.stat().st_size==model['bytes'] and sha(file)==model['sha256'],'weight hash mismatch'
  d=out/file.stem;d.mkdir();log=(d/'server.txt').open('w');cmd=[a.server,'-m',str(file),'--host','127.0.0.1','--port','18891','-t','2','-tb','2','-c','4096','-np','1','-ngl','0','--jinja','--no-warmup','--no-mmproj']
  (d/'command.json').write_text(json.dumps(cmd,indent=2));process=subprocess.Popen(cmd,stdout=log,stderr=log);start=time.perf_counter();stop=threading.Event();peak={'VmRSS':0,'VmHWM':0}
  def sample():
   while not stop.wait(.05):
    for k,v in proc_status(process.pid).items():peak[k]=max(peak[k],v)
  monitor=threading.Thread(target=sample,daemon=True);monitor.start();records=[]
  try:
   while True:
    if process.poll() is not None:raise RuntimeError('server exited: '+str(d/'server.txt'))
    try:
     if get('/health').get('status')=='ok':break
    except Exception:pass
    if time.perf_counter()-start>90:raise TimeoutError('startup')
    time.sleep(.05)
   startup=time.perf_counter()-start;props=get('/props');(d/'props.json').write_text(json.dumps(props,ensure_ascii=False,indent=2))
   with (d/'responses.jsonl').open('w') as target:
    def save(r):
     records.append(r);target.write(json.dumps(r,ensure_ascii=False)+'\n');target.flush();print(file.stem,r['id'],round(r['total_seconds'],2),r['reply'][:100].replace('\n',' '),flush=True)
    for case in data['cases']:
     system=data['system']+('\n本次宿主提供的上下文：\n'+case['context'] if case['context'] else '')
     save(request(case,[{'role':'system','content':system}]+case['history']+[{'role':'user','content':case['user']}]))
    for chain in data['chains']:
     messages=[{'role':'system','content':data['system']}]
     for index,(user,criteria) in enumerate(chain['turns'],1):
      messages.append({'role':'user','content':user});case={'id':chain['id']+'_'+str(index),'category':chain['category'],'user':user,'criteria':criteria,'chain_generated_history':True}
      r=request(case,list(messages));save(r)
      if r['error']:break
      messages.append({'role':'assistant','content':r['reply']})
   summary={'model':model,'requests':len(records),'errors':sum(r['error'] is not None for r in records),'truncated':sum('length' in r['finish_reasons'] for r in records),'reasoning_nonempty':sum(bool(r['reasoning']) for r in records),'startup_health_seconds':startup,'peak_single_process_kib':peak,'final_rss_kib':proc_status(process.pid),'latency':{k:{'p50':percentile([r[k] for r in records],.5),'p95':percentile([r[k] for r in records],.95)} for k in ['first_content_seconds','first_sentence_seconds','total_seconds']},'semantic_score':None}
   (d/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n');summaries.append(summary)
  finally:
   process.terminate()
   try:process.wait(timeout=10)
   except subprocess.TimeoutExpired:process.kill();process.wait()
   stop.set();monitor.join();log.close();(d/'unload.json').write_text(json.dumps({'exited':process.poll() is not None,'exit_code':process.returncode,'peak_single_process_kib':peak})+'\n')
 (out/'summary.json').write_text(json.dumps(summaries,ensure_ascii=False,indent=2)+'\n')
if __name__=='__main__':main()
