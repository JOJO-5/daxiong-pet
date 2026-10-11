#!/usr/bin/env python3
"""Real native LiteRT CPU qualification. Python only drives the test: the worker
loads a C library and performs inference without a Python runtime or child.
"""
import argparse,http.client,json,queue,socket,subprocess,threading,time,traceback
from pathlib import Path
from importlib.machinery import SourceFileLoader
stage=SourceFileLoader('stage_litert_native',str(Path(__file__).with_name('stage-litert-native.py'))).load_module()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--component',type=Path,required=True)
    parser.add_argument('--model',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args();component=args.component.resolve();out=args.out.resolve();out.mkdir(parents=True,exist_ok=True)
    manifest=json.loads((component/'component-manifest.json').read_text())
    model=stage.download(stage.SOURCES['test_model'],args.model.resolve())
    worker=component/('daxiong-litert-worker.exe' if manifest['target'].startswith('windows') else 'daxiong-litert-worker')
    library=component/manifest['source']['files'][0]
    result={'target':manifest['target'],'runtime':manifest['version'],'backend_requested':'cpu','model_sha256':stage.SOURCES['test_model']['sha256'],'checks':[],'timings':{},'replies':[],'scope':'native 0.8B INT8 CPU; synthetic text; no GPU or 2B/INT4 quality qualification'}
    processes=[];connections=[];peak_rss=0;stop_memory=threading.Event()
    def check(name,ok=True):
        assert ok,name
        result['checks'].append(name);print('PASS:',name,flush=True)
    def start():
        process=subprocess.Popen([str(worker),'--library',str(library),'--model',str(model),'--backend','cpu','--threads','2','--context','1024'],cwd=component,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(out/('worker-'+str(len(processes))+'.log')).open('w'),text=True)
        processes.append(process);lines=queue.Queue()
        def read():
            for line in process.stdout:
                try:lines.put(json.loads(line))
                except json.JSONDecodeError:lines.put({'event':'unexpected_stdout','line':line})
            lines.put({'event':'stdout_closed'})
        threading.Thread(target=read,daemon=True).start()
        ready=lines.get(timeout=100);assert ready['event']=='ready',ready
        assert ready['native'] and ready['pid']==process.pid
        result['timings'].setdefault('load_seconds',[]).append(ready['load_seconds'])
        return process,lines,ready['port']
    def request(port,method,path,body=None):
        c=http.client.HTTPConnection('127.0.0.1',port,timeout=100);connections.append(c)
        c.request(method,path,None if body is None else json.dumps(body),{} if body is None else {'Content-Type':'application/json'})
        return c,c.getresponse()
    def health(port):
        c,r=request(port,'GET','/health');assert r.status==200;d=json.loads(r.read());c.close();return d
    def wait_idle(port):
        deadline=time.monotonic()+8
        while time.monotonic()<deadline:
            if not health(port)['busy']:return
            time.sleep(.05)
        raise TimeoutError('worker remains busy')
    def payload(messages,tokens=32):return {'model':'daxiong-litert','stream':True,'messages':messages,'max_tokens':tokens}
    def read_chunk(response):
        while True:
            line=response.readline()
            if not line:return 'EOF',None
            if not line.startswith(b'data:'):continue
            value=line[5:].strip()
            if value==b'[DONE]':return 'DONE',None
            data=json.loads(value)
            if 'error' in data:return 'ERROR',data['error']
            text=data.get('choices',[{}])[0].get('delta',{}).get('content')
            if text:return 'TEXT',text
    def complete(port,messages):
        begun=time.monotonic();c,r=request(port,'POST','/v1/chat/completions',payload(messages));assert r.status==200
        text=[];n=0
        while True:
            kind,value=read_chunk(r)
            if kind=='TEXT':text.append(value);n+=1
            elif kind=='DONE':break
            else:raise RuntimeError((kind,value))
        c.close();reply=''.join(text);assert reply.strip();wait_idle(port)
        result['replies'].append(reply);result['timings'].setdefault('reply_seconds',[]).append(round(time.monotonic()-begun,3))
        return reply,n
    long=[{'role':'user','content':'请按顺序列出从1到200的整数，每行一个数，直到列完，不要解释。'}]
    try:
        for name,spec in manifest['files'].items():
            p=component/name;assert p.stat().st_size==spec['bytes'] and stage.sha(p)==spec['sha256'],name
        check('all staged native resources match fixed manifest hashes')
        check('component includes no Python files or model weights',not list(component.rglob('*.py')) and not list(component.rglob('*.gguf')) and not list(component.rglob('*.litertlm')))
        probe=subprocess.run([str(worker),'--library',str(library),'--probe'],cwd=component,capture_output=True,text=True,timeout=20)
        check('native ABI exports load without model or Python',probe.returncode==0 and json.loads(probe.stdout.strip())['native'])
        missing=subprocess.run([str(worker),'--library',str(out/'missing-runtime'),'--probe'],cwd=component,capture_output=True,text=True,timeout=10)
        check('missing explicit runtime fails without PATH fallback',missing.returncode!=0)
        process,lines,port=start();result['worker_pid']=process.pid
        import psutil
        native=psutil.Process(process.pid)
        def memory():
            nonlocal peak_rss
            while not stop_memory.wait(.05):
                try:peak_rss=max(peak_rss,native.memory_info().rss)
                except psutil.Error:return
        threading.Thread(target=memory,daemon=True).start()
        check('inference executable is the compiled native worker',Path(native.exe()).name==worker.name)
        check('native worker starts no Python inference child',not native.children(recursive=True))
        check('CPU health endpoint ready',health(port)['requested_backend']=='cpu')
        c,r=request(port,'POST','/v1/chat/completions',payload([{'role':'user','content':[{'type':'image_url','image_url':{'url':'data:image/png;base64,test'}}]}]));check('text worker rejects media instead of silently ignoring it',r.status==400);r.read();c.close()
        c,r=request(port,'POST','/v1/chat/completions',dict(payload(long),tools=[]));check('text worker rejects unsupported tools',r.status==400);r.read();c.close()
        with socket.create_connection(('127.0.0.1',port),timeout=5) as s:
            s.sendall(b'POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx')
            check('ambiguous HTTP length rejected',b' 400 ' in s.makefile('rb').readline())
        with socket.create_connection(('127.0.0.1',port),timeout=5) as s:
            s.sendall(b'GET /health HTTP/1.1\r\nHost: localhost\r\nOrigin: https://example.invalid\r\n\r\n')
            check('browser-origin requests rejected',b' 400 ' in s.makefile('rb').readline())
        first=[{'role':'system','content':'你是一只友好的桌面狗狗，简短回复。'},{'role':'user','content':'我的昵称是小林，请记住，只回复好的。'}]
        answer,n=complete(port,first);check('real model streams text and DONE',bool(answer) and n>0)
        history=first+[{'role':'assistant','content':answer},{'role':'user','content':'刚才我说我的昵称是什么？只回复昵称。'}]
        recall,_=complete(port,history);check('full request history reaches the native model','小林' in recall)
        for turn in range(3):
            c,r=request(port,'POST','/v1/chat/completions',payload(long,128));assert r.status==200
            kind,text=read_chunk(r);assert kind=='TEXT',(kind,text)
            assert health(port)['busy'],'generation already ended'
            if turn==0:
                other,reply=request(port,'POST','/v1/chat/completions',payload(long,16));check('second generation is rejected while one is active',reply.status==409);reply.read();other.close()
            begun=time.monotonic();process.stdin.write('{"op":"cancel"}\n');process.stdin.flush()
            ack=lines.get(timeout=8);assert ack['event']=='cancel' and ack['accepted'],ack
            while True:
                kind,value=read_chunk(r)
                if kind in ['DONE','EOF']:break
            c.close();wait_idle(port);elapsed=time.monotonic()-begun
            result['timings'].setdefault('cancel_seconds',[]).append(round(elapsed,3))
            check('active native cancellation '+str(turn+1),elapsed<8 and process.poll() is None)
        complete(port,[{'role':'user','content':'你好，简短打个招呼。'}]);check('same native engine recovers after repeated cancellation')
        c,r=request(port,'POST','/v1/chat/completions',payload(long,128));assert r.status==200;assert read_chunk(r)[0]=='TEXT';r.close();c.close();wait_idle(port)
        check('client disconnect clears native generation')
        complete(port,[{'role':'user','content':'你好，简短打个招呼。'}]);check('request after disconnect returns real text')
        c,r=request(port,'POST','/v1/chat/completions',payload(long,128));assert r.status==200;assert read_chunk(r)[0]=='TEXT'
        begun=time.monotonic();process.stdin.write('{"op":"shutdown"}\n');process.stdin.flush();code=process.wait(timeout=10)
        result['timings']['active_shutdown_seconds']=round(time.monotonic()-begun,3)
        check('shutdown during generation exits gracefully',code==0)
        r.close();c.close();check('exited native worker leaves no process',not psutil.pid_exists(process.pid))
        try:socket.create_connection(('127.0.0.1',port),timeout=.3).close();closed=False
        except OSError:closed=True
        check('shutdown removes loopback listener',closed)
        second,_,_=start();second.stdin.close();check('parent stdin EOF gracefully closes an idle worker',second.wait(timeout=10)==0)
        result['status']='passed'
    except Exception as e:
        result['status']='failed';result['error']=str(e);traceback.print_exc()
    finally:
        stop_memory.set();result['sampled_peak_native_rss_bytes']=peak_rss
        for c in connections:
            try:c.close()
            except Exception:pass
        for p in processes:
            if p.poll() is None:p.kill();p.wait()
        (out/'native-smoke.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
        print(json.dumps(result,ensure_ascii=False),flush=True)
    if result['status']!='passed':raise SystemExit(1)

if __name__=='__main__':main()
