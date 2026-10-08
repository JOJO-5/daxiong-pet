#!/usr/bin/env python3
"""Record real ball/disc catches and atomic releases in both directions."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
rounds=[]
try:
    new_session();main=command('GET','/window')
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}));time.sleep(.65)
    invoke('set_encounters',{'enabled':False});invoke('set_playful_fetch',{'enabled':False})
    native=pet_native()
    js("""window.__handoff=[];function sample(){
      const pet=document.querySelector('[data-testid=pet]');
      const toy=document.querySelector('[data-testid=released-toy]');
      if(pet){const r=toy?.getBoundingClientRect();window.__handoff.push({at:Date.now(),
        row:Number(pet.dataset.row),col:Number(pet.dataset.col),toy:toy?.dataset.toy||null,
        x:r?r.x+r.width/2:null,y:r?r.y+r.height/2:null});}
      requestAnimationFrame(sample);}requestAnimationFrame(sample);return true;""")
    initial=set(command('GET','/window/handles'));invoke('open_playground');time.sleep(1.2)
    panel=next(iter(set(command('GET','/window/handles'))-initial));command('POST','/window',{'handle':panel})
    windows=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    panel_native=next(w for w in windows if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30)
    for toy in ('ball','frisbee'):
        for right in (True,False):
            name=f'{toy}-'+('right' if right else 'left')
            invoke('play_action',{'action':'cancel'});time.sleep(.15)
            pointer('windowmove',native,600 if right else 70,320);time.sleep(.25)
            invoke('play_action',{'action':'show_frisbee' if toy=='frisbee' else 'show'})
            ball=wait(lambda:(v['ball'] if (v:=invoke('play_status'))['phase']=='ready' else None))
            before=invoke('play_status')['catches'];start=int(time.time()*1000)
            pointer('mousemove',*ball);time.sleep(.12);pointer('mousedown',1)
            wait(lambda:invoke('play_status')['phase']=='held')
            check(f'{name}: real pointer grabs independent toy')
            pointer('mousemove',100 if right else 1170,550 if toy=='frisbee' else 660)
            time.sleep(.2);pointer('mouseup',1);pointer('mousemove',30,30)
            states=[];deadline=time.monotonic()+25
            while time.monotonic()<deadline:
                v=invoke('play_status');states.append(v['phase'])
                if v['phase']=='releasing':
                    check(f'{name}: independent toy hidden during atomic release',not invoke('plugin:window|is_visible',{'label':'toy'}))
                    # One assertion is sufficient; continue sampling without duplicates.
                    wait(lambda:invoke('play_status')['phase']=='returned',3);break
                if v['phase']=='returned':raise AssertionError('release was not observed')
                time.sleep(.015)
            else:raise AssertionError('native return timed out')
            check(f'{name}: return counted once',invoke('play_status')['catches']==before+1)
            check(f'{name}: returned toy visible and reusable',invoke('plugin:window|is_visible',{'label':'toy'}))
            command('POST','/window',{'handle':main})
            samples=js(f'return window.__handoff.filter(s=>s.at>={start})')
            rounds.append({'name':name,'states':states,'samples':samples})
            release=[s for s in samples if s['toy']]
            expected=18 if right else 19
            check(f'{name}: release and pose share every painted frame',len(release)>=8 and all(s['row']==expected and s['toy']==toy for s in release))
            check(f'{name}: starts at open jaws',abs(release[0]['y']-162.75)<2)
            falling=[s for s in release if s['y']>180]
            check(f'{name}: falling toy clears chest and front paws',bool(falling) and all((s['x']-18>222 if right else s['x']+18<78) for s in falling))
            carry=(21 if right else 22) if toy=='frisbee' else (16 if right else 17)
            check(f'{name}: biting sprite has no second in-window toy',any(s['row']==carry for s in samples) and all(s['toy'] is None for s in samples if s['row']==carry))
            check(f'{name}: no release overlay survives completion',js("return !document.querySelector('[data-testid=released-toy]')"))
            subprocess.run(['import','-window','root',str(OUT/f'{name}-returned.png')],check=True)
            command('POST','/window',{'handle':panel})
finally:
    error=sys.exc_info()[1]
    OUT.joinpath('handoff-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'rounds':rounds,'error':str(error) if error else None},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
