#!/usr/bin/env python3
"""Native missing-resource and database-error paths, without starting any model."""
from pathlib import Path
exec((Path(__file__).resolve().parent / 'e2e-ai-chat.py').read_text().split('\ntry:\n    new_session()')[0])

try:
    new_session()
    main = command('GET', '/window')
    wait(lambda: js("return !!document.querySelector('.pet-sheet')?.complete"))
    time.sleep(.7)
    native = pet_native()
    def hover():
        pointer('mousemove', '--window', native, 150, 160)
        return js("return document.querySelector('[data-testid=pet]')?.dataset.clickable==='true'")
    wait(hover)
    pointer('click', 3)
    time.sleep(.4)
    pointer('key', 'Home', 'Down', 'Return')
    chat = wait(chat_window)
    check('real native context-menu chat entry opens the chat window', bool(chat))
    data = snap()
    check('missing resources show a clear explanation without activating AI', not data['assets']['available'] and not data['settings']['enabled'] and js("return document.querySelector('.chat-setup').textContent.includes('模型还没准备好') && document.querySelector('#chat-message').disabled"))
    click('性格与设置')
    fill('#ai-owner-name', '本地测试')
    click('保存性格')
    wait(lambda: snap()['personality']['owner_name'] == '本地测试')
    click('恢复默认')
    check('restore defaults stays a draft until saved', snap()['personality']['owner_name'] == '本地测试' and js("return document.querySelector('#ai-owner-name').value===''") )
    click('放弃修改')
    check('discard restores the saved profile', js("return document.querySelector('#ai-owner-name').value==='本地测试'"))
    screenshot('unavailable-personality.png')
    command('POST', '/window', {'handle': main})
    try:
        invoke('ai_snapshot')
        raise AssertionError('main window read private AI state')
    except AssertionError as e:
        check('private chat commands reject the pet window', '聊天窗口' in str(e))
    command('DELETE', '')
    session = None
    # Remove this isolated fixture's recovery journal before replacing its main file.
    # Otherwise a valid WAL can legitimately recover the intentionally broken file.
    for suffix in ('-wal', '-shm'):
        Path(str(db) + suffix).unlink(missing_ok=True)
    db.write_bytes(b'not a SQLite database\x00')
    new_session()
    invoke('open_chat')
    wait(chat_window)
    wait(lambda: js("return !!document.querySelector('.chat-error')"))
    check('corrupt database reports an error and is never silently replaced', db.read_bytes() == b'not a SQLite database\x00')
    screenshot('database-error.png')
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
