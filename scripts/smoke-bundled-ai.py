#!/usr/bin/env python3
"""Native CPU model smoke; no screen/chat GUI qualification is implied."""
import argparse, json, socket, subprocess, time, urllib.request, uuid
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--ai',required=True,type=Path);a=p.parse_args()
server=a.ai/'runtime'/('llama-server.exe' if __import__('os').name=='nt' else 'llama-server')
model=next((a.ai/'models').glob('*.gguf'))
with socket.socket() as sock: sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
key=uuid.uuid4().hex
process=subprocess.Popen([str(server),'--model',str(model),'--host','127.0.0.1','--port',str(port),'--api-key',key,'-ngl','0','-t','2','-c','4096','--jinja','--no-mmproj'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def request(path,body=None):
    req=urllib.request.Request(f'http://127.0.0.1:{port}'+path,data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
    with opener.open(req,timeout=180) as r:return json.load(r)
try:
    deadline=time.monotonic()+180
    while True:
        assert process.poll() is None,'CPU runtime exited during startup'
        try: request('/health');break
        except Exception:
            if time.monotonic()>deadline:raise
            time.sleep(1)
    reply=request('/v1/chat/completions',{'messages':[{'role':'user','content':'用中文说你好。'}],'max_tokens':32,'chat_template_kwargs':{'enable_thinking':False}})
    assert reply['choices'][0]['message']['content'].strip(),'empty model reply'
    print(json.dumps({'native_cpu_inference':True,'scope':'pinned runtime + staged resources; installed GUI is separate','reply':reply['choices'][0]['message']['content']},ensure_ascii=True))
finally:
    process.terminate()
    try:process.wait(timeout=15)
    except subprocess.TimeoutExpired:process.kill();process.wait()
