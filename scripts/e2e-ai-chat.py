#!/usr/bin/env python3
"""Native Tauri/WebKit/X11 integration with the pinned real CPU model.
Requires staged resources in DAXIONG_AI_RESOURCE_DIR; never downloads models.
"""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
import re
import signal
import sqlite3

db = Path(os.environ['XDG_DATA_HOME']) / 'com.jojo.daxiongpet' / 'ai.sqlite3'
timings = {}

def snap():
    return invoke('ai_snapshot')

def chat_window():
    for handle in command('GET', '/window/handles'):
        command('POST', '/window', {'handle': handle})
        if js("return !!document.querySelector('.chat-window')"):
            return handle
    return None

def native_click(expression):
    wait(lambda: invoke('plugin:window|is_visible', {'label': 'chat'}))
    rect = js("const e=" + expression + ";if(!e||e.disabled)return null;e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,width:innerWidth};")
    assert rect, 'button missing or disabled'
    def find_target():
        candidates = subprocess.run(['xdotool', 'search', '--onlyvisible', '--class', 'Daxiong-pet'], capture_output=True, text=True).stdout.splitlines()
        for candidate in candidates:
            geometry = subprocess.run(['xdotool', 'getwindowgeometry', '--shell', candidate], capture_output=True, text=True)
            if geometry.returncode == 0 and f"WIDTH={rect['width']}\n" in geometry.stdout:
                return candidate
        return None
    target = wait(find_target)
    info = subprocess.check_output(['xwininfo', '-id', target], text=True)
    geometry = {a: int(re.search(r'Absolute upper-left ' + a + r':\s*(-?\d+)', info).group(1)) for a in ('X', 'Y')}
    pointer('windowraise', target)
    pointer('windowactivate', '--sync', target)
    pointer('mousemove', geometry['X'] + round(rect['x']), geometry['Y'] + round(rect['y']))
    time.sleep(.12)
    pointer('mousedown', 1)
    time.sleep(.08)
    pointer('mouseup', 1)
    time.sleep(.12)

def fill(selector, value):
    # Native input setter + bubbling input event, exercising React controlled fields.
    js("const e=document.querySelector(" + json.dumps(selector) + ");const proto=e instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e," + json.dumps(value) + ");e.dispatchEvent(new Event('input',{bubbles:true}));")

def send(text):
    fill('#chat-message', text)
    click('发送')
    return wait(lambda: snap()['active_request_id'])

def done(request_id, status='complete', timeout=180):
    def result():
        data = snap()
        if data['active_request_id']:
            return None
        return next((m for m in data['messages'] if m['request_id'] == request_id and m['role'] == 'assistant'), None)
    message = wait(result, timeout)
    check('request ends as ' + status, message['status'] == status)
    return message

def model_pids():
    # Do not log argv: it includes a per-process authentication key.
    result = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            argv = (entry / 'cmdline').read_bytes().split(b'\0')
            if argv and b'llama-server' in argv[0] and any(b'chat-native-assets' in a for a in argv):
                result.append(int(entry.name))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    return result

def pet_is_above():
    return '_NET_WM_STATE_ABOVE' in subprocess.check_output(['xprop', '-id', pet_native(), '_NET_WM_STATE'], text=True)

def screenshot(name):
    time.sleep(.4)  # Allow WebKit and the X11 compositor to paint the latest React update.
    subprocess.run(['import', '-window', 'root', str(OUT / name)], check=True)

