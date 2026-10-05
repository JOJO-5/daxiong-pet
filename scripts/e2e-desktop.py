#!/usr/bin/env python3
"""Real Tauri + WebKitWebDriver + X11 input. No IPC mocks or browser-only harness."""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import shutil
import time
from webdriver_http import request

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get("E2E_OUT", str(ROOT / "test-results")))
OUT.mkdir(parents=True, exist_ok=True)
session = None
checks = []
fixture = ROOT/"src-tauri/target/debug/pets/e2e-legacy"

def command(method, path, data=None):
    return request(method, f"/session/{session}" + path, data)

def js(script):
    return command("POST", "/execute/sync", {"script": script, "args": []})

def invoke(name, args=None):
    script = "const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(" + json.dumps(name) + "," + json.dumps(args or {}) + ").then(v=>done({ok:true,value:v})).catch(e=>done({ok:false,error:String(e)}));"
    value = command("POST", "/execute/async", {"script": script, "args": []})
    if not value["ok"]:
        raise AssertionError(value["error"])
    return value.get("value")

def wait(fn, timeout=20):
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        value = fn()
        if value:
            return value
        time.sleep(.08)
    raise AssertionError("Timed out waiting for " + str(fn))

def check(name, condition=True):
    assert condition, name
    checks.append(name)
    print("PASS:", name, flush=True)

def native_click(expression):
    rect=js("const e="+expression+";if(!e||e.disabled)return null;e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};")
    assert rect,'button not found or disabled'
    time.sleep(.12)
    candidates=subprocess.check_output(['xdotool','search','--onlyvisible','--class','Daxiong-pet'],text=True).strip().splitlines()
    panel_window=next(w for w in candidates if 'WIDTH=360' in subprocess.check_output(['xdotool','getwindowgeometry','--shell',w],text=True))
    info=subprocess.check_output(['xwininfo','-id',panel_window],text=True)
    import re
    geometry={axis: int(re.search(r'Absolute upper-left '+axis+r':\s*(-?\d+)',info).group(1)) for axis in ('X','Y')}
    pointer('windowraise',panel_window)
    pointer('mousemove',int(geometry['X'])+round(rect['x']),int(geometry['Y'])+round(rect['y']))
    time.sleep(.08);pointer('click',1);time.sleep(.08)

def open_more():
    if not js("return document.querySelector('details').open"):
        native_click("document.querySelector('details summary')")
        wait(lambda: js("return document.querySelector('details').open"))

def click(text):
    if text in ('抛一球','扔飞盘'):
        native_click("document.querySelector('[data-testid=play-throw]')")
        return
    if text=='喂一块饼干':
        native_click("document.querySelector('[data-testid=quick-feed]') || [...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='喂一块饼干')")
        return
    native_click("[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==="+json.dumps(text)+")")

def window(title):
    for handle in command("GET", "/window/handles"):
        command("POST", "/window", {"handle": handle})
        if js("return document.title") == title:
            return handle
    return None

def pet_native():
    windows=subprocess.check_output(["xdotool","search","--onlyvisible","--class","Daxiong-pet"],text=True).strip().splitlines()
    return next(w for w in windows if "WIDTH=300" in subprocess.check_output(["xdotool","getwindowgeometry","--shell",w],text=True))

def pointer(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True)

def new_session():
    global session
    value = request("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {
        "application": str(ROOT / "src-tauri/target/debug/daxiong-pet")}}}})
    session = value["sessionId"]
    command("POST", "/timeouts", {"script": 15000, "implicit": 5000})

