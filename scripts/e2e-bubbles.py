#!/usr/bin/env python3
"""Check speech bounds in the actual transparent native pet window."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])

def bounds():
    return js("const e=document.querySelector('.bubble');if(!e)return null;const r=e.getBoundingClientRect();return {text:e.textContent,x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom,viewport:[innerWidth,innerHeight],page:Number(e.dataset.page||1),pages:Number(e.dataset.pages||1),full:e.getAttribute('aria-label')||e.textContent}")

def shoot(name):
    subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)

try:
    new_session()
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5200"))
    wait(lambda:invoke('plugin:window|is_visible',{'label':'main'}))
    native=pet_native();pointer('windowmove',native,380,260);pointer('mousemove',30,30)
    wait(bounds);wait(lambda:bounds()['bottom']<=78.01)
    intro=bounds();print('INTRO',json.dumps(intro,ensure_ascii=False),flush=True);shoot('intro')
    if os.environ.get('E2E_BUBBLE_DIAGNOSE'):
        print(json.dumps({'intro':intro},ensure_ascii=False));sys.exit(0)
    check('intro border and shadow fit inside native viewport',intro['y']>=8 and intro['x']>=12 and intro['right']<=288 and intro['bottom']<=83)
    check('intro uses available width instead of half-window shrink-to-fit',intro['width']>250)
    invoke('set_quiet_companion',{'enabled':True})
    messages=[('short','汪！'),('long','谢谢乔乔！飞盘接回来啦，再从这里扔一次，我们继续一起玩吧。'),('english','I brought your frisbee back. Throw it again from here and let us play together!'),('token','x'*200),('multiline','第一行\n第二行\n第三行\n第四行\n第五行')]
    for name,text in messages:
        invoke('plugin:event|emit',{'event':'pet:message','payload':text})
        wait(lambda:bounds() and bounds()['full']==text);wait(lambda:bounds()['bottom']<=78.01)
        r=bounds();print(name,json.dumps(r,ensure_ascii=False),flush=True)
        check(name+' border and shadow remain inside viewport',r['x']>=12 and r['y']>=8 and r['right']<=288 and r['bottom']<=83)
        shoot(name)
        collected=r['text']
        for page in range(2,r['pages']+1):
            part=wait(lambda:bounds() if bounds() and bounds()['page']==page else None,8)
            check(name+' page '+str(page)+' fits',part['y']>=8 and part['right']<=288 and part['bottom']<=83)
            collected+=part['text']
        check(name+' retains every character across pages',collected==text)
    error='宠物加载失败：'+'the_original_file_is_unavailable_'*16+'。已尝试恢复大熊。'
    invoke('plugin:event|emit',{'event':'pet:error','payload':error})
    wait(lambda:bounds() and bounds()['full']==error);wait(lambda:bounds()['bottom']<=78.01)
    r=bounds();check('long error is readable without leaving the viewport',r['y']>=8 and r['pages']>1);shoot('long-error')
    collected=r['text']
    for page in range(2,r['pages']+1):
        part=wait(lambda:bounds() if bounds() and bounds()['page']==page else None,8)
        check('error page '+str(page)+' fits',part['y']>=8 and part['right']<=288 and part['bottom']<=83)
        collected+=part['text']
        if page==4:
            invoke('plugin:event|emit',{'event':'pet:say','payload':'idle'})
            check('ambient speech cannot replace error after original seven seconds',bounds()['full']==error)
    check('long error retains every character',collected==error)
    wait(lambda:bounds() is None,8)
    check('speech disappears after the final page')
    check('pet size and mouth coordinate origin stay unchanged',js("const r=document.querySelector('.pet-clip').getBoundingClientRect();return r.x===78&&r.y===84&&r.width===144&&r.height===156"))
    (OUT/'result.json').write_text(json.dumps({'ok':True,'checks':checks},ensure_ascii=False,indent=2))
finally:
    if session:command('DELETE','')
