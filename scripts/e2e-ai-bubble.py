#!/usr/bin/env python3
"""Native frontend priority/timer regression; synthetic reply events, no model inference."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'e2e-desktop.py').read_text().split('\ntry:\n    new_session()')[0])
try:
    new_session()
    wait(lambda:js("return document.querySelector('.pet-sheet')?.naturalHeight===5616"))
    time.sleep(1)
    for event,payload in [('ai:screen-bubble',{'text':'正在核对你的笔记，我会安静陪你继续写。'}),('ai:reply',{'content':'好的，我会陪你把这份笔记写完。','status':'completed','preview':False,'error':None})]:
        text=payload.get('text',payload.get('content'))
        invoke('plugin:event|emit',{'event':event,'payload':payload})
        wait(lambda:js("return document.querySelector('.bubble')?.getAttribute('aria-label')")==text)
        invoke('plugin:event|emit',{'event':'pet:say','payload':'idle'})
        invoke('plugin:event|emit',{'event':'pet:message','payload':'自动台词'})
        time.sleep(3)
        check(event+': survives ordinary speech and old timeout',js("return document.querySelector('.bubble')?.getAttribute('aria-label')")==text)
        time.sleep(8)
        check(event+': expires after reading interval',js("return !document.querySelector('.bubble')"))
finally:
    OUT.joinpath('ai-bubble-e2e.json').write_text(json.dumps({'passed':checks,'error':str(sys.exc_info()[1]) if sys.exc_info()[1] else None},ensure_ascii=False,indent=2))
    if session:command('DELETE','')
