#!/usr/bin/env python3
"""Real GTK context menu, OS hotkey, retained panel and persisted preferences."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
def shoot(name): subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)
menu_origin=None
def pet_menu():
    global menu_origin
    menu_origin=command('GET','/window')
    if menu_origin!=main:command('POST','/window',{'handle':main})
    # Re-enter the webview after GTK releases a popup's native pointer grab.
    pointer('mousemove',30,30);time.sleep(.15)
    # GTK/Openbox may finish placing the window after it first becomes visible.
    # Follow its actual position until the real cursor is inside the pet hit area.
    def hover_current_position():
        pointer('mousemove','--window',native,150,160)
        return js("return document.querySelector('[data-testid=pet]').dataset.clickable==='true'")
    wait(hover_current_position)
    time.sleep(.15);pointer('click',3);time.sleep(.4)
    # Changing the WebDriver window focuses its webview and can dismiss GTK's popup.
    # Restore the previous inspection target only after the native key action.
def finish_menu(*keys):
    pointer('key',*keys);time.sleep(.3)
    if menu_origin!=main:command('POST','/window',{'handle':menu_origin})
def choose(index):
    finish_menu('Home',*(['Down']*index),'Return')
def visible(): return invoke('plugin:window|is_visible',{'label':'playground'})
try:
    new_session();main=command('GET','/window')
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    invoke('set_encounters',{'enabled':False});time.sleep(.7)
    native=pet_native()
    initial=set(command('GET','/window/handles'))
    pet_menu();shoot('native-menu');finish_menu('Escape');time.sleep(.2)
    check('Escape dismisses native menu without starting a game',invoke('play_status')['phase']=='off' and set(command('GET','/window/handles'))==initial)
    pet_menu();choose(0)
    wait(lambda:len(command('GET','/window/handles'))>len(initial))
    panel=next(iter(set(command('GET','/window/handles'))-initial));command('POST','/window',{'handle':panel})
    wait(lambda:js("return !!document.querySelector('.play-panel')"))
    check('right-click first item opens real panel',visible())
    # Cache the persistent panel before moving/disappearing toy and tooltip windows.
    def find_native_panel():
        found=subprocess.run(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],capture_output=True,text=True)
        for candidate in found.stdout.splitlines():
            geometry=subprocess.run(['xdotool','getwindowgeometry','--shell',candidate],capture_output=True,text=True)
            if geometry.returncode==0 and 'WIDTH=360' in geometry.stdout:return candidate
        return None
    panel_native=wait(find_native_panel)
    check('common games and feed visible while settings collapsed',js("const f=[...document.querySelectorAll('button')].find(b=>b.textContent==='喂一块饼干').getBoundingClientRect();return !document.querySelector('details').open&&f.bottom<innerHeight&&document.querySelectorAll('.game-picker button').length===5"))
    time.sleep(.8);shoot('compact-panel')
    pos=invoke('plugin:window|outer_position',{'label':'playground'});size=invoke('plugin:window|outer_size',{'label':'playground'})
    check('nearby panel remains inside screen',pos['x']>=0 and pos['y']>=0 and pos['x']+size['width']<=1280 and pos['y']+size['height']<=800)
    pet_menu();choose(3)
    wait(lambda:js("return document.querySelector('[data-testid=play-phase]')?.dataset.phase==='chasing'"))
    check('native frisbee item routes existing panel to frisbee',js("return [...document.querySelectorAll('.game-picker button')].find(b=>b.textContent==='飞盘').getAttribute('aria-pressed')==='true'") and invoke('play_status')['toy']=='frisbee')
    pointer('windowactivate','--sync',panel_native);pointer('key','Escape');wait(lambda:not visible())
    wait(lambda:invoke('play_status')['phase']=='returned',25)
    check('closing panel retains ongoing frisbee and reusable return',invoke('play_status')['catches']==1)
    pet_menu();choose(0);wait(visible)
    check('reopening retains selected game and completed phase',js("return document.querySelector('[data-testid=play-phase]').dataset.phase==='returned'"))
    pet_menu();choose(1);wait(lambda:invoke('companion_status')['treats']==1)
    check('native feeding commits exactly once and shows cooldown',js("return [...document.querySelectorAll('button')].some(b=>b.disabled&&b.textContent.includes('还在嚼'))"))
    pet_menu();shoot('cooldown-menu');finish_menu('Escape')
    open_more();check('shortcut defaults off',not invoke('shortcut_status')['enabled'])
    native_click("document.querySelector('[data-testid=shortcut-switch]')")
    wait(lambda:invoke('shortcut_status')['enabled']);check('real checkbox registers OS shortcut')
    shoot('preferences')
    native_click("document.querySelector('.panel-close')")
    wait(lambda:not visible());invoke('set_visible',{'visible':False});time.sleep(.3)
    pointer('key','ctrl+alt+p');wait(visible)
    check('actual OS hotkey restores hidden pet and panel',invoke('plugin:window|is_visible',{'label':'main'}))
    config=Path(os.environ['XDG_CONFIG_HOME'])/'com.jojo.daxiongpet'/'config.json'
    check('shortcut preference written to disk',json.loads(config.read_text())['shortcut_enabled'])
    for edge,(x,y) in [('left',(0,0)),('right',(980,500))]:
        native_click("document.querySelector('.panel-close')");wait(lambda:not visible())
        pointer('windowmove',native,x,y);time.sleep(.2);pet_menu();choose(0);wait(visible)
        p=invoke('plugin:window|outer_position',{'label':'playground'});s=invoke('plugin:window|outer_size',{'label':'playground'})
        check(f'{edge} edge opening clamps panel inside work area',p['x']>=0 and p['y']>=0 and p['x']+s['width']<=1280 and p['y']+s['height']<=800)
    open_more()
    if os.environ.get('E2E_RECORD','0')!='1':
        exec(ROOT.joinpath('scripts/e2e-memory.py').read_text().split('\nopen_more()\nwait(lambda: js(')[0])
        restart_app();check('real restart registers saved shortcut',invoke('shortcut_status')['enabled'])
        native_click("document.querySelector('[data-testid=shortcut-switch]')");wait(lambda:not invoke('shortcut_status')['enabled'])
        time.sleep(.8)
        blocker=subprocess.Popen([sys.executable,'-u','-c',"""import ctypes,sys
