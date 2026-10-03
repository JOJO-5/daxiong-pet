#!/usr/bin/env python3
"""Check the real, unshortened encounter interval in a production-timing build."""
import json
import os
from pathlib import Path
import subprocess
import time
from webdriver_http import request

root=Path(__file__).resolve().parents[1]
out=Path(os.environ.get("E2E_OUT", str(root/"test-results")))
out.mkdir(parents=True, exist_ok=True)
session=None

def invoke(name,args=None):
    script="const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke("+json.dumps(name)+","+json.dumps(args or {})+").then(done).catch(e=>done({error:String(e)}));"
    return request("POST",f"/session/{session}/execute/async",{"script":script,"args":[]})

try:
    session=request("POST","/session",{"capabilities":{"alwaysMatch":{"tauri:options":{"application":str(root/"src-tauri/target/debug/daxiong-pet")}}}})["sessionId"]
    subprocess.run(["xdotool","mousemove","30","30"],check=True)
    ready=time.monotonic()
    while not invoke("plugin:window|is_visible",{"label":"main"}):
        assert time.monotonic()-ready<15,"pet failed to show"
        time.sleep(.1)
    start=time.monotonic()
    print("WAIT: actual production encounter interval (90–150 seconds)",flush=True)
    while True:
        event=invoke("encounter_status")
        elapsed=time.monotonic()-start
        if event["kind"] is not None: break
        assert elapsed<170,"no production encounter before natural sleep"
        time.sleep(.25)
    assert 85<=elapsed<=170,("shortened interval leaked into production",elapsed)
    result={"passed":True,"elapsed_seconds":round(elapsed,2),"kind":event["kind"],"timing":"production (no e2e feature)"}
    (out/"production-timing.json").write_text(json.dumps(result,indent=2))
    subprocess.run(["import","-window","root",str(out/"production-encounter-desktop.png")],check=True)
    print("PASS: actual production cadence",json.dumps(result),flush=True)
finally:
    if session:
        request("DELETE",f"/session/{session}")
