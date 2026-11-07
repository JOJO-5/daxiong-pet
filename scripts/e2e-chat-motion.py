#!/usr/bin/env python3
"""Native preferences and motion rendering; model events are synthetic fixtures."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
geometry_observations=[]
def native_geometry():
    return {'tauri_position':invoke('plugin:window|outer_position',{'label':'main'}),
            'tauri_size':invoke('plugin:window|outer_size',{'label':'main'}),
            'x11':subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True),
            'pointer':subprocess.check_output(['xdotool','getmouselocation','--shell'],text=True),
            'frame':js("return {...document.querySelector('[data-testid=pet]').dataset}"),
            'mouse_follow':invoke('companion_status')['mouse_follow']}
try:
    new_session();main=command('GET','/window')
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    invoke('set_quiet_companion',{'enabled':True});invoke('set_encounters',{'enabled':False})
    invoke('open_playground');time.sleep(1)
    panel=next(h for h in command('GET','/window/handles') if h!=main)
    command('POST','/window',{'handle':panel})
    open_more()
    wait(lambda:js("return document.querySelector('[data-testid=mouse-follow]')&&!document.querySelector('[data-testid=mouse-follow]').disabled"))
    # Scroll before sampling native click coordinates; WebKit can apply the
    # container scroll after the first execute/sync returns.
    js("document.querySelector('[data-testid=mouse-follow]').scrollIntoView({block:'center'})")
    wait(lambda:js("const r=document.querySelector('[data-testid=mouse-follow]').getBoundingClientRect();return r.width>0&&r.top>=0&&r.bottom<=innerHeight"))
    check('mouse follow is enabled by default',js("return document.querySelector('[data-testid=mouse-follow]').checked"))
    native_click("document.querySelector('[data-testid=mouse-follow]')")
    wait(lambda:not invoke('companion_status')['mouse_follow'])
    check('native toggle persists mouse follow off',not js("return document.querySelector('[data-testid=mouse-follow]').checked"))
    command('POST','/window',{'handle':main})
    native=pet_native();geometry_observations.append({'stage':'before-placement',**native_geometry()})
    # WebKitWebDriver can leave GTK's cached geometry at (0,0),1x1 while
    # Openbox already displays a 300x240 window elsewhere. A real native move
    # delivers ConfigureNotify; require both APIs to agree before testing gaze.
    pointer('windowmove',native,380,300)
    wait(lambda:invoke('plugin:window|outer_position',{'label':'main'})=={'x':380,'y':300}
         and invoke('plugin:window|outer_size',{'label':'main'})=={'width':300,'height':240})
    geometry_observations.append({'stage':'placed',**native_geometry()})
    geometry=subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True)
    g=dict(line.split('=',1) for line in geometry.splitlines() if '=' in line)
    check('native and Tauri geometry agree before gaze input',int(g['X'])==380 and int(g['Y'])==300)
    pointer('mousemove',int(g['X'])+260,int(g['Y'])+30)
    wait(lambda:js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0,5)
    check('disabled mouse follow leaves idle pose',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0)
    invoke('set_mouse_follow',{'enabled':True})
    wait(lambda:invoke('companion_status')['mouse_follow'])
    wait(lambda:js("return [9,10].includes(Number(document.querySelector('[data-testid=pet]').dataset.row))"),5)
    check('enabled mouse follow turns toward pointer',js("return [9,10].includes(Number(document.querySelector('[data-testid=pet]').dataset.row))"))
    invoke('set_mouse_follow',{'enabled':False});pointer('mousemove',30,30)
    invoke('plugin:event|emit',{'event':'ai:started','payload':{'preview':False}})
    wait(lambda:js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==8)
    wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label')==='让我想想…'"));time.sleep(.4)
    subprocess.run(['import','-window','root',str(OUT/'thinking.png')],check=True)
    check('chat start renders thinking pose',True)
    time.sleep(2)
    check('thinking returns to idle between small motions',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0)
    invoke('plugin:event|emit',{'event':'ai:reply','payload':{'content':'我在这里陪你，把这份笔记慢慢写完就好，我们一起整理今天的见闻。','status':'completed','preview':False,'error':None}})
    wait(lambda:js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==15)
    time.sleep(.65)
    subprocess.run(['import','-window','root',str(OUT/'reply.png')],check=True)
    check('AI reply renders original-identity affectionate motion',True)
    time.sleep(2)
    check('reply returns naturally to idle',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0)
    time.sleep(4)
    check('ordinary long reply stays calm without periodic celebrations',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0)
    invoke('plugin:event|emit',{'event':'ai:cleared','payload':None})
    wait(lambda:js("return Number(document.querySelector('[data-testid=pet]').dataset.row)")==0)
    check('clearing chat stops reply motion',True)
finally:
    if session and 'native' in globals():
        try:geometry_observations.append({'stage':'final',**native_geometry()})
        except Exception as error:geometry_observations.append({'diagnostic_error':str(error)})
    OUT.joinpath('chat-motion-geometry.json').write_text(json.dumps(geometry_observations,ensure_ascii=False,indent=2))
    OUT.joinpath('chat-motion-e2e.json').write_text(json.dumps({'passed':checks,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] else None},ensure_ascii=False,indent=2))
    if session:command('DELETE','')
