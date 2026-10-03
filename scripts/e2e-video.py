#!/usr/bin/env python3
"""Record a continuous native interaction session, including both real encounters."""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
segments=[]
start=time.monotonic()
def mark(name):
    segments.append({'scene':name,'seconds':round(time.monotonic()-start,1)})
    print('SCENE:',segments[-1],flush=True)
try:
    new_session()
    main=command('GET','/window')
    wait(lambda: js("return !!document.querySelector('.pet-sheet')?.naturalWidth"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}));time.sleep(.65)
    invoke('set_encounters',{'enabled':False})
    invoke('open_playground')
    wait(lambda:invoke('plugin:window|is_visible',{'label':'playground'}));time.sleep(1.2)
    handles=wait(lambda: (h if len(h:=command('GET','/window/handles'))>1 else None))
    panel=next(h for h in handles if h!=main)
    wait(lambda: invoke('plugin:window|is_visible',{'label':'playground'}))
    time.sleep(.7)
    command('POST','/window',{'handle':panel})
    candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    panel_native=next(w for w in candidates if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    pointer('windowmove',panel_native,870,30)
    pointer('mousemove',30,30)
    mark('throw-chase-return')
    click('抛一球')
    wait(lambda: invoke('play_status')['phase']=='returned',25)
    check('recorded real fetch return')
    time.sleep(2)
    click('收起玩具')
    mark('nickname-and-treat');open_more()
    field=command('POST','/element',{'using':'css selector','value':'#nickname'})['element-6066-11e4-a52e-4f735466cecf']
    command('POST',f'/element/{field}/value',{'text':'乔乔'})
    click('记住昵称')
    invoke('feed_treat')
    time.sleep(3)
    check('recorded treat reward',invoke('companion_status')['treats']==1)
    invoke('set_encounters',{'enabled':True})
    seen=set()
    for _ in range(2):
        event=wait(lambda: (v if (v:=invoke('encounter_status'))['kind'] else None),25)
        kind=event['kind'];seen.add(kind);mark(kind)
        time.sleep(1)
        if kind=='ball':
            wait(lambda: invoke('encounter_status')['phase']=='inviting',5)
        shot=OUT/f'video-{kind}.png'
        subprocess.run(['import','-window','root',str(shot)],check=True)
        if kind=='ball':
            x,y=invoke('play_status')['ball']
            rgb=subprocess.check_output(['convert',str(shot),'-crop',f'1x1+{x}+{y}','-depth','8','rgb:-'])
            check('visible ball center matches native pointer hit center',len(rgb)==3 and rgb[1]>rgb[0]+20 and rgb[1]>rgb[2]+20)
            print('TOY SIZE:',invoke('plugin:window|outer_size',{'label':'toy'}),flush=True)
        wait(lambda: invoke('encounter_status')['kind'] is None,12)
        time.sleep(.5)
    check('recorded both natural encounter animations',seen=={'butterfly','ball'})
    invoke('set_encounters',{'enabled':False})
    if os.environ.get('E2E_VIDEO_QUICK'):
        sys.exit(0)
    mark('idle-to-natural-sleep')
    command('POST','/window',{'handle':main})
    wait(lambda: js("return document.querySelector('[data-testid=pet]').dataset.sleeping==='true'"),195)
    time.sleep(4)
    subprocess.run(['import','-window','root',str(OUT/'video-sleep.png')],check=True)
    check('recorded natural sleep animation')
    mark('wake')
    native=pet_native()
    pointer('mousemove','--window',native,150,160)
    wait(lambda: js("return document.querySelector('[data-testid=pet]').dataset.sleeping==='false'"))
    time.sleep(3)
    check('recorded wake animation')
finally:
    OUT.joinpath('video-scenes.json').write_text(json.dumps({'scenes':segments,'checks':checks,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] and not isinstance(sys.exc_info()[1], SystemExit) else None},ensure_ascii=False,indent=2))
    if session:
        command('DELETE','')
