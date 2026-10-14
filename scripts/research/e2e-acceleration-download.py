#!/usr/bin/env python3
"""Real UI download of fixed upstream SDK + released worker. Reuses the exact
verified research model to avoid another gigabyte in this isolated test profile.
This machine has no hardware GPU; qualification covers install and CPU fallback.
"""
from pathlib import Path
import sys
script_root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(script_root));__file__=str(script_root/'e2e-ai-chat.py')
exec(Path(__file__).read_text().split('\ntry:\n    new_session()')[0])
import hashlib
def optional_pids():
    result=[]
    for p in Path("/proc").iterdir():
        if not p.name.isdigit():continue
        try:
            if (p/"exe").readlink().name=="daxiong-litert-worker":result.append(int(p.name))
        except (FileNotFoundError,PermissionError,OSError):pass
    return result
root=Path(os.environ['XDG_DATA_HOME'])/'com.jojo.daxiongpet'/'ai-acceleration'
model=Path(os.environ['LITERT_TEST_MODEL'])
workers=json.loads((ROOT/'native/litert/worker-catalog.json').read_text())
try:
    new_session();main=command('GET','/window');invoke('open_chat');chat=wait(chat_window)
    check('acceleration is available but disabled by default',snap()['acceleration']['available'] and not snap()['settings']['acceleration_enabled'])
    settings=snap()['settings'];settings.update(enabled=True,backend='bundled',output_tokens=32);invoke('ai_configure',{'settings':settings})
    root.mkdir(parents=True,exist_ok=True);(root/'Qwen3.5-0.8B_int8.litertlm').symlink_to(model)
    check('reuse exact research model bytes without copying weights',(root/'Qwen3.5-0.8B_int8.litertlm').stat().st_size==963184864)
    click('性格与设置');click('模型与性能')
    native_click("document.querySelector('[data-testid=acceleration-settings] summary')")
    wait(lambda:js("return document.querySelector('[data-testid=acceleration-settings] details').open"))
    wait(lambda:js("return [...document.querySelectorAll('[data-testid=acceleration-settings] button')].some(b=>b.textContent==='下载加速组件与模型'&&!b.disabled)"))
    native_click("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='下载加速组件与模型')")
    # Cancel through the actual UI handler immediately, while connection starts.
    wait(lambda:snap()['acceleration']['busy']);js("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='取消加速下载').click()")
    wait(lambda:not snap()['acceleration']['busy'],30)
    check('cancel does not install an incomplete component',not snap()['acceleration']['installed'] and '停止' in snap()['acceleration']['status'])
    check('cancel removes temporary network file',not list(root.glob('*.part')))
    wait(lambda:js("return [...document.querySelectorAll('[data-testid=acceleration-settings] button')].some(b=>b.textContent==='下载加速组件与模型'&&!b.disabled)"))
    native_click("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='下载加速组件与模型')")
    wait(lambda:snap()['acceleration']['busy']);wait(lambda:not snap()['acceleration']['busy'],180)
    state=snap();check('real SDK and released worker download and install',state['acceleration']['installed'] and '校验完成' in state['acceleration']['status'])
    check('download remains opt-in without changing backend',not state['settings']['acceleration_enabled'] and state['settings']['backend']=='bundled')
    components=list(root.glob('component-*'));assert len(components)==1;component=components[0]
    manifest=json.loads((component/'installed.json').read_text())
    for name,expected in manifest['files'].items():
        digest=hashlib.sha256((component/name).read_bytes()).hexdigest();assert digest==expected['sha256']
    check('installed libraries and worker match recorded SHA256',len(manifest['files'])==2)
    check('component contains licenses and no Python runtime',bool(list((component/'licenses').rglob('*LICENSE*'))) and not list(component.rglob('*.py')))
    wait(lambda:js("return [...document.querySelectorAll('[data-testid=acceleration-settings] button')].some(b=>b.textContent==='试运行并启用加速'&&!b.disabled)"))
    screenshot('download-installed.png')
    native_click("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='试运行并启用加速')")
    wait(lambda:snap()['acceleration_testing']);active_pids=wait(optional_pids,20)
    wait(lambda:js("return [...document.querySelectorAll('[data-testid=acceleration-settings] button')].some(b=>b.textContent==='停止试运行')"))
    native_click("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='停止试运行')")
    wait(lambda:not snap()['acceleration_testing'],8);wait(lambda:all(not Path('/proc',str(p)).exists() for p in active_pids),5)
    check('Stop GPU initialization clears parent-owned native process',not snap()['settings']['acceleration_enabled'])

    # Corrupt a copied library, never the upstream fixture or original CPU runtime.
    library=component/'liblitert-lm.so';data=library.read_bytes();library.write_bytes(b'bad')
    corruption_error=None
    try:invoke('ai_acceleration_enable')
    except AssertionError as error:corruption_error=str(error)
    assert corruption_error and '损坏' in corruption_error,corruption_error
    check('corrupt component rejects enable and preserves CPU',not snap()['settings']['acceleration_enabled'] and snap()['acceleration_runtime']['backend']=='cpu')
    library.write_bytes(data)
    # Exercise real GPU initialization, not a simulated GPU-success response.
    started=time.monotonic()
    native_click("[...document.querySelectorAll('[data-testid=acceleration-settings] button')].find(b=>b.textContent==='试运行并启用加速')")
    wait(lambda:snap()['acceleration_testing']);wait(lambda:not snap()['acceleration_testing'],110)
    state=snap();gpu_result=state['acceleration_runtime']['note']
    timings['gpu_probe_seconds']=round(time.monotonic()-started,3)
    check('GPU probe finishes with honest backend',state['acceleration_runtime']['backend'] in ('cpu','ml-drift-gpu'))
    if state['settings']['acceleration_enabled']:invoke('ai_acceleration_disable')
    check('CPU remains selected after disabled or unsupported acceleration',snap()['acceleration_runtime']['backend']=='cpu')
    screenshot('gpu-probe-or-cpu-fallback.png')
    click('聊天');reply=done(send('你好，简短打个招呼。'));check('original CPU model still replies after optional probe',bool(reply['content'].strip()))
    click('性格与设置');invoke('ai_acceleration_remove',{'model':False})
    check('removing component keeps independent model',not snap()['acceleration']['installed'] and snap()['acceleration']['model_installed'])
    invoke('ai_acceleration_remove',{'model':True});check('removing optional model leaves CPU assets ready',not snap()['acceleration']['model_installed'] and snap()['assets']['available'])
    screenshot('removed-cpu-preserved.png')
finally:
    error=sys.exc_info()[1]
    if error:
        try:screenshot('failure.png')
        except Exception:pass
    OUT.joinpath('acceleration-result.json').write_text(json.dumps({'error':str(error) if error else None,'checks':checks,'timings':timings,'gpu_probe':locals().get('gpu_result'),'worker_tag':workers['tag'],'scope':'real GUI upstream SDK + worker download; verified existing 0.8B model reuse; CPU fallback; no physical GPU or speed qualification'},ensure_ascii=False,indent=2))
    if session:
        try:command('DELETE','')
        except Exception:pass
