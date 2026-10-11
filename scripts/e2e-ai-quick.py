#!/usr/bin/env python3
"""Native pet tap, anchored input and two-way chat synchronization; loopback fixture."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-ai-chat.py').read_text().split('\ntry:\n    new_session()')[0])
import threading
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
requests=[]
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_POST(self):
        payload=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(payload)
        content=payload['messages'][-1]['content'];time.sleep(4 if 'slow' not in content else 7)
        reply='我陪着你，慢慢来。'+('同步回复：'+content)
        body=json.dumps({'choices':[{'message':{'content':reply}}]},ensure_ascii=False).encode()
        self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers()
        try:self.wfile.write(body)
        except (BrokenPipeError,ConnectionResetError):pass
server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
def select(selector):
    for handle in command('GET','/window/handles'):
        command('POST','/window',{'handle':handle})
        if js('return !!document.querySelector('+json.dumps(selector)+')'):return handle
    return None
def tap_pet():
    command('POST','/window',{'handle':main})
    pointer('mousemove',30,30);time.sleep(.2)
    wait(lambda:(pointer('mousemove','--window',pet_native(),150,160) or js("return document.querySelector('[data-testid=pet]').dataset.clickable==='true'")))
    time.sleep(.2);pointer('mousedown',1);time.sleep(.06);pointer('mouseup',1)
def press_button(text):
    r=js("const e=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==="+json.dumps(text)+");if(!e||e.disabled)return null;const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,width:innerWidth};")
    assert r, 'missing enabled button '+text
    candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).splitlines()
    target=next(w for w in candidates if f"WIDTH={r['width']}\n" in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowactivate','--sync',target);pointer('mousemove','--window',target,round(r['x']),round(r['y']));time.sleep(.15);pointer('click',1)
def send_here(text):
    fill('#chat-message',text);press_button('发送');return wait(lambda:snap()['active_request_id'])
def switch(handle):command('POST','/window',{'handle':handle})
try:
    new_session();main=command('GET','/window');wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    invoke('set_encounters',{'enabled':False});invoke('set_gravity',{'enabled':False})
    native=pet_native();pointer('windowmove',native,800,200);time.sleep(.5)
    tap_pet();quick=wait(lambda:select('.quick-chat'));wait(lambda:invoke('plugin:window|is_visible',{'label':'quick-chat'}))
    check('short native pet tap opens compact input')
    wait(lambda:js("return document.activeElement?.id==='chat-message'"));check('pet tap immediately focuses message input')
    print('quick geometry',invoke('plugin:window|outer_size',{'label':'quick-chat'}),js('return {height:innerHeight,width:innerWidth}'),flush=True)
    check('compact window respects requested height',invoke('plugin:window|outer_size',{'label':'quick-chat'})['height']==146)
    settings=snap()['settings'];settings.update(enabled=True,backend='external',service_url=f'http://127.0.0.1:{server.server_port}/v1',service_model='quick-fixture');invoke('ai_configure',{'settings':settings})
    wait(lambda:not js("return [...document.querySelectorAll('button')].some(b=>b.textContent==='启用聊天')"))
    p=invoke('plugin:window|outer_position',{'label':'quick-chat'});dog=invoke('plugin:window|outer_position',{'label':'main'})
    print('anchor geometry',p,dog,invoke('plugin:window|outer_size',{'label':'main'}),flush=True)
    wait(lambda:invoke('plugin:window|outer_position',{'label':'quick-chat'})['x']==invoke('plugin:window|outer_position',{'label':'main'})['x']-10)
    p=invoke('plugin:window|outer_position',{'label':'quick-chat'});dog=invoke('plugin:window|outer_position',{'label':'main'})
    check('compact input fits centered below pet when space is available',p['x']==dog['x']-10 and p['y']>=dog['y']+240 and p['y']+146<=800)
    press_button('聊天记录');full=wait(lambda:select('.chat-window'));wait(lambda:invoke('plugin:window|is_visible',{'label':'chat'}))
    full_native=next(w for w in subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).splitlines() if 'WIDTH=500\n' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',full_native,20,30)
    switch(quick);first=send_here('小框发送一条消息')
    switch(full);wait(lambda:js("return document.querySelector('.chat-log').textContent.includes('小框发送一条消息')"));check('compact user message immediately reaches full chat')
    wait(lambda:js("return [...document.querySelectorAll('button')].some(b=>['停止','停止回复'].includes(b.textContent))"));check('full chat synchronizes generating state')
    done(first,timeout=30);wait(lambda:js("return document.querySelector('.chat-log').textContent.includes('同步回复：小框发送一条消息')"));check('same completed reply reaches full chat')
    switch(main);wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label').includes('同步回复：小框发送一条消息')"));check('pet speaks same complete reply through paginated bubble');screenshot('quick-reply.png')
    switch(full);second=send_here('完整窗口发送消息')
    switch(quick);wait(lambda:js("return [...document.querySelectorAll('button')].some(b=>['停止','停止回复'].includes(b.textContent))"));check('full chat synchronizes busy state to compact input')
    done(second,timeout=30);switch(main);wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label').includes('同步回复：完整窗口发送消息')"));check('full chat response also reaches pet bubble')
    switch(quick);slow=send_here('slow 请稍等');switch(full);wait(lambda:js("return [...document.querySelectorAll('button')].some(b=>['停止','停止回复'].includes(b.textContent))"));press_button('停止回复');done(slow,'cancelled',timeout=10)
    switch(quick);wait(lambda:js("return [...document.querySelectorAll('button')].some(b=>b.textContent==='发送')"));check('stop from full chat releases compact input too')
    third=send_here('slow 关闭大窗口继续');switch(full);invoke('close_chat');switch(quick);done(third,timeout=30);check('closing full chat preserves visible compact conversation')
    fourth=send_here('slow 收起小框仍用气泡');invoke('close_chat');time.sleep(.3);switch(full);invoke('open_chat');done(fourth,timeout=30);check('closing compact input preserves its reply and shared record')
    invoke('ai_clear_chat');switch(quick);wait(lambda:not snap()['messages']);switch(main);wait(lambda:not js("return !!document.querySelector('.bubble')"));check('clear full chat clears shared messages and pet reply')
    tap_pet();switch(quick);wait(lambda:invoke('plugin:window|is_visible',{'label':'quick-chat'}));invoke('close_chat');switch(full);invoke('close_chat');switch(main)
    native=pet_native();pointer('mousemove','--window',native,150,160);time.sleep(.2);pointer('mousedown',1);time.sleep(.1);pointer('mousemove','--window',native,220,180);time.sleep(.25);pointer('mouseup',1);time.sleep(.5)
    check('native pet drag does not reopen compact input',not invoke('plugin:window|is_visible',{'label':'quick-chat'}))
    pointer('windowmove',native,980,560);time.sleep(.4);tap_pet();switch(quick);wait(lambda:invoke('plugin:window|is_visible',{'label':'quick-chat'}))
    p=invoke('plugin:window|outer_position',{'label':'quick-chat'});size=invoke('plugin:window|outer_size',{'label':'quick-chat'})
    check('bottom right edge keeps whole compact window inside screen',p['x']>=0 and p['y']>=0 and p['x']+size['width']<=1280 and p['y']+size['height']<=800)
    screenshot('quick-edge.png')
    pointer('windowmove',native,0,560);time.sleep(.5);p=invoke('plugin:window|outer_position',{'label':'quick-chat'})
    check('visible compact input follows pet to opposite edge',p['x']>=0 and p['x']+size['width']<=1280 and p['y']+size['height']<=800)
    screenshot('quick-left.png');switch(main);invoke('set_visible',{'visible':False});check('hiding pet also hides compact input',not invoke('plugin:window|is_visible',{'label':'quick-chat'}));invoke('set_visible',{'visible':True});check('one durable pair per shared request and no duplicated service calls',len(requests)==5)
    if os.environ.get('E2E_AI_QUICK_CPU') == '1':
        tap_pet();switch(quick);wait(lambda:invoke('plugin:window|is_visible',{'label':'quick-chat'}))
        settings=snap()['settings'];settings.update(enabled=True,backend='bundled',threads=2)
        invoke('ai_configure',{'settings':settings});invoke('ai_clear_chat')
        wait(lambda:js("return [...document.querySelectorAll('button')].some(b=>b.textContent==='发送')"))
        started=time.monotonic();cpu=send_here('Hello! Reply with one short friendly sentence.')
        reply=done(cpu,timeout=180);timings['compact_cpu_reply_seconds']=round(time.monotonic()-started,3)
        check('real bundled CPU model completes through compact input',bool(reply['content'].strip()))
        check('CPU conversation has one durable pair',len(snap()['messages'])==2)
        switch(main);wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label')==="+json.dumps(reply['content'])))
        check('real CPU reply reaches pet speech bubble');screenshot('quick-cpu.png')
        switch(quick);press_button('聊天记录');switch(full)
        wait(lambda:js("return document.querySelector('.chat-log').textContent.includes("+json.dumps(reply['content'])+")"))
        check('full chat loads identical real CPU reply',len(requests)==5)
finally:
    error=sys.exc_info()[1]
    if error:
        try:screenshot('failure.png')
        except Exception:pass
    OUT.joinpath('e2e.json').write_text(json.dumps({'timings':timings,'passed':checks,'error':str(error) if error else None,'scope':'Native Linux X11/WebKit with real OS input and synthetic loopback protocol; GPU/mixed-DPI untested'},ensure_ascii=False,indent=2))
    server.shutdown();server.server_close()
    if session:
        try:command('DELETE','')
        except Exception:pass
