#!/usr/bin/env python3
"""Real Tauri + WebKitWebDriver + X11 input. No IPC mocks or browser-only harness."""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "test-results"
OUT.mkdir(exist_ok=True)
BASE = "http://127.0.0.1:4444"
session = None
checks = []

def request(method, path, data=None):
    req = urllib.request.Request(BASE + path, data=None if data is None else json.dumps(data).encode(), method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=40) as r:
        value = json.load(r).get("value")
    if isinstance(value, dict) and "error" in value:
        raise AssertionError(value)
    return value

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

def click(text):
    result = command("POST", "/element", {"using": "xpath", "value": f"//button[normalize-space()='{text}']"})
    element = result["element-6066-11e4-a52e-4f735466cecf"]
    command("POST", f"/element/{element}/click", {})

def window(title):
    for handle in command("GET", "/window/handles"):
        command("POST", "/window", {"handle": handle})
        if js("return document.title") == title:
            return handle
    return None

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
    native = subprocess.check_output(["xdotool", "search", "--onlyvisible", "--class", "Daxiong-pet"], text=True).strip().splitlines()[-1]
    initial_handles = set(command("GET", "/window/handles"))
    pointer("mousemove", "--window", native, 150, 160)
    time.sleep(.15)
    pointer("click", 3)
    wait(lambda: len(command("GET", "/window/handles")) >= 3)
    panel = next(iter(set(command("GET", "/window/handles"))-initial_handles))
    command("POST", "/window", {"handle":panel})
    wait(lambda: js("return !!document.querySelector('.play-panel')"))
    check("pet right-click opens real interaction window")
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
    click("拿出球")
    wait(lambda: invoke("play_status")["phase"] == "ready")
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
    if session:
        try:
            command("DELETE", "")
        except Exception:
            pass
