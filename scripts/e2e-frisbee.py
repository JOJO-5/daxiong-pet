#!/usr/bin/env python3
"""Native throwing, bidirectional disc biting, air catches and reusable return."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
def phase():return js("return document.querySelector('[data-testid=play-phase]').dataset.phase")
def shoot(name):subprocess.run(['import','-window','root',str(OUT/f'{name}.png')],check=True)
try:
 new_session();main=command('GET','/window')
 wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===4784"))
 invoke('set_encounters',{'enabled':False});initial=set(command('GET','/window/handles'));invoke('open_playground');time.sleep(1.2)
 panel=next(iter(set(command('GET','/window/handles'))-initial));command('POST','/window',{'handle':panel})
 native_windows=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
 native=next(w for w in native_windows if 'WIDTH=300' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
 panel_native=next(w for w in native_windows if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True));pointer('windowmove',panel_native,870,30)
 invoke('plugin:event|listen',{'event':'pet:frame','target':{'kind':'Any'},'handler':js('window.__frames=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__frames.push(e.payload))')})
 click('飞盘');click('拿出飞盘');wait(lambda:phase()=='ready')
 check('native button shows independent blue disc',invoke('play_status')['toy']=='frisbee' and invoke('plugin:window|is_visible',{'label':'toy'}));shoot('ready')
 for turn in [1,2]:
  js('window.__frames=[];return true');seen=set();checked=set()
  if turn==1:click('扔飞盘');pointer('mousemove',30,30)
  else:
   ball=invoke('play_status')['ball'];pointer('mousemove',*ball);time.sleep(.1);pointer('mousedown',1);wait(lambda:phase()=='held')
   check('returned frisbee accepts real pointer grab')
   pointer('mousemove',180,550);time.sleep(.2);pointer('mouseup',1);pointer('mousemove',30,30)
  wait(lambda:phase()=='chasing');check(f'throw {turn}: real button or drag starts glide')
  def returned():
   current=phase();seen.add(current)
   if current in ('catching','returning') and current not in checked:
    checked.add(current)
    assert not invoke('plugin:window|is_visible',{'label':'toy'}),'duplicate disc while biting'
   return current=='returned'
  wait(returned,25)
  check(f'throw {turn}: air catch, biting carry and release',{'catching','returning','releasing','returned'}.issubset(seen))
  check(f'throw {turn}: returned once',invoke('play_status')['catches']==turn)
  check(f'throw {turn}: airborne feedback',invoke('play_status')['last_catch']=='air')
  frames=js('return window.__frames');expected=22 if turn==1 else 21
  check(f'throw {turn}: actual disc biting row {expected} rendered',any(f['row']==expected for f in frames))
  check(f'throw {turn}: disc visible at feet for reuse',invoke('plugin:window|is_visible',{'label':'toy'}));shoot(f'returned-{turn}')
  command('POST','/window',{'handle':main});wait(lambda:js("return document.querySelector('.bubble')?.textContent.includes('飞盘')"))
  check(f'throw {turn}: spoken feedback matches frisbee');command('POST','/window',{'handle':panel})
 before=invoke('play_status');time.sleep(1);check('returned disc remains available and score stable',phase()=='returned' and invoke('play_status')['catches']==before['catches'])
 # Record the first native play update to distinguish a launch from a respawn.
 origin=invoke('play_status')['ball'];scale=js('return window.devicePixelRatio')
 invoke('plugin:event|listen',{'event':'pet:play','target':{'kind':'Any'},'handler':js("window.__rethrow=[];return window.__TAURI_INTERNALS__.transformCallback(e=>window.__rethrow.push(e.payload))")})
 click('扔飞盘')
 launched=wait(lambda:js("return window.__rethrow.find(v=>v.toy==='frisbee'&&v.phase==='chasing')||null"))
 check('button rethrows returned disc from its release spot',abs(launched['ball'][0]-origin[0])<=30*scale and abs(launched['ball'][1]-origin[1])<=10*scale)
 wait(lambda:phase()=='returned',25)
 check('button rethrow completes a third round exactly once',invoke('play_status')['catches']==before['catches']+1)
 shoot('button-rethrow-returned')
 click('拿出飞盘');invoke('set_pomodoro',{'active':True});wait(lambda:phase()=='off');check('focus cleans up disc');invoke('set_pomodoro',{'active':False})
 click('拿出飞盘');invoke('set_visible',{'visible':False});wait(lambda:phase()=='off');check('hide cancels disc and toy window',not invoke('plugin:window|is_visible',{'label':'toy'}))
except Exception:shoot('failure');raise
finally:
 error=sys.exc_info()[1];OUT.joinpath('frisbee-e2e.json').write_text(json.dumps({'version':json.loads(ROOT.joinpath('package.json').read_text())['version'],'passed':checks,'error':str(error) if error else None},ensure_ascii=False,indent=2))
 if session:
  try:command('DELETE','')
  except Exception:pass
