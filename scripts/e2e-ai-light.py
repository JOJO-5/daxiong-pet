#!/usr/bin/env python3
"""Real GUI download from the pinned source, then native CPU chat in a light stage."""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-ai-chat.py').read_text().split('\ntry:\n    new_session()')[0])

try:
    root = Path(os.environ['DAXIONG_AI_RESOURCE_DIR'])
    check('light resources omit chat and visual weights', not list(root.rglob('*.gguf')))
    check('light retains CPU runner and OCR', (root/'runtime/llama-server').is_file() and (root/'ocr/manifest.json').is_file())
    new_session()
    invoke('open_chat');chat = wait(chat_window)
    check('light starts without a ready local model', not snap()['assets']['available'])
    click('性格与设置')
    native_click("[...document.querySelectorAll('.chat-memory')].find(e=>e.textContent.includes('Qwen3.5-2B-Q4_K_M.gguf')).querySelector('button')")
    wait(lambda: snap()['download']['busy'])
    def receiving():
        download=snap()['download']
        assert download['busy'] or download['received'] > 0, download['status']
        return download['received'] > 0
    wait(receiving, 120)
    check('GUI download receives real network bytes', snap()['download']['total']==1270808032)
    wait(lambda: not snap()['download']['busy'], 900)
    state=snap()
    check('download verifies pinned size and SHA256', '校验通过' in state['download']['status'])
    check('download enables CPU model without bundled weights', state['assets']['available'] and not list(root.rglob('*.gguf')))
    wait(lambda: js("return [...document.querySelectorAll('button')].some(b=>b.textContent.trim()==='启用本地聊天'&&!b.disabled)"))
    click('启用本地聊天');wait(lambda: snap()['settings']['enabled']);click('聊天')
    started=time.monotonic();reply=done(send('你好，简短打个招呼吧。'))
    timings['downloaded_cpu_reply_seconds']=round(time.monotonic()-started,3)
    check('downloaded model generates through native CPU', bool(reply['content'].strip()))
    screenshot('light-downloaded-chat.png')
finally:
    error=sys.exc_info()[1]
    if error:
        try:screenshot('failure.png')
        except Exception:pass
    OUT.joinpath('light-result.json').write_text(json.dumps({'error':str(error) if error else None,'checks':checks,'timings':timings},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
