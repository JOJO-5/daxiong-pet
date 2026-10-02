#!/usr/bin/env python3
"""Native pointer throws in both directions, then inspect each carrying frame."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
samples=[]
try:
    new_session();main=command('GET','/window')
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===3744"))
    invoke('set_encounters',{'enabled':False})
    invoke('open_playground')
    handles=wait(lambda: (h if len(h:=command('GET','/window/handles'))>1 else None))
    panel=next(h for h in handles if h!=main)
    wait(lambda: invoke('plugin:window|is_visible',{'label':'playground'}));time.sleep(.7)
    command('POST','/window',{'handle':panel})
    candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    panel_native=next(w for w in candidates if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30)
    pointer('mousemove',30,30)
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js("window.__carryFrame=null;window.__carryFrames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>{window.__carryFrame=e.payload;window.__carryFrames.push(e.payload)})")})
    for target,expected,name in [(100,16,'right'),(1170,17,'left')]:
        invoke('play_action',{'action':'show'})
        ball=wait(lambda: (v['ball'] if (v:=invoke('play_status'))['phase']=='ready' else None))
        pointer('mousemove',*ball);time.sleep(.15);pointer('mousedown',1);time.sleep(.12)
        check(f'{name}: visible ball accepts native pointer grab',invoke('play_status')['phase']=='held')
        pointer('mousemove',target,660);time.sleep(.2);pointer('mouseup',1)
        wait(lambda: invoke('play_status')['phase']=='returning',25)
        columns=set();seen=set();shot=False
        while True:
            view=js("return {phase:document.querySelector('[data-testid=play-phase]').dataset.phase,frame:window.__carryFrame}")
            if view['phase']!='returning': break
            frame=view['frame']
            if frame is None: continue
            if frame['row']==expected:
                columns.add(frame['col']);seen.add(frame['row'])
                if not shot:
                    subprocess.run(['import','-window','root',str(OUT/f'carry-{name}.png')],check=True)
                    shot=True
            samples.append({'direction':name,'frame':frame})
            time.sleep(.08)
        check(f'{name}: dedicated closed-mouth cycle rendered',seen=={expected})
        columns={f['col'] for f in js('return window.__carryFrames') if f['row']==expected}
        check(f'{name}: all eight carrying frames exercised',columns==set(range(8)))
        check(f'{name}: return completes once',invoke('play_status')['phase']=='returned')
        invoke('play_action',{'action':'cancel'});time.sleep(1.5)
finally:
    OUT.joinpath('carry-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'samples':samples,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] else None},ensure_ascii=False,indent=2))
    if session: command('DELETE','')