try:
    new_session()
    main = command("GET", "/window")
    wait(lambda: js("return !!document.querySelector('.pet-sheet')?.complete"))
    check("native WebKit loads decoded built-in pet", js("return document.querySelector('.pet-sheet').naturalWidth===1536 && !document.querySelector('.vite-error-overlay')"))
    # Right click through actual OS pointer events to open the same user-facing panel.
    wait(lambda: invoke("plugin:window|is_visible", {"label":"main"}))
    time.sleep(.65)
    native = pet_native()
    initial_handles = set(command("GET", "/window/handles"))
    pointer("mousemove", "--window", native, 150, 160)
    wait(lambda: js("return document.querySelector('[data-testid=pet]')?.dataset.clickable==='true'"))
    pointer("click", 3)
    time.sleep(.35);pointer("key","Home","Return")
    wait(lambda: len(command("GET", "/window/handles")) > len(initial_handles))
    panel = next(iter(set(command("GET", "/window/handles"))-initial_handles))
    wait(lambda: invoke("plugin:window|is_visible", {"label":"playground"}))
    time.sleep(.7)
    command("POST", "/window", {"handle":panel})
    wait(lambda: js("return !!document.querySelector('.play-panel')"))
    check("pet native context menu opens real interaction window")
    wait(lambda: js("return !!document.querySelector('[data-testid=play-phase]')"))
    click("抛一球")
    phases = set()
    def returned():
        phase = js("return document.querySelector('[data-testid=play-phase]').dataset.phase")
        phases.add(phase)
        return phase == "returned"
    wait(returned, 25)
    check("throw, chase, carry, return and score in native runtime", {"chasing", "returning", "returned"}.issubset(phases)
          and js("return document.querySelector('[data-testid=catches]').textContent") == "1")
    # Evidence includes the actual desktop and independent toy window.
    time.sleep(.3)
    subprocess.run(["import", "-window", "root", str(OUT / "fetch-desktop.png")], check=True)
    wait(lambda: invoke("play_status")["phase"] == "returned")
    ball = invoke("play_status")["ball"]
    pointer("mousemove", ball[0], ball[1]); time.sleep(.15)
    pointer("mousedown", 1); time.sleep(.12)
    check("real pointer grabs toy window", invoke("play_status")["phase"] == "held")
    pointer("mousemove", ball[0]+180, ball[1]-90); time.sleep(.05)
    pointer("mouseup", 1)
    wait(lambda: invoke("play_status")["phase"] == "returned", 25)
    check("real drag/release throws toy and completes a second fetch", invoke("play_status")["catches"] == 2)
    click("收起玩具")
    wait(lambda: invoke("play_status")["phase"] == "off")
    check("cancel removes toy")
    click("拿出球")
    wait(lambda: invoke("play_status")["phase"] == "ready")
    invoke("set_visible", {"visible":False})
    wait(lambda: invoke("play_status")["phase"] == "off")
    check("hiding the pet removes independent toy window", not invoke("plugin:window|is_visible",{"label":"toy"}))
    invoke("set_visible", {"visible":True})
    wait(lambda: invoke("plugin:window|is_visible",{"label":"main"}))
    click("抛一球")
    invoke("set_pomodoro", {"active": True})
    wait(lambda: invoke("play_status")["phase"] == "off")
    check("focus cancels toys")
    invoke("set_pomodoro", {"active": False})
    click("拿出球")
    wait(lambda: invoke("play_status")["phase"] == "ready")
    # Pet dragging must take priority over toys.
    pointer("mousemove", "--window", native, 150, 160); time.sleep(.12)
    pointer("mousedown", 1); time.sleep(.12)
    pointer("mousemove_relative", "--", 50, 0); time.sleep(.12)
    pointer("mouseup", 1)
    wait(lambda: invoke("play_status")["phase"] == "off")
    check("native pet drag interrupts play")
    check("interaction panel has no error banner", js("return !document.querySelector('[role=alert]')"))
    fixture.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(ROOT/"public/spritesheet.webp",fixture/"spritesheet.webp")
    (fixture/"pet.json").write_text(json.dumps({"id":"e2e-legacy","displayName":"E2E Legacy","spriteVersionNumber":2,"spritesheetPath":"spritesheet.webp"}))
    invoke("rescan_pets")
    click("拿出球")
    wait(lambda: invoke("play_status")["phase"]=="ready")
    invoke("set_pet",{"id":"e2e-legacy"})
    wait(lambda: invoke("play_status")["phase"]=="off")
    command("POST","/window",{"handle":main})
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===2288"))
    check("legacy atlas loads and pet switching cancels toys",invoke("current_pet")["rows"]==11 and js("return Number(document.querySelector('[data-testid=pet]').dataset.row)<11"))
    invoke("set_pet",{"id":"__builtin__"})
    wait(lambda: js("return document.querySelector('.pet-sheet')?.naturalHeight===5200"))
    check("switching back restores all 25 built-in animation rows")
    command("POST","/window",{"handle":panel})
    if os.environ.get("E2E_MEMORY"):
        # Extended by the next version; kept in the same cumulative native suite.
        exec((ROOT / "scripts/e2e-memory.py").read_text(), globals())
    if os.environ.get("E2E_ENCOUNTERS"):
        exec((ROOT / "scripts/e2e-encounters.py").read_text(), globals())
    OUT.joinpath("e2e.json").write_text(json.dumps({"passed":checks,"version":json.loads(ROOT.joinpath("package.json").read_text())["version"]},ensure_ascii=False,indent=2))
finally:
    error = sys.exc_info()[1]
    if error:
        OUT.joinpath("e2e.json").write_text(json.dumps({"passed":checks,"error":str(error)},ensure_ascii=False,indent=2))
        subprocess.run(["import","-window","root",str(OUT/"failure-desktop.png")],check=False)
    if fixture.exists():
        shutil.rmtree(fixture)
    if session:
        try:
            command("DELETE", "")
        except Exception:
            pass
