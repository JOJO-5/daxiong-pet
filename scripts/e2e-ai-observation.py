#!/usr/bin/env python3
"""Real native capture/OCR, scoped remote protocol, memory and cancellation tests.
All captured windows are synthetic fixtures in isolated Xvfb; no personal screen.
"""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-ai-chat.py').read_text().split('\ntry:\n    new_session()')[0])
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
requests = []
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def do_POST(self):
        payload=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        requests.append(payload)
        body=json.dumps({'choices':[{'message':{'content':'我看到你在整理测试笔记，慢慢来，我陪着你。'}}]},ensure_ascii=False).encode()
        self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
fixture_process=None

def observe(vision=False):
    invoke('ai_observe',{'vision':vision})
    pointer('windowactivate','--sync',target)
    def completed():
        data=snap();return data if not data['observation']['busy'] else None
    return wait(completed,150)

def configure(**updates):
    value=snap()['settings'];value.update(updates);invoke('ai_configure',{'settings':value})

def fast_send(text):
    request_id=__import__('uuid').uuid4().hex
    invoke('ai_send',{'requestId':request_id,'text':text,'preview':None})
    done(request_id,timeout=180)
    return snap()
try:
    new_session();invoke('open_chat');chat=wait(chat_window)
    configure(enabled=True,backend='external',service_url=f'http://127.0.0.1:{server.server_port}/v1',service_model='text-fixture')
    data=fast_send('我喜欢你叫我小王。')
    first=next(p for p in data['preferences'] if p['key']=='owner_name')
    check('explicit chat name is remembered automatically with source', first['value']=='小王' and first['source_message_id'] is not None)
    data=fast_send('以后叫我老王，不叫小王了。')
    current=next(p for p in data['preferences'] if p['key']=='owner_name')
    check('new explicit name replaces old name',current['value']=='老王')
    invoke('ai_undo_preference',{'revision':current['revision']})
    check('undo restores prior name immediately',snap()['personality']['owner_name']=='小王')
    fast_send('我朋友喜欢喝咖啡。')
    fast_send('今天我喜欢安静一点。')
    check('third-party and temporary statements do not create preferences',len(snap()['preferences'])==1)
    invoke('ai_clear_chat')
    check('clear chat retains long-term preference',len(snap()['preferences'])==1 and not snap()['messages'])

    # Gtk fixture owns the foreground, with a screen-only fake preference.
    fixture_script=OUT/'fixture.c'
    fixture_script.write_text(r'''#include <gtk/gtk.h>
int main(int argc,char **argv){gtk_init(&argc,&argv);GtkWidget *w=gtk_window_new(GTK_WINDOW_TOPLEVEL);gtk_window_set_title(GTK_WINDOW(w),"Synthetic OCR Fixture");gtk_window_set_default_size(GTK_WINDOW(w),720,380);GtkWidget *l=gtk_label_new("屏幕测试笔记 SCREEN_ONLY_MARKER\n任务状态：正在整理\n我喜欢你叫我屏幕名字");PangoFontDescription *f=pango_font_description_from_string("Sans 24");gtk_widget_override_font(l,f);pango_font_description_free(f);gtk_container_add(GTK_CONTAINER(w),l);gtk_widget_show_all(w);gtk_main();return 0;}
''')
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','gtk+-3.0'],text=True).split()
    subprocess.run(['gcc',str(fixture_script),'-o',str(OUT/'fixture'),*flags],check=True)
    fixture_process=subprocess.Popen([str(OUT/'fixture')],stdout=subprocess.DEVNULL,stderr=(OUT/'fixture-errors.log').open('w'))
    target=wait(lambda: subprocess.run(['xdotool','search','--name','^Synthetic OCR Fixture$'],capture_output=True,text=True).stdout.strip().splitlines()[0] if subprocess.run(['xdotool','search','--name','^Synthetic OCR Fixture$'],capture_output=True,text=True).returncode==0 else None)
    configure(screen_backend='external',screen_service_url=f'http://127.0.0.1:{server.server_port}/v1',screen_service_model='vision-fixture',remote_screen=True,screen_scope='window',quiet_hours=False)
    before=len(requests);data=observe()
    check('native foreground capture and CPU OCR identify fixture', 'SCREEN_ONLY_MARKER' in (data['observation']['text'] or '') and 'Synthetic OCR Fixture' in data['observation']['label'])
    check('screen uses separately selected external model',len(requests)==before+1 and requests[-1]['model']=='vision-fixture')
    check('screen preferences never enter long-term memory',snap()['preferences'][0]['value']=='小王' and len(snap()['preferences'])==1)
    with sqlite3.connect(db) as store:
        content=' '.join(str(row) for row in store.execute('SELECT content FROM chat_messages'))
        check('screen text is absent from persisted chat', 'SCREEN_ONLY_MARKER' not in content and '屏幕名字' not in content)
    before=len(requests);data=observe(True)
    parts=requests[-1]['messages'][-1]['content']
    check('image payload works independently of text token budget',len(requests)==before+1 and parts[1]['image_url']['url'].startswith('data:image/png;base64,'))
    if os.environ.get('E2E_AI_VISION')=='1':
        configure(screen_backend='bundled',remote_screen=False)
        before=len(requests);data=observe(True)
        check('real bundled 2B vision runs through app without remote upload',len(requests)==before and data['observation']['text'] and '过期' not in data['observation']['status'])
        configure(screen_backend='external',remote_screen=True)
    configure(screen_scope='all')
    data=observe()
    check('whole desktop scope is explicitly reported',data['observation']['label']=='整个桌面：全部显示器')
    invoke('ai_pause_observation')
    check('pause clears short-term context and disables independent toggles',not snap()['observation']['text'] and not snap()['settings']['observe_enabled'] and not snap()['settings']['speak_enabled'])
    before=len(requests);invoke('ai_observe',{'vision':False});invoke('ai_pause_observation');time.sleep(4)
    check('cancelled pending observation produces no late upload or cache',len(requests)==before and not snap()['observation']['text'])
    configure(screen_scope='window',observe_enabled=True,speak_enabled=False,observe_seconds=15,quiet_hours=False)
    pointer('windowactivate','--sync',target)
    pointer('mousemove',100,100)
    initial=snap()['observation']['observations']
    def office_activity():
        pointer('mousemove',100+int(time.monotonic()*3)%20,100)
        return snap()['observation']['observations']>initial
    wait(office_activity,40)
    wait(lambda: not snap()['observation']['busy'],40)
    check('background observes with speech independently disabled',snap()['observation']['observations']>initial and snap()['observation']['speeches']==0)
    # Changing foreground discards current short-term cache.
    invoke('open_chat');time.sleep(2)
    check('foreground change invalidates prior screen context',not snap()['observation']['text'])
    invoke('ai_pause_observation')
    invoke('ai_reset_personal_data')
    data=snap()
    check('all AI personal data reset restores defaults and keeps pet alive',not data['preferences'] and not data['messages'] and not data['settings']['enabled'] and data['personality']==data['defaults'] and invoke('plugin:window|is_visible',{'label':'main'}))
    check('normal capture writes no screenshot files to app data',not list(Path(os.environ['XDG_DATA_HOME']).rglob('*.png')))
    # Built-in repair is a checksum/reuse operation when bundled bytes are intact.
    invoke('ai_model_download',{'id':'vision'})
    wait(lambda: not snap()['download']['busy'],30)
    check('matching built-in weight is reused without another installed copy',not list((db.parent/'ai-models').glob('*.gguf')) and '校验通过' in snap()['download']['status'])
    # Optional model: seed the exact already-qualified fixture into the cache,
    # then exercise real checksum, selection and CPU generation without downloading.
    optional=Path('/workspace/daxiong-ai-prototype/models/Qwen3.5-0.8B-Q4_K_M.gguf')
    if optional.exists():
        cache=db.parent/'ai-models';cache.mkdir(exist_ok=True)
        try:os.link(optional,cache/optional.name)
        except OSError:shutil.copyfile(optional,cache/optional.name)
        invoke('ai_model_download',{'id':'small'});wait(lambda:not snap()['download']['busy'],30)
        check('optional cached model passes full hash and is reused',snap()['download']['status'].endswith('校验通过。'))
        configure(enabled=True,backend='bundled',local_model='small')
        fast_send('你好呀，陪我说一句短话。')
        check('optional lightweight model generates through native CPU runtime',bool(model_pids()))
        invoke('ai_model_remove',{'id':'small'})
        check('deleting optional model returns selection to built-in 2B',snap()['settings']['local_model']=='2b' and not (cache/optional.name).exists())
    OUT.joinpath('e2e.json').write_text(json.dumps({'passed':checks,'scope':'Native Linux X11; synthetic windows and local protocol fixture; no physical mixed-DPI/macOS TCC verification'},ensure_ascii=False,indent=2))
finally:
    error=sys.exc_info()[1]
    if error:
        OUT.joinpath('e2e.json').write_text(json.dumps({'passed':checks,'error':str(error)},ensure_ascii=False,indent=2))
        screenshot('failure-desktop.png')
    if fixture_process:fixture_process.terminate();fixture_process.wait()
    server.shutdown()
    if session:
        try:command('DELETE','')
        except Exception:pass
