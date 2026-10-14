#!/usr/bin/env python3
"""Real native settings layout at normal and narrow window sizes; no downloads."""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-ai-chat.py').read_text().split('\ntry:\n    new_session()')[0])

try:
    new_session()
    invoke('open_chat')
    wait(chat_window)
    click('性格与设置');click('模型与性能')
    wait(lambda: js("return document.querySelectorAll('.chat-model-card').length===3"))
    native_click("document.querySelector('[data-testid=acceleration-settings] summary')")
    for width, height in [(500, 700), (380, 480)]:
        command('POST', '/window/rect', {'width': width, 'height': height})
        time.sleep(.4)
        check(f'{width}px settings have no horizontal overflow', js("const e=document.querySelector('.chat-settings');return e.scrollWidth<=e.clientWidth+1"))
        check(f'{width}px model names and metadata never overlap', js("return [...document.querySelectorAll('.chat-model-card')].every(e=>{const n=e.querySelector('strong').getBoundingClientRect(),m=e.querySelector('.chat-model-meta').getBoundingClientRect();return m.top>=n.bottom+5})"))
        check(f'{width}px accelerator controls have visible gaps', js("const b=[...document.querySelectorAll('.chat-download-actions button')].map(e=>e.getBoundingClientRect());return b.every((r,i)=>b.slice(i+1).every(s=>r.right+8<=s.left||s.right+8<=r.left||r.bottom+8<=s.top||s.bottom+8<=r.top))"))
        for selector, name in [('.chat-model-card', 'models'), ('[data-testid=acceleration-settings]', 'acceleration')]:
            js(f"document.querySelector({json.dumps(selector)}).scrollIntoView({{block:'start'}})")
            time.sleep(.2)
            screenshot(f'{name}-{width}.png')
    OUT.joinpath('e2e.json').write_text(json.dumps({'passed': checks}, ensure_ascii=False, indent=2))
finally:
    error = sys.exc_info()[1]
    if error:
        OUT.joinpath('e2e.json').write_text(json.dumps({'passed': checks, 'error': str(error)}, ensure_ascii=False, indent=2))
        screenshot('failure-desktop.png')
    if session:
        try:
            command('DELETE', '')
        except Exception:
            pass
