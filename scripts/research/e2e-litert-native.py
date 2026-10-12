#!/usr/bin/env python3
"""Use the standalone native component through the existing Tauri external
backend. No production settings/SQLite or model bundle is modified.
"""
from pathlib import Path
import sys
script_root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(script_root))
__file__=str(script_root/'e2e-ai-chat.py')
exec(Path(__file__).read_text().split('\ntry:\n    new_session()')[0])
import queue,threading,http.client
worker=None;events=queue.Queue();port=None
def select(selector):
    for handle in command('GET','/window/handles'):
        command('POST','/window',{'handle':handle})
        if js('return !!document.querySelector('+json.dumps(selector)+')'):return handle
    return None
def idle():
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=5);c.request('GET','/health');r=c.getresponse();d=json.loads(r.read());c.close();return not d['busy']
try:
    component=Path(os.environ['LITERT_COMPONENT']).resolve();model=Path(os.environ['LITERT_TEST_MODEL']).resolve()
    manifest=json.loads((component/'component-manifest.json').read_text())
    worker=subprocess.Popen([str(component/'daxiong-litert-worker'),'--library',str(component/manifest['source']['files'][0]),'--model',str(model),'--context','2048'],cwd=component,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(OUT/'native-worker.log').open('w'),text=True)
    def read_worker():
        for line in worker.stdout:events.put(json.loads(line))
    threading.Thread(target=read_worker,daemon=True).start();ready=events.get(timeout=100);assert ready['event']=='ready',ready;port=ready['port'];timings['native_load_seconds']=ready['load_seconds']
    new_session();main=command('GET','/window')
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    invoke('set_gravity',{'enabled':False});invoke('set_encounters',{'enabled':False});native=pet_native();pointer('windowmove',native,800,200)
    invoke('open_chat');full=wait(chat_window);wait(lambda:invoke('plugin:window|is_visible',{'label':'chat'}))
    full_native=wait(lambda:next((w for w in subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).splitlines() if 'WIDTH=500\n' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True)),None))
    pointer('windowmove',full_native,20,30)
    settings=snap()['settings'];settings.update(enabled=True,backend='external',service_url=f'http://127.0.0.1:{port}/v1',service_model='daxiong-litert',output_tokens=32)
    invoke('ai_configure',{'settings':settings});wait(lambda:js("return !document.querySelector('#chat-message').disabled"))
    begun=time.monotonic();reply=done(send('你好，简短打个招呼。'));timings['full_native_reply_seconds']=round(time.monotonic()-begun,3)
    check('full native chat renders standalone C API model reply',bool(reply['content'].strip()))
    command('POST','/window',{'handle':main});wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label')==="+json.dumps(reply['content'])))
    check('native LiteRT reply reaches pet bubble');screenshot('native-full-and-bubble.png')
    pointer('mousemove',30,30);time.sleep(.2)
    wait(lambda:(pointer('mousemove','--window',native,150,160) or js("return document.querySelector('[data-testid=pet]').dataset.clickable==='true'")))
    pointer('mousedown',1);time.sleep(.06);pointer('mouseup',1)
    quick=wait(lambda:select('.quick-chat'));wait(lambda:invoke('plugin:window|is_visible',{'label':'quick-chat'}))
    check('native pet tap opens compact input with research backend')
    request_id=send('我们来玩吧，简短回答。');reply=done(request_id)
    check('compact input completes through standalone native process',bool(reply['content'].strip()))
    command('POST','/window',{'handle':full});wait(lambda:js('return document.querySelector(".chat-window")?.textContent.includes('+json.dumps(reply['content'])+')'))
    check('full and compact chat share native model reply')
    command('POST','/window',{'handle':main});wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label')==="+json.dumps(reply['content'])))
    screenshot('native-shared-and-bubble.png')
    command('POST','/window',{'handle':full});settings['output_tokens']=256;invoke('ai_configure',{'settings':settings});request_id=send('请按顺序列出从1到200的整数，每行一个，不要解释。')
    wait(lambda:next((m for m in snap()['messages'] if m['request_id']==request_id and m['role']=='assistant' and m['status']=='generating' and m['content'].strip()),None),30)
    click('停止回复');done(request_id,'cancelled');wait(idle,8);check('pet Stop disconnect clears native generation')
    reply=done(send('你好，简短打个招呼。'));check('chat works again after native cancellation',bool(reply['content'].strip()))
    worker.stdin.write('{"op":"shutdown"}\n');worker.stdin.flush();check('standalone component shuts down cleanly after pet usage',worker.wait(timeout=10)==0)
finally:
    error=sys.exc_info()[1]
    OUT.joinpath('native-pet-result.json').write_text(json.dumps({'error':str(error) if error else None,'checks':checks,'timings':timings,'scope':'real standalone C ABI CPU worker and Tauri external entry; 0.8B text/context2048; no GPU, bundled backend or GUI component download claim'},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
    if worker and worker.poll() is None:worker.kill();worker.wait()
