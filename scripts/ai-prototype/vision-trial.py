#!/usr/bin/env python3
"""Four synthetic vision probes; no actual desktop capture, OCR engine or tools."""
import argparse,base64,hashlib,json,subprocess,time,urllib.request
from pathlib import Path
from importlib.util import spec_from_file_location,module_from_spec
spec=spec_from_file_location('chat',Path(__file__).with_name('chat-trial.py'));chat=module_from_spec(spec);spec.loader.exec_module(chat)
p=argparse.ArgumentParser();p.add_argument('--server',required=True);p.add_argument('--models',required=True);p.add_argument('--manifest',required=True);p.add_argument('--fixtures',required=True);p.add_argument('--out',required=True);p.add_argument('--min-image-tokens',type=int,default=0);p.add_argument('--case-ids',default='');a=p.parse_args();out=Path(a.out)
if out.exists() and any(out.iterdir()):p.error('--out must be empty')
out.mkdir(parents=True);manifest=json.loads(Path(a.manifest).read_text());fixture=Path(a.fixtures);cases=json.loads((fixture/'cases.json').read_text());system=json.loads(Path(__file__).with_name('chat-cases.json').read_text())['system']
if a.min_image_tokens < 0 or a.min_image_tokens > 1024:p.error('min-image-tokens must be 0..1024')
if a.case_ids:cases=[c for c in cases if c['id'] in a.case_ids.split(',')]
if not cases:p.error('no cases selected')
for m in manifest['candidates']:
 file=Path(a.models)/m['name'];assert file.stat().st_size==m['bytes'] and chat.sha(file)==m['sha256']
model=next(m for m in manifest['candidates'] if not m['name'].startswith('mmproj'));proj=next(m for m in manifest['candidates'] if m['name'].startswith('mmproj'))
cmd=[a.server,'-m',str(Path(a.models)/model['name']),'--mmproj',str(Path(a.models)/proj['name']),'--no-mmproj-offload','--host','127.0.0.1','--port','18891','-t','2','-tb','2','-c','4096','-np','1','-ngl','0','--jinja','--no-warmup','--image-max-tokens','1024']
if a.min_image_tokens:cmd+=['--image-min-tokens',str(a.min_image_tokens)]
params={'stream':True,'stream_options':{'include_usage':True},'max_tokens':192,'temperature':0.6,'top_p':0.9,'top_k':40,'min_p':0.0,'repeat_penalty':1.05,'seed':20261010,'cache_prompt':True,'chat_template_kwargs':{'enable_thinking':False}}
(out/'configuration.json').write_text(json.dumps({'command':cmd,'parameters':params,'manifest':manifest,'cases':cases,'system':system,'note':'Synthetic 960x540 PNGs, max image tokens 1024; minimum and selected probes are in command/cases; no real capture/OCR, no privacy/permission integration. Same shared CPU as chat.'},ensure_ascii=False,indent=2)+'\n')
(out/'source-hashes.json').write_text(json.dumps({x.name:chat.sha(x) for x in [Path(__file__),Path(__file__).with_name('chat-trial.py'),Path(__file__).with_name('make-vision-cases.py')]},indent=2)+'\n')
log=(out/'server.txt').open('w');process=subprocess.Popen(cmd,stdout=log,stderr=log);base='http://127.0.0.1:18891';records=[];started=time.perf_counter()
try:
 while True:
  if process.poll() is not None:raise RuntimeError('server exited; inspect '+str(out/'server.txt'))
  try:
   with urllib.request.urlopen(base+'/health',timeout=3) as r:
    if json.load(r).get('status')=='ok':break
  except Exception:pass
  if time.perf_counter()-started>90:raise TimeoutError('startup')
  time.sleep(.05)
 startup=time.perf_counter()-started
 with urllib.request.urlopen(base+'/props',timeout=3) as r:(out/'props.json').write_text(json.dumps(json.load(r),ensure_ascii=False,indent=2))
 with (out/'responses.jsonl').open('w') as target:
  for case in cases:
   image=(fixture/case['image']).read_bytes();body={'messages':[{'role':'system','content':system},{'role':'user','content':[{'type':'text','text':case['user']},{'type':'image_url','image_url':{'url':'data:image/png;base64,'+base64.b64encode(image).decode()}}]}],**params}
   req=urllib.request.Request(base+'/v1/chat/completions',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'});start=time.perf_counter();events=[];reply='';reasoning='';first=None;finish=[];usage={};error=None
   try:
    with urllib.request.urlopen(req,timeout=120) as r:
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
   except Exception as ex:error=type(ex).__name__+': '+str(ex)[:400]
   record={**case,'image_sha256':hashlib.sha256(image).hexdigest(),'reply':reply,'reasoning':reasoning,'first_content_seconds':first,'total_seconds':time.perf_counter()-start,'finish_reasons':finish,'usage':usage,'error':error,'events':events}
   target.write(json.dumps(record,ensure_ascii=False)+'\n');target.flush();records.append(record);print(case['id'],record['total_seconds'],reply,flush=True)
 summary={'requests':len(records),'errors':sum(bool(r['error']) for r in records),'startup_health_seconds':startup,'single_process_final_status_kib':chat.proc_status(process.pid),'latency':{key:{'p50':chat.percentile([r[key] for r in records],.5),'p95':chat.percentile([r[key] for r in records],.95)} for key in ['first_content_seconds','total_seconds']},'semantic_score':None}
 (out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
finally:
 process.terminate()
 try:process.wait(timeout=10)
 except subprocess.TimeoutExpired:process.kill();process.wait()
 log.close();(out/'unload.json').write_text(json.dumps({'exited':process.poll() is not None,'exit_code':process.returncode})+'\n')
