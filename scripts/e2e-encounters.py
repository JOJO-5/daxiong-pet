"""Wait for the real scheduler; no forced state transitions or mock event payloads."""
def toggle_encounters(enabled):
    checkbox = command("POST","/element",{"using":"css selector","value":".encounter-heading input"})["element-6066-11e4-a52e-4f735466cecf"]
    if js("return document.querySelector('.encounter-heading input').checked") != enabled:
        command("POST",f"/element/{checkbox}/click",{})
    wait(lambda: invoke("encounter_status")["enabled"]==enabled)

def quiet():
    return invoke("encounter_status")["kind"] is None

toggle_encounters(False)
wait(quiet)
time.sleep(5)
check("disabled encounters never schedule and remove invitation toys",quiet() and invoke("play_status")["phase"]=="off")
restart_app()
check("event preference survives real application restart",not invoke("encounter_status")["enabled"] and not js("return document.querySelector('.encounter-heading input').checked"))
toggle_encounters(True)
invoke("set_pomodoro",{"active":True})
time.sleep(5)
check("real focus session suppresses automatic encounters",quiet())
invoke("set_pomodoro",{"active":False})
invoke("set_visible",{"visible":False})
time.sleep(5)
check("hidden pet schedules no events or toys",quiet() and invoke("play_status")["phase"]=="off")
invoke("set_visible",{"visible":True})
wait(lambda: invoke("plugin:window|is_visible",{"label":"main"}))
pointer("mousemove",30,30)
seen = set()
for _ in range(2):
    event = wait(lambda: (v if (v:=invoke("encounter_status"))["kind"] else None),20)
    kind = event["kind"]
    seen.add(kind)
    if kind == "butterfly":
        command("POST","/window",{"handle":main})
        wait(lambda: js("return !!document.querySelector('[data-testid=butterfly]')"))
        check("naturally scheduled butterfly is rendered in actual pet window")
        command("POST","/window",{"handle":panel})
    else:
        check("naturally scheduled invitation displays independent ball window",invoke("plugin:window|is_visible",{"label":"toy"}) and invoke("play_status")["phase"]=="ready")
    phases = {event["phase"]}
    def ended():
        v=invoke("encounter_status")
        if v["kind"]: phases.add(v["phase"])
        return v["kind"] is None
    wait(ended,12)
    check(f"{kind} event advances and finishes without interaction",len(phases)>=2)
    if kind == "ball":
        wait(lambda: invoke("play_status")["phase"]=="off")
        check("unaccepted invitation cleans up ball")
check("scheduler alternates both event types",seen=={"butterfly","ball"})

# A manual pointer press must accept an invitation rather than erase the ball.
wait(lambda: invoke("encounter_status")["kind"]=="ball",25)
ball = invoke("play_status")["ball"]
pointer("mousemove",*ball);time.sleep(.1)
pointer("mousedown",1);time.sleep(.12)
check("real pointer can grab automatically offered ball",invoke("play_status")["phase"]=="held" and quiet())
pointer("mousemove",ball[0]+160,ball[1]-70);time.sleep(.05);pointer("mouseup",1)
wait(lambda: invoke("play_status")["phase"]=="returned",25)
check("accepted invitation becomes real fetch game")
click("收起玩具")
wait(lambda: invoke("play_status")["phase"]=="off")
wait(lambda: not quiet(),20)
pointer("mousemove","--window",native,150,160);time.sleep(.15)
pointer("mousedown",1);time.sleep(.1)
pointer("mousemove_relative","--",30,0);time.sleep(.15);pointer("mouseup",1)
wait(quiet)
check("native pet dragging immediately interrupts encounter and clears invitation")
pointer("mousemove",30,30)
wait(lambda: not quiet(),20)
toggle_encounters(False)
wait(quiet)
check("turning off events cancels active event immediately")
toggle_encounters(True)
print("WAIT: natural three-minute sleep with automatic encounters enabled",flush=True)
command("POST","/window",{"handle":main})
wait(lambda: js("return document.querySelector('[data-testid=pet]')?.dataset.sleeping==='true'"),195)
check("automatic events do not prevent natural sleep",quiet() and invoke("play_status")["phase"]=="off")
time.sleep(5)
check("sleeping pet stays free of automatic encounters",quiet())
subprocess.run(["import","-window","root",str(OUT/"sleep-desktop.png")],check=True)
pointer("mousemove","--window",native,150,160)
wait(lambda: js("return document.querySelector('[data-testid=pet]')?.dataset.sleeping==='false'"))
check("normal pointer interaction wakes pet after encounter-enabled sleep")
command("POST","/window",{"handle":panel})
subprocess.run(["import","-window","root",str(OUT/"encounters-desktop.png")],check=True)
