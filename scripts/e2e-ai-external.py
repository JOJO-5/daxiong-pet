#!/usr/bin/env python3
"""Native external chat must work without bundled weights; localhost fixture only."""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-ai-observation.py').read_text().split('\ntry:\n    new_session()')[0])
try:
    new_session();invoke('open_chat');wait(chat_window)
    check('fixture has no bundled AI resources',not snap()['assets']['available'])
    configure(enabled=True,backend='external',service_url=f'http://127.0.0.1:{server.server_port}/v1',service_model='external-only-fixture')
    # ai_configure emits the same live settings event used by the real UI.
    wait(lambda:js("return !!document.querySelector('#chat-message') && !document.querySelector('#chat-message').disabled"))
    check('external chat composer works without bundled weights',js("return !!document.querySelector('#chat-message') && !document.querySelector('#chat-message').disabled"))
    done(send('你好，陪我说一句话。'),timeout=30)
    check('separate external-only chat sends real request',len(requests)==1 and requests[0]['model']=='external-only-fixture')
    check('external-only chat does not start a local model',not model_pids())
    screenshot('external-only.png')
    invoke('ai_reset_personal_data')
    OUT.joinpath('e2e.json').write_text(json.dumps({'passed':checks,'scope':'Native Linux WebKit; bundled resources intentionally missing; localhost synthetic service'},ensure_ascii=False,indent=2),encoding='utf-8')
finally:
    error=sys.exc_info()[1]
    if error:OUT.joinpath('e2e.json').write_text(json.dumps({'passed':checks,'error':str(error)},ensure_ascii=False,indent=2),encoding='utf-8')
    server.shutdown();server.server_close()
    if session:
        try:command('DELETE','')
        except Exception:pass
