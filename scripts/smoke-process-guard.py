#!/usr/bin/env python3
"""Native POSIX guard lifetime contract; synthetic child, no model or screen data."""
import argparse, json, os, signal, subprocess, sys, tempfile, time
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--guardian',type=Path,required=True);parser.add_argument('--out',type=Path,required=True);args=parser.parse_args()
guard=str(args.guardian.resolve());checks=[]
def wait(fn,seconds=5):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if fn():return
        time.sleep(.05)
    raise AssertionError('process lifetime check timed out')
def alive(pid):
    try:os.kill(pid,0);return True
    except ProcessLookupError:return False
with tempfile.TemporaryDirectory() as directory:
    root=Path(directory);child_script=root/'child.py';child_pid=root/'child.pid';guard_pid=root/'guard.pid'
    child_script.write_text('import os,time,sys\nfrom pathlib import Path\nPath(sys.argv[1]).write_text(str(os.getpid()))\ntime.sleep(60)\n')
    normal=subprocess.Popen([guard,str(os.getpid()),'--',sys.executable,str(child_script),str(child_pid)])
    try:
        wait(child_pid.exists);pid=int(child_pid.read_text());normal.terminate();normal.wait(timeout=5);wait(lambda:not alive(pid));checks.append('normal guard termination kills and reaps model child')
    finally:
        if normal.poll()is None:normal.kill();normal.wait()
    child_pid.unlink()
    parent_script=root/'parent.py';parent_script.write_text('import os,subprocess,time,sys\nfrom pathlib import Path\ng=subprocess.Popen([sys.argv[1],str(os.getpid()),"--",sys.executable,sys.argv[2],sys.argv[3]])\nPath(sys.argv[4]).write_text(str(g.pid))\nwhile not Path(sys.argv[3]).exists():time.sleep(.05)\nos._exit(42)\n')
    parent=subprocess.Popen([sys.executable,str(parent_script),guard,str(child_script),str(child_pid),str(guard_pid)])
    try:
        wait(child_pid.exists);pid=int(child_pid.read_text());parent.wait(timeout=5);assert parent.returncode==42;wait(lambda:not alive(pid));checks.append('abnormal desktop parent exit kills and reaps model child')
    finally:
        if parent.poll()is None:parent.kill();parent.wait()
        if guard_pid.exists()and alive(int(guard_pid.read_text())):
            try:os.kill(int(guard_pid.read_text()),signal.SIGTERM)
            except ProcessLookupError:pass
args.out.mkdir(parents=True,exist_ok=True);(args.out/'process-guard.json').write_text(json.dumps({'passed':checks},indent=2)+'\n',encoding='utf-8');print('Native parent-exit and cancellation guard passed')
