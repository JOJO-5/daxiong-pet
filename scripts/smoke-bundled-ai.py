#!/usr/bin/env python3
"""Actual CPU text/image protocol probe; GUI and image semantics qualify separately."""
import argparse, base64, json, socket, subprocess, time, urllib.request, uuid
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--ai',required=True,type=Path);a=p.parse_args()
root=Path(__file__).resolve().parents[1]
spec=json.loads((root/'scripts/ai-prototype/default-model.json').read_text(encoding='utf-8'))
server=a.ai/'runtime'/('llama-server.exe' if __import__('os').name=='nt' else 'llama-server')
model=a.ai/'models'/spec['default_text_model']['name']
projector=a.ai/'models'/spec['vision_component']['name']
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
results=[]
for vision in [False,True]:
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    key=uuid.uuid4().hex
    command=[str(server),'--model',str(model),'--host','127.0.0.1','--port',str(port),'--api-key',key,'-ngl','0','-t','2','-tb','2','-c','4096','--jinja','--no-warmup']
    command += ['--mmproj',str(projector),'--no-mmproj-offload','--image-min-tokens','256','--image-max-tokens','256'] if vision else ['--no-mmproj']
    process=subprocess.Popen(command,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    def request(path,body=None):
        req=urllib.request.Request(f'http://127.0.0.1:{port}'+path,data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
        with opener.open(req,timeout=180) as response:return json.load(response)
    try:
        started=time.monotonic();deadline=started+180
        while True:
            assert process.poll() is None,'CPU runtime exited during startup'
            try:request('/health');break
            except Exception:
                if time.monotonic()>deadline:raise
                time.sleep(.2)
        content='用中文说你好。'
        if vision:
            image=(root/'docs/research/ocr-cpu-trial-2026-10-11/v5/chinese-12.png').read_bytes()
            content=[{'type':'text','text':'用一句话说明图片里是否有文字。'},{'type':'image_url','image_url':{'url':'data:image/png;base64,'+base64.b64encode(image).decode()}}]
        reply=request('/v1/chat/completions',{'messages':[{'role':'user','content':content}],'max_tokens':32,'chat_template_kwargs':{'enable_thinking':False}})
        text=reply['choices'][0]['message']['content'].strip();assert text,'empty model reply'
        results.append({'input':'image' if vision else 'text','seconds':round(time.monotonic()-started,2),'reply':text})
    finally:
        process.terminate()
        try:process.wait(timeout=15)
        except subprocess.TimeoutExpired:process.kill();process.wait()
print(json.dumps({'native_cpu_inference':True,'scope':'pinned runtime and staged text/projector; image semantics and installed GUI separate','results':results},ensure_ascii=True))
