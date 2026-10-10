#!/usr/bin/env python3
"""Linux loopback prototype: idle CPU, stream disconnect, recovery and exit."""
import argparse,json,os,subprocess,time,urllib.request
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--server',required=True);p.add_argument('--model',required=True);p.add_argument('--out',required=True);args=p.parse_args()
out=Path(args.out);out.mkdir(parents=True,exist_ok=True)
base='http://127.0.0.1:18891'
def get(path):return json.load(urllib.request.urlopen(base+path,timeout=3))
def cpu(pid):
 values=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
 return (int(values[11])+int(values[12]))/os.sysconf('SC_CLK_TCK')
def request(body):return urllib.request.urlopen(urllib.request.Request(base+'/v1/chat/completions',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=45)
log=(out/'server.log').open('w')
process=subprocess.Popen([args.server,'-m',args.model,'--host','127.0.0.1','--port','18891','-t','2','-tb','2','-c','2048','-np','1','-ngl','0','--jinja','--no-warmup'],stdout=log,stderr=log)
result={}
try:
 start=time.perf_counter()
 while True:
  if process.poll() is not None:raise RuntimeError('server exited')
  try:
   if get('/health')['status']=='ok':break
  except Exception:pass
  if time.perf_counter()-start>90:raise TimeoutError('startup')
  time.sleep(.05)
 initial=cpu(process.pid);start=time.perf_counter();time.sleep(3)
 result['idle_sample']={'wall_seconds':time.perf_counter()-start,'process_cpu_seconds':cpu(process.pid)-initial,'definition':'single-process CPU seconds / wall seconds; 1.0 = one busy core'}
 body={'messages':[{'role':'user','content':'请写一段很长的狗狗童话故事，至少三百字。'}],'max_tokens':512,'temperature':0.1,'chat_template_kwargs':{'enable_thinking':False},'stream':True}
 with request(body) as response:
  for line in response:
   if line.startswith(b'data: ') and line[6:].strip()!=b'[DONE]':
    event=json.loads(line[6:]);chunks=[c.get('delta',{}).get('content') for c in event.get('choices',[])]
    if any(chunks):result['first_chunk_before_cancel']=''.join(c for c in chunks if c);break
 # Closing the SSE transport is the tested cancellation mechanism.
 start=time.perf_counter();slots=[]
 while time.perf_counter()-start<10:
  slots=get('/slots')
  if all(not s.get('is_processing') for s in slots):break
  time.sleep(.02)
 result['cancel']={'seconds_until_slot_idle':time.perf_counter()-start,'all_slots_idle':all(not s.get('is_processing') for s in slots),'slots':slots}
 start=time.perf_counter()
 with request({'messages':[{'role':'user','content':'用一句中文打招呼。'}],'max_tokens':48,'temperature':0.1,'chat_template_kwargs':{'enable_thinking':False}}) as response:result['recovery']=json.load(response)
 result['recovery_seconds']=time.perf_counter()-start
 subprocess.run(['python3',str(Path(__file__).with_name('diagnose.py')),'--out',str(out/'diagnostic.json')],check=True)
finally:
 start=time.perf_counter();process.terminate()
 try:process.wait(timeout=10)
 except subprocess.TimeoutExpired:process.kill();process.wait()
 result['unload']={'process_exited':process.poll() is not None,'seconds':time.perf_counter()-start,'exit_code':process.returncode,'proc_entry_present':Path(f'/proc/{process.pid}').exists()}
 log.close();(out/'lifecycle.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(result,ensure_ascii=False),flush=True)
