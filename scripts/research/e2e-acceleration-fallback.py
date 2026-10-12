#!/usr/bin/env python3
"""Real CPU recovery from a previous-enabled setting with a damaged optional
component. Fixture setup edits only this test profile; no GPU success is mocked.
"""
from pathlib import Path
import sys
script_root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(script_root));__file__=str(script_root/'e2e-ai-chat.py')
exec(Path(__file__).read_text().split('\ntry:\n    new_session()')[0])
root=Path(os.environ['XDG_DATA_HOME'])/'com.jojo.daxiongpet'/'ai-acceleration'
try:
    new_session();invoke('open_chat');wait(chat_window);settings=snap()['settings'];settings.update(enabled=True,backend='bundled',output_tokens=32);invoke('ai_configure',{'settings':settings})
    catalog=json.loads((ROOT/'native/litert/worker-catalog.json').read_text());component=root/('component-'+catalog['workers']['linux-x64']['sha256'][:16]);component.mkdir(parents=True)
    (component/'liblitert-lm.so').write_bytes(b'corrupt')
    (component/'installed.json').write_text(json.dumps({'files':{'liblitert-lm.so':{'bytes':131785976,'sha256':'0'*64}}}))
    # Represents a profile enabled on a previous launch; production API requires
    # successful GPU probing and cannot enable an unverified component.
    settings['acceleration_enabled']=True
    with sqlite3.connect(db) as connection:connection.execute('UPDATE ai_settings SET data=? WHERE id=1',(json.dumps(settings),))
    check('fixture restores previous opt-in with corrupt component',snap()['settings']['acceleration_enabled'])
    reply=done(send('你好，简短打个招呼。'))
    check('damaged optional component automatically retries on real CPU',bool(reply['content'].strip()) and snap()['acceleration_runtime']['backend']=='cpu' and '损坏' in snap()['acceleration_runtime']['note'])
    check('fallback writes one completed reply, not two models concatenated',len([m for m in snap()['messages'] if m['request_id']==reply['request_id'] and m['role']=='assistant'])==1)
    screenshot('automatic-cpu-fallback.png')
    reply=done(send('谢谢，简短回应就好。'));check('following request continues on CPU without repeated GPU initialization',bool(reply['content'].strip()))
finally:
    error=sys.exc_info()[1]
    OUT.joinpath('fallback-result.json').write_text(json.dumps({'error':str(error) if error else None,'checks':checks,'scope':'restored previous-enabled fixture plus damaged library; real production fallback and CPU model; no hardware GPU claim'},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
