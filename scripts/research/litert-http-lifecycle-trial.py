import subprocess,http.client,socket,time,json,os,signal,traceback
from pathlib import Path
import argparse
parser=argparse.ArgumentParser(description='Independent LiteRT CPU lifecycle probe; work directory is <model-and-venv-directory>/round2.')
parser.add_argument('--work',type=Path,required=True)
root=parser.parse_args().work.resolve()
root.mkdir(parents=True,exist_ok=True)
port=4531
result={'backend':'CPU','runtime':'litert-lm 0.18.0','scope':'isolated official Python CLI loopback process, synthetic text only; not production app fallback','checks':[]}
def save():(root/'http-lifecycle-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
def check(name,passed):
    result['checks'].append({'name':name,'passed':bool(passed)});save();print(name,passed,flush=True)
def post(text,limit):
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=100)
    payload={'model':str(root.parent/'Qwen3.5-0.8B_int8.litertlm'),'messages':[{'role':'user','content':text}],'max_tokens':limit,'stream':True}
    c.request('POST','/v1/chat/completions',json.dumps(payload),{'Content-Type':'application/json'})
    r=c.getresponse();assert r.status==200,(r.status,r.read())
    return c,r
def chunk(response):
    while True:
        line=response.readline()
        if not line:return None
        if not line.startswith(b'data:'):continue
        value=line[5:].strip()
        if value==b'[DONE]':return None
        d=json.loads(value);text=d.get('choices',[{}])[0].get('delta',{}).get('content')
        if text:return text
server=None;connection=None
def terminated(sig,frame):
    result['status']='interrupted';save();raise SystemExit(128+sig)
signal.signal(signal.SIGTERM,terminated)
try:
    with socket.socket() as probe:probe.bind(('127.0.0.1',port))
    server=subprocess.Popen([str(root.parent/'venv/bin/litert-lm'),'serve','--host','127.0.0.1','--port',str(port),'--config',str(root.parent/'serve-config.json')],stdout=(root/'http-server.log').open('w'),stderr=subprocess.STDOUT,start_new_session=True)
    deadline=time.monotonic()+20
    while time.monotonic()<deadline:
        if server.poll() is not None:raise RuntimeError('server exited before listening')
        try:
            with socket.create_connection(('127.0.0.1',port),timeout=.3):break
        except OSError:time.sleep(.1)
    else:raise TimeoutError('server startup timeout')
    check('official server listens on loopback',True)
    connection,response=post('请依次列出从1到200的整数，不要提前结束。',128)
    result['before_disconnect']=chunk(response);check('real SSE emits nonempty content',bool(result['before_disconnect']))
    response.close();connection.close();connection=None;time.sleep(.5)
    connection,response=post('你好，简短打个招呼。',16)
    pieces=[]
    while True:
        text=chunk(response)
        if text is None:break
        pieces.append(text)
    result['after_disconnect_reply']=''.join(pieces)
    check('request after stream disconnect returns content',bool(result['after_disconnect_reply'].strip()))
    response.close();connection.close();connection=None
    connection,response=post('请依次列出从1到200的整数，不要提前结束。',128)
    result['before_crash']=chunk(response);check('crash injection begins during real generation',bool(result['before_crash']))
    os.killpg(server.pid,signal.SIGKILL);result['crash_exit_code']=server.wait(timeout=10)
    check('owned worker process exits after injected crash',result['crash_exit_code']==-signal.SIGKILL)
    response.close();connection.close();connection=None
    try:
        with socket.create_connection(('127.0.0.1',port),timeout=.5):closed=False
    except OSError:closed=True
    check('crashed worker leaves no listening port',closed)
    result['status']='passed' if all(c['passed'] for c in result['checks']) else 'failed_checks'
except Exception as error:
    result['status']='error';result['error']=str(error);traceback.print_exc()
finally:
    if connection:connection.close()
    if server and server.poll() is None:
        os.killpg(server.pid,signal.SIGTERM)
        try:server.wait(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait()
    save();print(json.dumps(result,ensure_ascii=False),flush=True)
if result.get('status')!='passed':raise SystemExit(1)
