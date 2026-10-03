"""Cumulative scenarios executed by e2e-desktop.py against the real application."""
import signal

def restart_app():
    global session, main, panel, native
    old_pid = int(subprocess.check_output(["xdotool","getwindowpid",native],text=True))
    command("DELETE", "")
    session = None
    try:
        os.kill(old_pid,signal.SIGTERM)
    except ProcessLookupError:
        pass
    wait(lambda: not Path(f"/proc/{old_pid}").exists(),10)
    new_session()
    main = command("GET","/window")
    wait(lambda: js("return !!document.querySelector('.pet-sheet')?.naturalWidth"))
    wait(lambda: invoke("plugin:window|is_visible",{"label":"main"}))
    time.sleep(.65)
    native = pet_native()
    initial = set(command("GET","/window/handles"))
    invoke("open_playground")
    time.sleep(1.2)
    wait(lambda: len(command("GET","/window/handles"))>len(initial))
    panel = next(iter(set(command("GET","/window/handles"))-initial))
    wait(lambda: invoke("plugin:window|is_visible",{"label":"playground"}))
    time.sleep(.7)
    command("POST","/window",{"handle":panel})
    open_more()
    wait(lambda: js("return !!document.querySelector('#nickname')"))

open_more()
wait(lambda: js("return !!document.querySelector('#nickname')"))
memory = invoke("companion_status")
check("completed fetches are recorded in local companion memory",memory["fetches"]==2 and memory["affection"]>=3)
field = command("POST","/element",{"using":"css selector","value":"#nickname"})["element-6066-11e4-a52e-4f735466cecf"]
command("POST",f"/element/{field}/value",{"text":"乔乔"})
click("记住昵称")
wait(lambda: invoke("companion_status")["nickname"]=="乔乔")
check("nickname saved through real form")
try:
    invoke("set_nickname",{"nickname":"熊"*17})
    raise AssertionError("oversized nickname accepted")
except AssertionError as e:
    check("backend rejects oversized nickname", "16" in str(e))
# A native menu can leave the pointer over the dog long enough to pet it.
# Respect the real 60-second affection cooldown instead of changing saved dates.
invoke("set_encounters",{"enabled":False});pointer("mousemove",30,30)
pat_path=Path(os.environ["XDG_CONFIG_HOME"])/"com.jojo.daxiongpet"/"companion.json"
print("WAIT: real petting reward cooldown before feeding/restart scenario",flush=True)
wait(lambda: time.time()-float(json.loads(pat_path.read_text()).get("last_pat") or 0)>=60,65)
before = invoke("companion_status")
click("喂一块饼干")
wait(lambda: invoke("companion_status")["treats"]==before["treats"]+1)
check("treat awards affection only after local save",invoke("companion_status")["affection"]==before["affection"]+4)
command("POST","/window",{"handle":main})
wait(lambda: js("return document.querySelector('[data-testid=pet]')?.dataset.row==='20' || !!document.querySelector('[data-testid=treat-cookie]')"),3)
check("pet renders eating response and personalized thank-you",js("return document.querySelector('.bubble')?.textContent.includes('乔乔')"))
command("POST","/window",{"handle":panel})
try:
    invoke("feed_treat")
    raise AssertionError("cooldown was bypassed")
except AssertionError as e:
    check("feeding cooldown is enforced in Rust", "饼干" in str(e))
check("feeding cooldown disables button",js("return [...document.querySelectorAll('button')].some(b=>b.disabled && b.textContent.includes('下块饼干'))"))
time.sleep(.15)
# A hover is the application's existing real petting gesture.
before_pat = invoke("companion_status")["pats"]
pointer("mousemove","--window",native,140,145)
wait(lambda: invoke("companion_status")["pats"]>before_pat,10)
pointer("mousemove",30,30)
check("real hover petting persists affection")
saved = invoke("companion_status")
restart_app()
restored = invoke("companion_status")
check("real process restart retains nickname, affection and counters",all(restored[k]==saved[k] for k in ["nickname","affection","treats","fetches","pats"]))
check("restart retains remaining feeding cooldown",restored["treat_wait"]>0)

subprocess.run(["import","-window","root",str(OUT/"companion-saved-desktop.png")],check=True)

memory_path = Path(os.environ["XDG_CONFIG_HOME"])/"com.jojo.daxiongpet"/"companion.json"
assert memory_path.is_file(), memory_path
stored_bytes = memory_path.read_bytes()
# Replace the file with a directory: even root cannot atomically overwrite it with a regular file.
memory_path.unlink()
memory_path.mkdir()
try:
    try:
        invoke("set_nickname",{"nickname":"不应写入"})
        raise AssertionError("save unexpectedly succeeded")
    except AssertionError as e:
        check("real disk write failure is returned", "保存陪伴记忆失败" in str(e))
    check("failed save leaves in-memory nickname unchanged",invoke("companion_status")["nickname"]==saved["nickname"])
finally:
    memory_path.rmdir()
    memory_path.write_bytes(stored_bytes)

memory_path.write_text("broken-memory")
restart_app()
wait(lambda: js("return !!document.querySelector('[role=alert]')"))
check("corrupt memory is reported without overwriting original", "读取失败" in invoke("companion_status")["error"] and memory_path.read_text()=="broken-memory")
native_click("document.querySelector('details summary')")
check("corrupt memory explanation stays visible with settings collapsed",js("const e=document.querySelector('.quick-feed [role=alert]');return !document.querySelector('details').open&&e?.getBoundingClientRect().height>0"))
click("查看记忆与恢复");wait(lambda:js("return document.querySelector('details').open"))
click("备份旧记忆并重新开始")
wait(lambda: invoke("companion_status")["error"] is None)
check("explicit recovery backs up broken data and creates valid memory",any(p.read_text()=="broken-memory" for p in memory_path.parent.glob("companion.backup-*.json")) and json.loads(memory_path.read_text())["affection"]==0)
subprocess.run(["import","-window","root",str(OUT/"companion-desktop.png")],check=True)
