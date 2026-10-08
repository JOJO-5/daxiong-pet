#!/usr/bin/env python3
"""Real pointer contact, body zones, rolling and interruption in native Tauri."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])

def touch():
    return js("const e=document.querySelector('[data-testid=petting-feedback]');return e?{kind:e.dataset.kind,phase:e.dataset.phase}:null")

def pet_point(x,y):
    pointer('mousemove','--window',native,x,y)

def shoot(name):
    subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)

try:
    new_session();main=command('GET','/window')
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}))
    native=pet_native();pointer('windowmove',native,380,260);pointer('mousemove',30,30)
    invoke('set_quiet_companion',{'enabled':True});invoke('set_encounters',{'enabled':False})
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js('window.__touchFrames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__touchFrames.push(e.payload))')})
    before=invoke('companion_status')['pats']
    pet_point(145,125)
    wait(lambda:touch() and touch()['phase']=='waiting')
    check('head contact shows a dwell cue before committing')
    wait(lambda:touch() and touch()['phase']=='head')
    check('head contact uses original dedicated animation',js("return document.querySelector('[data-testid=pet]').dataset.row==='11'"))
    wait(lambda:invoke('companion_status')['pats']==before+1)
    check('one genuine touch contributes companionship once')
    for x in [142,146,150,146,142,138,142,146]:
        pet_point(x,125);time.sleep(.14)
    time.sleep(1.5);shoot('head-rub')
    check('gentle head strokes retain the pose and animate multiple frames',len(set(v['col'] for v in js('return window.__touchFrames') if v['row']==11))>=6)
    check('head overlay is decorative and never captures drag input',js("return getComputedStyle(document.querySelector('[data-testid=petting-feedback]')).pointerEvents==='none'"))
    check('holding contact does not farm repeated rewards',invoke('companion_status')['pats']==before+1)
    pointer('mousemove',30,30);wait(lambda:touch() is None)
    wait(lambda:js("return document.querySelector('[data-testid=pet]').dataset.row!=='11'"));time.sleep(3)
    pet_point(178,199)
    wait(lambda:touch() and touch()['phase']=='down',10)
    check('belly contact begins the roll instead of head animation',js("return document.querySelector('[data-testid=pet]').dataset.row==='26'"))
    time.sleep(.55);pet_point(172,145)
    wait(lambda:touch() and touch()['phase']=='belly')
    check('contact follows exposed belly after rolling',js("return document.querySelector('[data-testid=pet]').dataset.row==='25'"))
    js('window.__touchFrames=[];return true')
    for x in [169,173,177,173,169,165,169,173]:
        pet_point(x,145);time.sleep(.16)
    time.sleep(1.6);shoot('belly-rub')
    check('belly kick and tail loop visits its animation frames',len(set(v['col'] for v in js('return window.__touchFrames') if v['row']==25))>=7)
    pet_point(110,207);time.sleep(.25)
    wait(lambda:touch() and touch()['kind']=='head')
    check('head can be stroked while lying without snapping upright',js("return document.querySelector('[data-testid=pet]').dataset.row==='25'"));shoot('lying-head')
    pointer('mousemove',30,30)
    wait(lambda:js("const p=document.querySelector('[data-testid=pet]');return p.dataset.row==='26'&&Number(p.dataset.col)>=4"),4)
    check('moving away plays the roll-up sequence')
    wait(lambda:js("return !['25','26'].includes(document.querySelector('[data-testid=pet]').dataset.row)"))
    shoot('standing-again');time.sleep(1)
    pet_point(178,199);wait(lambda:touch() and touch()['phase']=='down',10)
    pointer('mousedown',1);time.sleep(.15)
    check('pressing cancels belly feedback before a drag',touch() is None)
    pointer('mousemove_relative','--',50,0);time.sleep(.5)
    check('press and movement still drag the pet',js("return document.querySelector('[data-testid=pet]').dataset.row==='6'"))
    pointer('mouseup',1);pointer('mousemove',30,30);time.sleep(3)
    pet_point(178,199);wait(lambda:touch() and touch()['phase']=='down',10)
    invoke('set_pomodoro',{'active':True});wait(lambda:touch() is None)
    check('focus interrupts an active belly rub')
    invoke('set_pomodoro',{'active':False});pointer('mousemove',30,30);time.sleep(3)
    invoke('play_action',{'action':'show'});pet_point(145,125);time.sleep(1.8)
    check('showing a toy suppresses accidental body petting',touch() is None)
    invoke('play_action',{'action':'cancel'})
    fixture.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/'public/spritesheet.webp',fixture/'spritesheet.webp')
    (fixture/'pet.json').write_text(json.dumps({'id':'e2e-legacy','displayName':'E2E Legacy','spriteVersionNumber':2,'spritesheetPath':'spritesheet.webp'}))
    invoke('rescan_pets');invoke('set_pet',{'id':'e2e-legacy'})
    wait(lambda:js("return document.querySelector('.pet-sheet').naturalHeight===2288"));pointer('mousemove',30,30);time.sleep(4)
    pet_point(178,199);wait(lambda:touch() and touch()['kind']=='belly',10)
    wait(lambda:touch() and touch()['phase']=='belly',10)
    check('legacy pet gives feedback within its original rows',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)<11"))
    invoke('set_pet',{'id':'__builtin__'});wait(lambda:js("return document.querySelector('.pet-sheet').naturalHeight===5616"));pointer('mousemove',30,30)
    wait(lambda:touch() is None)
    check('switching pets clears the active touch state')
    (OUT/'result.json').write_text(json.dumps({'ok':True,'checks':checks},ensure_ascii=False,indent=2))
except Exception:
    shoot('failure')
    print('DIAGNOSTIC',json.dumps({'touch':touch(),'frames':js('return window.__touchFrames.slice(-25)'),'pointer':subprocess.check_output(['xdotool','getmouselocation','--shell'],text=True),'geometry':subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True)},ensure_ascii=False),flush=True)
    raise
finally:
    if session:command('DELETE','')
