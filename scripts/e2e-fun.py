#!/usr/bin/env python3
"""Real desktop validation for the four playful interaction versions."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
frames=[]
def phase(): return invoke('play_status')['phase']
def shoot(name): subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)
try:
    new_session();main=command('GET','/window')
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===4160"))
    invoke('set_encounters',{'enabled':False})
    initial=set(command('GET','/window/handles'));invoke('open_playground')
    wait(lambda: len(command('GET','/window/handles'))>len(initial))
    panel=next(iter(set(command('GET','/window/handles'))-initial))
    wait(lambda: invoke('plugin:window|is_visible',{'label':'playground'}));time.sleep(.7)
    command('POST','/window',{'handle':panel})
    native_windows=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    native=next(w for w in native_windows if 'WIDTH=300' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    panel_native=next(w for w in native_windows if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30);pointer('mousemove',30,30)
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js("window.__frames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__frames.push(e.payload))")})
    for turn in range(1,7):
        click('抛一球')
        wait(lambda: phase() in ('returning','teasing'),25)
        if turn%3==0:
            wait(lambda: phase()=='teasing',15)
            check(f'catch {turn}: real third fetch invites chase')
            check(f'catch {turn}: only sprite carries ball',not invoke('plugin:window|is_visible',{'label':'toy'}))
            shoot(f'tease-{turn}')
            if turn==3: click('放下球')
            else: pointer('mousemove','--window',native,150,160)
        wait(lambda: phase()=='returned',25)
        check(f'catch {turn}: ball released exactly once',invoke('play_status')['catches']==turn)
        pointer('mousemove',30,30)
    invoke('set_playful_fetch',{'enabled':False})
    check('invitation preference saves to local memory',not invoke('companion_status')['playful_fetch'])
    invoke('play_action',{'action':'cancel'});click('抛一球')
    wait(lambda: phase()=='returned',25)
    check('disabled playful fetch still returns usable ball')
    invoke('set_visible',{'visible':False});wait(lambda: phase()=='off')
    check('hiding cleans toy and play',not invoke('plugin:window|is_visible',{'label':'toy'}))
finally:
    frames=js('return window.__frames') if session and not sys.exc_info()[1] else []
    OUT.joinpath('fun-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'frames':frames,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] else None},ensure_ascii=False,indent=2))
    if session: command('DELETE','')
