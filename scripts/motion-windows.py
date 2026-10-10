#!/usr/bin/env python3
"""Record real Win32 ball/disc movement. Timings are evidence, not a FPS assertion."""
from pathlib import Path
exec((Path(__file__).resolve().parent/'smoke-windows.py').read_text(encoding="utf-8").split('\ntry:\n    process=')[0])
import shutil
samples={}
recording=None
try:
    process=subprocess.Popen([str(Path(args.application).resolve())],stdout=(out/'stdout.log').open('w'),stderr=(out/'stderr.log').open('w'))
    main=wait(lambda:next(iter(windows('大熊')),None),40)
    time.sleep(2)
    ffmpeg=shutil.which('ffmpeg')
    if ffmpeg:
        recording=subprocess.Popen([ffmpeg,'-y','-f','gdigrab','-framerate','60','-i','desktop','-an','-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p',str(out/'motion.mp4')],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=(out/'recording.log').open('w'))
    for label in ['抛一球','扔飞盘']:
        choose(main,label)
        user.SetCursorPos(5,5)
        started=time.perf_counter();trace=[]
        while time.perf_counter()-started<13:
            r=rect(main)
            trace.append([round((time.perf_counter()-started)*1000,3),r.left,r.top])
            time.sleep(.005)
        changed=[trace[0]]+[point for before,point in zip(trace,trace[1:]) if point[1:]!=before[1:]]
        samples[label]={'samples':trace,'position_changes':len(changed)-1,
                       'change_intervals_ms':[round(b[0]-a[0],3) for a,b in zip(changed,changed[1:])],
                       'note':'Intervals include intentional catch/release pauses; inspect alongside video.'}
        check(label+' moves the native pet window',len(changed)>10)
    report['video']='motion.mp4' if recording else 'ffmpeg unavailable; native coordinates only, visual verification pending'
except Exception as error:
    report['error']=str(error)
    raise
finally:
    if recording:
        try:recording.communicate(b'q',timeout=15)
        except subprocess.TimeoutExpired:recording.kill();recording.wait()
    user.SetCursorPos(cursor.x,cursor.y)
    if process:
        process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait()
    (out/'motion.json').write_text(json.dumps({'report':report,'games':samples},ensure_ascii=False,indent=2),encoding='utf-8')
