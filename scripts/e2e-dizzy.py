#!/usr/bin/env python3
"""Real native gaze gestures, pose/halo snapshots, cooldown and interruptions."""
import math
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])

def stars():
    return js("const e=document.querySelector('[data-testid=dizzy-stars]');return e?{phase:e.dataset.phase}:null")

def shoot(name):
    time.sleep(.12)
    subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)

def shake(stop=True):
    for i in range(18):
        pointer('mousemove','--window',native,35 if i%2==0 else 265,52)
        time.sleep(.085)
        if stop and stars():return True
    return stars() is not None

try:
    new_session()
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}))
    native=pet_native();pointer('windowmove',native,380,260);pointer('mousemove',30,30)
    invoke('set_quiet_companion',{'enabled':True});invoke('set_encounters',{'enabled':False})
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js('window.__dizzyFrames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__dizzyFrames.push(e.payload))')})
    before=invoke('companion_status')['pats']
    for i in range(28):
        pointer('mousemove','--window',native,35+i*8,52);time.sleep(.095)
    check('ordinary slow gaze does not show dizzy stars',stars() is None)
    pointer('mousemove',30,30);time.sleep(.3)
    check('rapid native left-right gaze triggers dizziness',shake())
    triggered=time.monotonic()
    check('stars begin with the head wobble',stars()['phase']=='wobble')
    shoot('wobble')
    wait(lambda:stars() and stars()['phase']=='down')
    shoot('side-lying')
    check('dizzy collapse reuses the reviewed side-lying pose',js("const e=document.querySelector('[data-testid=pet]');return e.dataset.row==='26'&&e.dataset.col==='2'"))
    check('three stars orbit around the head',js("return document.querySelectorAll('[data-testid=dizzy-stars] path').length===3"))
    check('star decoration never captures mouse input',js("return getComputedStyle(document.querySelector('[data-testid=dizzy-stars]')).pointerEvents==='none'"))
    wait(lambda:stars() and stars()['phase']=='recover');shoot('recovery')
    wait(lambda:stars() is None)
    check('complete collapse and rise visits the existing transition frames',set(v['col'] for v in js('return window.__dizzyFrames') if v['row']==26)>={0,1,2,5,6,7})
    check('all halo positions remain inside the native stage',js('return window.__dizzyFrames.filter(f=>f.dizzy).every(f=>f.dizzy.x>=36&&f.dizzy.x<=264&&f.dizzy.y>=19&&f.dizzy.y<=221)'))
    check('dizziness does not award petting affection',invoke('companion_status')['pats']==before)
    check('immediate repeated gestures respect the cooldown',not shake(False))
    pointer('mousemove',30,30)
    while time.monotonic()-triggered<15.5:time.sleep(.1)
    seen=False
    for i in range(32):
        a=i*.44
        pointer('mousemove','--window',native,round(150+130*math.cos(a)),round(162+130*math.sin(a)))
        time.sleep(.035)
        if stars():seen=True;break
    check('rapid native circular gaze also triggers after cooldown',seen)
    wait(lambda:stars() and stars()['phase']=='down');shoot('circle')
    pointer('mousemove','--window',native,145,165);pointer('mousedown',1);time.sleep(.12)
    check('pressing the pet immediately clears dizzy stars',stars() is None)
    pointer('mousemove_relative','--',45,0);time.sleep(.35)
    check('dizzy interruption preserves real pet dragging',js("return document.querySelector('[data-testid=pet]').dataset.row==='6'"))
    pointer('mouseup',1)
    wait(lambda:js("return document.querySelector('[data-testid=pet]').dataset.row==='3'"));pointer('mousemove',30,30);time.sleep(3)
    # Let the gesture cooldown expire before checking independent blockers.
    time.sleep(16)
    invoke('set_pomodoro',{'active':True})
    wait(lambda:js("return document.querySelector('[data-testid=pet]').dataset.row==='8'"))
    check('focus suppresses fast gaze gestures',not shake(False))
    invoke('set_pomodoro',{'active':False})
    wait(lambda:js("return ['9','10'].includes(document.querySelector('[data-testid=pet]').dataset.row)"))
    invoke('play_action',{'action':'show'});check('a visible toy suppresses dizzy gestures',not shake(False));invoke('play_action',{'action':'cancel'})
    fixture.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/'public/spritesheet.webp',fixture/'spritesheet.webp')
    (fixture/'pet.json').write_text(json.dumps({'id':'e2e-legacy','displayName':'E2E Legacy','spriteVersionNumber':2,'spritesheetPath':'spritesheet.webp'}))
    invoke('rescan_pets');invoke('set_pet',{'id':'e2e-legacy'})
    wait(lambda:js("return document.querySelector('.pet-sheet').naturalHeight===2288"));pointer('mousemove',30,30)
    check('legacy pet retains ordinary gaze within original atlas rows',not shake(False) and js("return Number(document.querySelector('[data-testid=pet]').dataset.row)<11"))
    invoke('set_pet',{'id':'__builtin__'})
    wait(lambda:js("return document.querySelector('.pet-sheet').naturalHeight===5616"));pointer('mousemove',30,30);time.sleep(.5)
    assert shake(),"expected an active builtin dizzy gesture before switching"
    invoke('set_pet',{'id':'e2e-legacy'})
    wait(lambda:js("return document.querySelector('.pet-sheet').naturalHeight===2288"))
    wait(lambda:stars() is None)
    check('switching pets clears an active dizzy overlay',stars() is None)
    (OUT/'result.json').write_text(json.dumps({'ok':True,'checks':checks},ensure_ascii=False,indent=2))
except Exception:
    shoot('failure')
    print('DIAGNOSTIC',json.dumps({'stars':stars(),'frames':js('return window.__dizzyFrames.slice(-20)')},ensure_ascii=False),flush=True)
    raise
finally:
    if session:command('DELETE','')
