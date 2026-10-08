#!/usr/bin/env python3
"""Actual native buttons, baked eating frames, rewards and snack completion."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
def shoot(name):subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)
def clear_frames():js('window.__frames=[];return true')
def park_panel():
    geometry=dict(line.split('=') for line in subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True).strip().splitlines())
    pointer('windowmove',panel_native,870 if int(geometry['X'])<640 else 30,30)
def eating(name):
    wait(lambda: any(f['row']==20 for f in js('return window.__frames')),5)
    time.sleep(.5);shoot(name+'-bite')
    wait(lambda: {f['col'] for f in js('return window.__frames') if f['row']==20}==set(range(8)),5)
    check(name+': all eight taking, biting, chewing and licking frames rendered')
    command('POST','/window',{'handle':main})
    check(name+': no second floating biscuit overlay',js("return !document.querySelector('[data-testid=treat-cookie]')"))
    command('POST','/window',{'handle':panel})
try:
    new_session();main=command('GET','/window')
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    invoke('set_encounters',{'enabled':False})
    initial=set(command('GET','/window/handles'));invoke('open_playground');time.sleep(1.2)
    panel=next(iter(set(command('GET','/window/handles'))-initial));command('POST','/window',{'handle':panel})
    candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    native=next(w for w in candidates if 'WIDTH=300' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    panel_native=next(w for w in candidates if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30);pointer('mousemove',30,30)
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js('window.__frames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__frames.push(e.payload))')})
    if os.environ.get('E2E_TREAT_SCOPE','visual')=='visual':
        before=invoke('companion_status');park_panel();clear_frames();click('喂一块饼干');pointer('mousemove',30,30)
        eating('feeding')
        after=invoke('companion_status');check('normal feeding saves exactly one biscuit',after['treats']==before['treats']+1)
        try:invoke('feed_treat');raise AssertionError('cooldown bypassed')
        except AssertionError as e:check('cooldown rejects repeated feeding','饼干' in str(e))
        click('小指令');native_click("document.querySelector('[data-testid=trick-stay]')");pointer('mousemove',30,30)
        wait(lambda: invoke('trick_status')['rewardable'])
        wait(lambda: js("return [...document.querySelectorAll('button')].some(b=>b.textContent==='奖励这次指令'&&!b.disabled)"),25)
        park_panel();clear_frames();click('奖励这次指令');pointer('mousemove',30,30);eating('reward')
        check('training reward saves one learned practice',invoke('companion_status')['training'][3]==1)
        click('找零食');click('藏一块零食');clear_frames();click('开始寻找');pointer('mousemove',30,30)
        wait(lambda: invoke('trick_status')['phase']=='found',15);eating('found')
        check('found snack eats without increasing feeding counters',invoke('companion_status')['treats']==after['treats']+1)
        invoke('set_visible',{'visible':False})
    else:
        click('找零食');click('藏一块零食');clear_frames();click('开始寻找');pointer('mousemove',30,30)
        wait(lambda: any(f['row']==20 for f in js('return window.__frames')),15)
        invoke('set_pomodoro',{'active':True});clear_frames()
        wait(lambda: any(f['row']!=20 for f in js('return window.__frames')),5)
        check('focus immediately interrupts eating');invoke('set_pomodoro',{'active':False})
        fixture.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/'public/spritesheet.webp',fixture/'spritesheet.webp')
        (fixture/'pet.json').write_text(json.dumps({'id':'e2e-legacy','displayName':'E2E Legacy','spriteVersionNumber':2,'spritesheetPath':'spritesheet.webp'}))
        invoke('rescan_pets');invoke('set_pet',{'id':'e2e-legacy'})
        command('POST','/window',{'handle':main});wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===2288"))
        command('POST','/window',{'handle':panel})
        wait(lambda: invoke('companion_status')['treat_wait']==0,25)
        wait(lambda: js("return !!document.querySelector('[data-testid=quick-feed]:not(:disabled)')"),5)
        park_panel();clear_frames();click('喂一块饼干');pointer('mousemove',30,30)
        command('POST','/window',{'handle':main});wait(lambda:js("return !!document.querySelector('[data-testid=treat-cookie]')"),3)
        check('legacy pet keeps supported reaction and cookie fallback',js("return Number(document.querySelector('[data-testid=pet]').dataset.row)<11"))
        command('POST','/window',{'handle':panel});invoke('set_pet',{'id':'__builtin__'});invoke('set_visible',{'visible':False})

except Exception:
    shoot('failure');raise
finally:
    error=sys.exc_info()[1]
    OUT.joinpath('treat-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'error':str(error) if error else None},ensure_ascii=False,indent=2))
    if fixture.exists():shutil.rmtree(fixture)
    if session:
        try:command('DELETE','')
        except Exception:pass