try:
    new_session()
    main = command('GET', '/window')
    wait(lambda: js("return !!document.querySelector('.pet-sheet')?.complete"))
    check('AI database is lazy; ordinary pet startup does not create it', not db.exists())
    invoke('open_chat')
    chat = wait(chat_window)
    data = snap()
    check('real chat defaults to disabled with verified resources available', not data['settings']['enabled'] and data['assets']['available'])
    click('性格与设置')
    fill('#ai-owner-name', '小木')
    click('保存性格')
    wait(lambda: snap()['personality']['owner_name'] == '小木')
    check('GUI personality saved to SQLite', sqlite3.connect(db).execute('SELECT count(*) FROM personality_profiles').fetchone()[0] == 1)
    check('focused chat lowers pet overlay so form controls cannot be intercepted', not pet_is_above())
    click('启用本地聊天')
    wait(lambda: snap()['settings']['enabled'])
    click('聊天')
    started = time.monotonic()
    first = send('今天工作有点累，陪我聊一句吧。')
    invoke('play_action', {'action': 'throw'})
    phases = set()
    def returned():
        phase = invoke('play_status')['phase']
        phases.add(phase)
        return phase == 'returned'
    wait(returned, 35)
    check('real model request and native fetch run together', 'returning' in phases and 'chasing' in phases)
    message = done(first)
    timings['first_reply_seconds_in_shared_container'] = round(time.monotonic() - started, 2)
    check('real CPU model produced Chinese text persisted and rendered', bool(message['content']) and js("return [...document.querySelectorAll('.chat-message.assistant p')].some(e=>e.textContent.length>0)"))
    screenshot('chat-reply-desktop.png')
    invoke('play_action', {'action': 'cancel'})

    second = send('给我讲一个狗狗的长故事，多讲几句。')
    wait(lambda: any(m['request_id'] == second and m['role'] == 'assistant' and m['content'] for m in snap()['messages']), 180)
    click('停止回复')
    cancelled = done(second, 'cancelled')
    time.sleep(1)
    check('cancel preserves partial reply without later deltas', next(m for m in snap()['messages'] if m['id'] == cancelled['id'])['content'] == cancelled['content'])

    third = send('再陪我说几句工作累了怎么办。')
    invoke('ai_cancel', {'requestId': second})
    check('stale Stop cannot cancel the new request', snap()['active_request_id'] == third)
    click('×')
    command('POST', '/window', {'handle': main})
    wait(lambda: not invoke('plugin:window|is_visible', {'label': 'chat'}))
    check('closing chat keeps the pet alive', invoke('plugin:window|is_visible', {'label': 'main'}))
    wait(pet_is_above)
    check('closing chat restores the pet always-on-top layer', pet_is_above())
    invoke('open_chat')
    command('POST', '/window', {'handle': chat})
    done(third, 'cancelled')

    click('性格与设置')
    count = len(snap()['messages'])
    fill('#ai-owner-name', '预览主人')
    click('试聊')
    wait(lambda: snap()['active_request_id'])
    wait(lambda: not snap()['active_request_id'], 180)
    check('real draft preview produces text without saving profile or chat', js("return document.querySelector('.chat-preview-reply').textContent!=='点一下，听听狗狗怎么回应。'") and len(snap()['messages']) == count and snap()['personality']['owner_name'] == '小木')
    screenshot('personality-desktop.png')
    click('聊天')
    crashed = send('我想听一个详细的故事。')
    wait(lambda: model_pids())
    for pid in model_pids():
        os.kill(pid, signal.SIGKILL)
    done(crashed, 'failed')
    check('model crash reports failure while pet commands still work', js("return !!document.querySelector('.chat-error')") and invoke('play_status')['phase'] == 'off')
    retry = send('你好呀，回来陪我说一句。')
    done(retry)
    check('model restarts successfully after crash', bool(model_pids()))

    count = len(snap()['messages'])
    command('DELETE', '')
    session = None
    wait(lambda: not model_pids(), 10)
    check('exiting native application leaves no model child', not model_pids())
    with sqlite3.connect(db) as store:
        old = int(time.time()) - 31 * 86400
        store.execute("INSERT INTO chat_messages(session_id,role,content,created_at,status,request_id) VALUES(1,'user','expired test',?,'complete','expired-e2e')", (old,))
    new_session()
    invoke('open_chat')
    chat = wait(chat_window)
    data = snap()
    check('restart preserves chat/personality/settings and prunes expired messages', len(data['messages']) == count and data['personality']['owner_name'] == '小木' and data['settings']['enabled'] and all(m['request_id'] != 'expired-e2e' for m in data['messages']))
    command('POST', '/window/rect', {'width': 380, 'height': 480})
    click('聊天')
    time.sleep(.3)
    check('small chat window has no horizontal overflow and usable composer', js("const e=document.querySelector('.chat-compose');const r=e.getBoundingClientRect();return document.documentElement.scrollWidth<=innerWidth && r.bottom<=innerHeight && r.width>200"))
    screenshot('chat-small-desktop.png')
    click('性格与设置')
    click('清空聊天')
    # Place the real always-on-top pet over the confirmation area. The focused
    # chat must still receive the click, without hiding or disabling the dog.
    rect = js("const r=[...document.querySelectorAll('button')].find(b=>b.textContent==='确认清空').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}")
    pos = invoke('plugin:window|inner_position', {'label': 'chat'})
    pointer('windowmove', pet_native(), int(pos['x'] + rect['x'] - 150), int(pos['y'] + rect['y'] - 160))
    pats = invoke('companion_status')['pats']
    click('确认清空')
    wait(lambda: not snap()['messages'])
    check('clear chat retains saved personality', snap()['personality']['owner_name'] == '小木')
    check('clicking chat over an occluded pet does not pet the dog through the window', invoke('companion_status')['pats'] == pats)
    OUT.joinpath('e2e.json').write_text(json.dumps({'passed': checks, 'timings': timings, 'scope': 'Native Linux text-only developer slice; not a Windows/macOS or consumer hardware benchmark'}, ensure_ascii=False, indent=2))
finally:
    error = sys.exc_info()[1]
    if error:
        OUT.joinpath('e2e.json').write_text(json.dumps({'passed': checks, 'error': str(error)}, ensure_ascii=False, indent=2))
        screenshot('failure-desktop.png')
    if session:
        try:
            command('DELETE', '')
        except Exception:
            pass
