#!/usr/bin/env python3
"""Real desktop validation for the four playful interaction versions."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
frames=[]
scope=os.environ.get("E2E_FUN_SCOPE","all")
def click_id(value):
    element=command('POST','/element',{'using':'css selector','value':f'[data-testid={value}]'})['element-6066-11e4-a52e-4f735466cecf']
    command('POST',f'/element/{element}/click',{})
def geometry():
    g=dict(line.split('=') for line in subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True).strip().splitlines())
    return (int(g['X']),int(g['Y']))
def phase(): return js("return document.querySelector('[data-testid=play-phase]').dataset.phase")
def shoot(name): subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)
try:
    new_session();main=command('GET','/window')
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===4160"))
    invoke('set_encounters',{'enabled':False})
    initial=set(command('GET','/window/handles'));invoke('open_playground');time.sleep(1.2)
    wait(lambda: len(command('GET','/window/handles'))>len(initial))
    panel=next(iter(set(command('GET','/window/handles'))-initial))
    time.sleep(.3)
    command('POST','/window',{'handle':panel})
    native_windows=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    native=next(w for w in native_windows if 'WIDTH=300' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    panel_native=next(w for w in native_windows if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30);pointer('mousemove',30,30)
    invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js("window.__frames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__frames.push(e.payload))")})
    if scope=="all":
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
    if int(json.loads(ROOT.joinpath('package.json').read_text())['version'].split('.')[-1])>=13 and scope=='all':
        invoke('set_visible',{'visible':True});wait(lambda: invoke('plugin:window|is_visible',{'label':'main'}))
        for turn,style,row in [(1,'near',4),(2,'far',11),(3,'far',15)]:
            if phase()=='off': invoke('play_action',{'action':'show'})
            ball=wait(lambda: (v['ball'] if (v:=invoke('play_status'))['phase'] in ('ready','returned') else None))
            geom=dict(line.split('=') for line in subprocess.check_output(['xdotool','getwindowgeometry','--shell',native],text=True).strip().splitlines())
            wx,wy=int(geom['X']),int(geom['Y'])
            tx=min(1200,wx+210) if style=='near' else (100 if wx+150>600 else 1170)
            ty=min(760,wy+226)
            pointer('mousemove',*ball);pointer('mousedown',1);time.sleep(.12)
            check(f'{style}: real returned ball grabbed',phase()=='held')
            pointer('mousemove',tx,ty);time.sleep(.2);pointer('mouseup',1)
            wait(lambda: invoke('play_status')['style']==style)
            check(f'{style}: actual release selects distance feedback')
            wait(lambda: phase()=='returned',25)
            wait(lambda: any(f['row']==row for f in js('return window.__frames.slice(-20)')))
            check(f'fetch streak {turn}: celebration row {row} rendered',invoke('play_status')['streak']==turn)
            shoot(f'variety-{turn}')
        click('推回给我');wait(lambda: phase()=='rolling')
        before=invoke('play_status')['ball'];time.sleep(.2);after=invoke('play_status')['ball']
        check('push back moves ball horizontally',after[0]!=before[0] and after[1]==before[1])
        shoot('rolling')
        pointer('mousemove',*after);pointer('mousedown',1)
        wait(lambda: phase()=='held');check('rolling ball accepts real pointer grab')
        pointer('mouseup',1);wait(lambda: phase()=='returned',25)
        invoke('play_action',{'action':'cancel'})
    if int(json.loads(ROOT.joinpath('package.json').read_text())['version'].split('.')[-1])>=14:
        def trick_phase(): return js("return document.querySelector('[data-testid=trick-phase]').dataset.phase")
        for cue,expected in [('come',None),('spin',None),('down',12),('stay',6)]:
            before=geometry();js('window.__frames=[];return true');click_id(f'trick-{cue}')
            wait(lambda: trick_phase()=='attention');check(f'{cue}: looks at you before action')
            pointer('mousemove',400,350) if cue=='come' else pointer('mousemove',30,30)
            wait(lambda: trick_phase()=='performing')
            positions=[]
            while trick_phase()=='performing':
                positions.append(geometry());time.sleep(.1)
            wait(lambda: trick_phase()=='completed')
            actual=js('return window.__frames')
            if cue=='come': check('come physically approaches cursor',geometry()!=before)
            elif cue=='spin': check('spin physically circles and returns',len(set(positions))>5 and all(abs(a-b)<3 for a,b in zip(geometry(),before)))
            else: check(f'{cue}: existing pose rendered without moving',any(f['row']==expected for f in actual) and geometry()==before)
            check(f'{cue}: completed action can be rewarded',invoke('trick_status')['rewardable'])
            shoot(f'trick-{cue}')
        for n in range(1,4 if scope!="tricks" else 2):
            if n>1:
                click_id('trick-stay');pointer('mousemove',30,30);wait(lambda: trick_phase()=='completed',10)
            wait(lambda: invoke('companion_status')['treat_wait']==0,25)
            wait(lambda: js("return [...document.querySelectorAll('button')].some(b=>b.textContent==='奖励这次指令'&&!b.disabled)"),5)
            click('奖励这次指令')
            wait(lambda: invoke('companion_status')['training'][3]==n)
            check(f'practice {n}: treat saves skill progress')
            try:
                invoke('reward_trick');raise AssertionError('duplicate reward accepted')
            except AssertionError as e: check(f'practice {n}: repeated reward rejected','奖励' in str(e) or '饼干' in str(e))
        if scope!="tricks":
            wait(lambda: js("return document.querySelector('[data-testid=skill-stay]').textContent==='已学会'"))
            check('three rewarded practices visibly learn command');shoot('learned-stay')
            exec(ROOT.joinpath('scripts/e2e-memory.py').read_text().split('\nwait(lambda: js(')[0])
            restart_app()
            check('real restart preserves learned commands',invoke('companion_status')['training'][3]==3)
            click_id('trick-stay');pointer('mousemove',30,30);wait(lambda: trick_phase()=='performing')
            check('learned command executes after restart')
        invoke('set_pomodoro',{'active':True});wait(lambda: trick_phase()=='off')
        click_id('trick-spin');wait(lambda: trick_phase()=='blocked')
        check('focus rejects practice without reward',not invoke('trick_status')['rewardable'])
        invoke('set_pomodoro',{'active':False});click_id('trick-spin');wait(lambda: trick_phase()=='attention')
        click('结束练习');wait(lambda: trick_phase()=='off');check('manual stop cancels command')
finally:
    if sys.exc_info()[1]: shoot('failure')
    frames=js('return window.__frames') if session and not sys.exc_info()[1] else []
    OUT.joinpath('fun-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'frames':frames,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] else None},ensure_ascii=False,indent=2))
    if session: command('DELETE','')
