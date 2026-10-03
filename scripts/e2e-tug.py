#!/usr/bin/env python3
"""Real Tauri/X11 mouse input for rope grabbing, resisting, release and cleanup."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])

def phase():
    return js("return document.querySelector('[data-testid=tug-phase]')?.dataset.phase")

def screenshot(name):
    subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)

try:
    new_session()
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalWidth===1536"))
    invoke('set_encounters', {'enabled': False})
    before=set(command('GET','/window/handles'))
    invoke('open_playground')
    panel=wait(lambda: next(iter(set(command('GET','/window/handles'))-before),None))
    command('POST','/window',{'handle':panel})
    wait(lambda: js("return !!document.querySelector('.play-panel')"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'playground'}))
    def panel_window():
        candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
        return next((w for w in candidates if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True)),None)
    panel_native=wait(panel_window)
    pointer('windowmove',panel_native,870,30)
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js('window.__tugFrames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__tugFrames.push(e.payload))')})
    click('拔河');click('拿出绳子');wait(lambda: phase()=='tug_ready')
    initial=invoke('play_status')
    check('rope button opens independent native toy',initial['toy']=='rope' and initial['tug'] is not None and invoke('plugin:window|is_visible',{'label':'toy'}))
    for turn in range(2):
        invoke('play_action',{'action':'cancel'})
        native=pet_native();pointer('windowmove',native,70 if turn==0 else 650,320);time.sleep(.25)
        js('window.__tugFrames=[];return true')
        click('拿出绳子');wait(lambda:phase()=='tug_ready')
        rope=invoke('play_status')['tug']
        check(f'round {turn+1}: intended rope direction',rope['right']==(turn==0))
        pointer('mousemove',*rope['handle']);time.sleep(.15);pointer('mousedown',1)
        wait(lambda:phase()=='tugging')
        check(f'round {turn+1}: OS mouse grabs rope')
        native=pet_native()
        before_position=subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True)
        direction=1 if rope['right'] else -1
        pointer('mousemove',rope['mouth'][0]+direction*155,rope['mouth'][1])
        time.sleep(2.1)
        check(f'round {turn+1}: tension and pet resistance are real',invoke('play_status')['tug']['tension']>=20 and before_position!=subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True))
        frames=js('return window.__tugFrames');row=23 if rope['right'] else 24
        check(f'round {turn+1}: planted bite poses rendered',{f['col'] for f in frames if f['row']==row}=={0,1,2,3})
        screenshot(f'tug-pulling-{turn+1}')
        pointer('mouseup',1)
        wait(lambda:phase()=='tug_done')
        current=invoke('play_status')
        check(f'round {turn+1}: release completes exactly once',current['tug_rounds']==initial['tug_rounds']+turn+1 and current['catches']==initial['catches'])
        check(f'round {turn+1}: rope hides for happy response',not invoke('plugin:window|is_visible',{'label':'toy'}))
        wait(lambda:phase()=='tug_ready')
        check(f'round {turn+1}: ready for reuse',invoke('plugin:window|is_visible',{'label':'toy'}))
    click('飞盘');click('拿出飞盘');wait(lambda:invoke('play_status')['toy']=='frisbee')
    check('switching games removes rope',invoke('play_status')['tug'] is None)
    click('拔河');click('拿出绳子');wait(lambda:phase()=='tug_ready')
    invoke('set_pomodoro',{'active':True});wait(lambda:phase()=='off')
    check('focus removes native rope',not invoke('plugin:window|is_visible',{'label':'toy'}))
    invoke('set_pomodoro',{'active':False});click('拿出绳子');wait(lambda:phase()=='tug_ready')
    invoke('set_visible',{'visible':False});wait(lambda:phase()=='off')
    check('hide clears rope',invoke('play_status')['tug'] is None and not invoke('plugin:window|is_visible',{'label':'toy'}))
except Exception:
    pointer('mouseup',1)
    screenshot('failure')
    raise
finally:
    error=sys.exc_info()[1]
    OUT.joinpath('tug-e2e.json').write_text(json.dumps({'passed':checks,'error':str(error) if error else None},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