x=ctypes.CDLL('libX11.so.6');x.XOpenDisplay.restype=ctypes.c_void_p;x.XDefaultRootWindow.argtypes=[ctypes.c_void_p];x.XDefaultRootWindow.restype=ctypes.c_ulong;x.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong];x.XGrabKey.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_uint,ctypes.c_ulong,ctypes.c_int,ctypes.c_int,ctypes.c_int];x.XSync.argtypes=[ctypes.c_void_p,ctypes.c_int]
d=x.XOpenDisplay(None);key=x.XKeysymToKeycode(d,ord('p'));x.XGrabKey(d,key,12,x.XDefaultRootWindow(d),0,1,1);x.XSync(d,0);print('ready',flush=True);sys.stdin.read()
"""],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        try:
            assert blocker.stdout.readline().strip()=='ready'
            native_click("document.querySelector('[data-testid=shortcut-switch]')")
            wait(lambda:js("return document.querySelector('[data-testid=preferences-panel] [role=alert]')?.textContent.includes('未生效')"))
            check('real OS shortcut conflict is visible and does not claim activation',not invoke('shortcut_status')['enabled'] and not invoke('shortcut_status')['requested'])
        finally:blocker.terminate();blocker.wait(timeout=5)
        saved=config.read_bytes();config.unlink();config.mkdir()
        try:
            native_click("document.querySelector('[data-testid=shortcut-switch]')")
            wait(lambda:js("return document.querySelector('[data-testid=preferences-panel] [role=alert]')?.textContent.includes('保存失败')"))
            check('disk failure visibly reported and registration rolled back',not invoke('shortcut_status')['enabled'] and not invoke('shortcut_status')['requested'])
        finally:config.rmdir();config.write_bytes(saved)
        native_click("document.querySelector('[data-testid=shortcut-switch]')");wait(lambda:invoke('shortcut_status')['enabled'])
        native_click("document.querySelector('[data-testid=shortcut-switch]')");wait(lambda:not invoke('shortcut_status')['enabled'])
        check('shortcut can recover and unregister after error',not json.loads(config.read_text())['shortcut_enabled'])
    invoke('set_pomodoro',{'active':False});invoke('play_action',{'action':'cancel'});click('飞盘');pointer('mousemove',30,30)
    invoke('set_encounters',{'enabled':True})
    wait(lambda:invoke('encounter_status')['kind']=='ball',35)
    wait(lambda:js("return [...document.querySelectorAll('.game-picker button')].find(b=>b.textContent==='接球').getAttribute('aria-pressed')==='true'"))
    check('natural ball invitation routes an existing frisbee panel to fetch',js("return [...document.querySelectorAll('.game-picker button')].find(b=>b.textContent==='接球').getAttribute('aria-pressed')==='true'"))
    invoke('set_encounters',{'enabled':False})
    pet_menu();finish_menu('End','Up','Return');wait(lambda:invoke('play_status')['phase']=='off')
    check('native stop removes invitation and independent toy',not invoke('plugin:window|is_visible',{'label':'toy'}))
    pet_menu();finish_menu('End','Up','Up','Return');time.sleep(.3);wait(lambda:js("return document.querySelector('[data-testid=trick-phase]')?.dataset.phase&&['attention','performing','completed'].includes(document.querySelector('[data-testid=trick-phase]').dataset.phase)"))
    check('native come item selects and starts real command',invoke('trick_status')['kind']=='come')
    pet_menu();finish_menu('End','Up','Return');wait(lambda:invoke('trick_status')['phase']=='off')
    check('native stop cancels command without a reward',not invoke('trick_status')['rewardable'])
    invoke('set_pomodoro',{'active':True});time.sleep(.3)
    try:invoke('play_action',{'action':'throw_frisbee'});raise AssertionError('focus accepted')
    except AssertionError as e:check('focus rejects toy with explicit explanation','专注' in str(e))
except Exception:shoot('failure');raise
finally:
    error=sys.exc_info()[1];OUT.joinpath('menu-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'error':str(error) if error else None},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
