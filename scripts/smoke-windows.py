#!/usr/bin/env python3
"""Native Win32 input/window smoke test. Run in an interactive Windows session."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
import subprocess
import time

parser=argparse.ArgumentParser()
parser.add_argument('--application',required=True)
parser.add_argument('--out',default='test-results/windows')
args=parser.parse_args()
out=Path(args.out);out.mkdir(parents=True,exist_ok=True)
report={'platform':'Windows','passed':[],'error':None,'scope':'native launch, click-through, head/belly contact, menu, panel and mouse-held tug; single display'}
user=c.WinDLL('user32',use_last_error=True)
user.SetProcessDPIAware()
user.EnumWindows.argtypes=[c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM),w.LPARAM]
user.GetWindowThreadProcessId.argtypes=[w.HWND,c.POINTER(w.DWORD)]
user.GetWindowRect.argtypes=[w.HWND,c.POINTER(w.RECT)]
user.GetClientRect.argtypes=[w.HWND,c.POINTER(w.RECT)]
user.GetWindowTextW.argtypes=[w.HWND,w.LPWSTR,c.c_int]
user.GetClassNameW.argtypes=[w.HWND,w.LPWSTR,c.c_int]
user.IsWindowVisible.argtypes=[w.HWND]
user.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
user.SetForegroundWindow.argtypes=[w.HWND]
user.GetWindowLongW.argtypes=[w.HWND,c.c_int]
user.GetWindowLongW.restype=w.LONG
user.mouse_event.argtypes=[w.DWORD,w.DWORD,w.DWORD,w.DWORD,c.c_size_t]
user.keybd_event.argtypes=[w.BYTE,w.BYTE,w.DWORD,c.c_size_t]
user.SetCursorPos.argtypes=[c.c_int,c.c_int]
user.SendMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM];user.SendMessageW.restype=c.c_ssize_t
user.GetMenuStringW.argtypes=[w.HMENU,w.UINT,w.LPWSTR,c.c_int,w.UINT]
user.GetMenuItemCount.argtypes=[w.HMENU]
user.GetMenuItemRect.argtypes=[w.HWND,w.HMENU,w.UINT,c.POINTER(w.RECT)]
process=None
cursor=w.POINT();user.GetCursorPos(c.byref(cursor))

def windows(title=None):
    found=[]
    @c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
    def collect(hwnd,_):
        pid=w.DWORD();user.GetWindowThreadProcessId(hwnd,c.byref(pid))
        if process and pid.value==process.pid and user.IsWindowVisible(hwnd):
            text=c.create_unicode_buffer(256);user.GetWindowTextW(hwnd,text,256)
            if title is None or text.value==title:found.append(hwnd)
        return True
    user.EnumWindows(collect,0)
    return found

def rect(hwnd):
    r=w.RECT();assert user.GetWindowRect(hwnd,c.byref(r));return r

def wait(fn,seconds=20):
    until=time.monotonic()+seconds
    while time.monotonic()<until:
        if process.poll() is not None:raise AssertionError(f'application exited: {process.returncode}')
        value=fn()
        if value:return value
        time.sleep(.08)
    raise AssertionError('native check timed out')

def check(name,condition=True):
    assert condition,name
    report['passed'].append(name);print('PASS:',name,flush=True)

def key(code):
    user.keybd_event(code,0,0,0);user.keybd_event(code,0,2,0);time.sleep(.12)

def choose(hwnd,label):
    r=rect(hwnd);scale=(r.right-r.left)/300
    user.SetForegroundWindow(hwnd)
    assert user.SetCursorPos(round(r.left+150*scale),round(r.top+160*scale)),f'SetCursorPos: {c.get_last_error()}'
    time.sleep(.35)
    actual=w.POINT();assert user.GetCursorPos(c.byref(actual))
    report['menu_pointer']={'requested':[round(r.left+150*scale),round(r.top+160*scale)],'actual':[actual.x,actual.y],
        'extended_at_pet':hex(user.GetWindowLongW(hwnd,-20)&0xffffffff)}
    check('hovered pet accepts native input',not user.GetWindowLongW(hwnd,-20)&0x00000020)
    user.mouse_event(0x0008,0,0,0,0);user.mouse_event(0x0010,0,0,0,0)
    def popup():
        for window in windows():
            name=c.create_unicode_buffer(256);user.GetClassNameW(window,name,256)
            if name.value=='#32768':return window
    native_menu=wait(popup)
    menu_handle=user.SendMessageW(native_menu,0x01E1,0,0) # MN_GETHMENU
    assert menu_handle,'native popup has no HMENU'
    names=[];item=None
    for index in range(user.GetMenuItemCount(menu_handle)):
        text=c.create_unicode_buffer(256);user.GetMenuStringW(menu_handle,index,text,256,0x0400)
        names.append(text.value)
        if text.value.startswith(label):item=index
    report['native_menu_items']=names
    assert item is not None,f'menu item missing: {label}: {names}'
    target=w.RECT();assert user.GetMenuItemRect(None,menu_handle,item,c.byref(target))
    user.SetCursorPos((target.left+target.right)//2,(target.top+target.bottom)//2);time.sleep(.12)
    user.mouse_event(0x0002,0,0,0,0);user.mouse_event(0x0004,0,0,0,0)

def capture(name):
    target=str((out/name).resolve()).replace("'","''")
    code="Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $r=[System.Windows.Forms.SystemInformation]::VirtualScreen; $b=New-Object System.Drawing.Bitmap $r.Width,$r.Height; $g=[System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($r.Left,$r.Top,0,0,$r.Size); $b.Save('"+target+"'); $g.Dispose(); $b.Dispose()"
    subprocess.run(['powershell','-NoProfile','-Command',code],check=True)

try:
    process=subprocess.Popen([str(Path(args.application).resolve())],stdout=(out/'stdout.log').open('w'),stderr=(out/'stderr.log').open('w'))
    main=wait(lambda:next(iter(windows('大熊')),None),40)
    time.sleep(1)
    check('built-in pet creates a visible native window')
    style=user.GetWindowLongW(main,-16);extended=user.GetWindowLongW(main,-20)
    r=rect(main);client=w.RECT();assert user.GetClientRect(main,c.byref(client))
    report['native_style']={'style':hex(style&0xffffffff),'extended':hex(extended&0xffffffff),
        'outer':[r.left,r.top,r.right,r.bottom],'client':[client.left,client.top,client.right,client.bottom]}
    # Tao can retain caption style bits while removing the non-client frame with WM_NCCALCSIZE.
    check('pet has no non-client border',r.right-r.left==client.right-client.left and r.bottom-r.top==client.bottom-client.top)
    check('pet is always on top',bool(extended&0x00000008))
    user.SetCursorPos(5,5);time.sleep(.4)
    check('transparent desktop area is click-through',bool(user.GetWindowLongW(main,-20)&0x00000020))
    def hover_pet(x,y):
        current=rect(main);scale=(current.right-current.left)/300
        user.SetCursorPos(round(current.left+x*scale),round(current.top+y*scale))
        return current.left,current.top
    before_touch=hover_pet(145,125);time.sleep(2)
    capture('head-rub.png')
    current=rect(main)
    check('head contact keeps the native window stationary',(current.left,current.top)==before_touch)
    user.SetCursorPos(5,5);time.sleep(5)
    before_touch=hover_pet(178,199);time.sleep(1.8)
    hover_pet(172,145);time.sleep(1.5)
    capture('belly-rub.png')
    current=rect(main)
    check('belly contact keeps the native window stationary',(current.left,current.top)==before_touch)
    report['petting_visuals']='real pointer head/belly screenshots; pose inspection is separate from stationary checks'
    user.SetCursorPos(5,5);time.sleep(1.5)
    choose(main,"打开互动面板")
    panel=wait(lambda:next(iter(windows('和大熊一起玩')),None))
    check('real right-click menu opens interaction panel')
    user.PostMessageW(panel,0x0010,0,0)
    wait(lambda:not windows('和大熊一起玩'))
    check('closing panel keeps pet alive',process.poll() is None)
    choose(main,"一起拔河")
    toy=wait(lambda:next(iter(windows('大熊的球')),None))
    r=rect(main);scale=(r.right-r.left)/300
    # The engine chooses the side with more work-area room on this display.
    class MonitorInfo(c.Structure):
        _fields_=[('size',w.DWORD),('monitor',w.RECT),('work',w.RECT),('flags',w.DWORD)]
    user.MonitorFromWindow.argtypes=[w.HWND,w.DWORD];user.MonitorFromWindow.restype=w.HANDLE
    user.GetMonitorInfoW.argtypes=[w.HANDLE,c.POINTER(MonitorInfo)]
    info=MonitorInfo();info.size=c.sizeof(info)
    assert user.GetMonitorInfoW(user.MonitorFromWindow(main,2),c.byref(info))
    direction=1 if (r.left+r.right)/2<(info.work.left+info.work.right)/2 else -1
    tr=rect(toy);hx=round((tr.left+tr.right)/2+direction*40*scale);hy=round((tr.top+tr.bottom)/2)
    user.SetCursorPos(hx,hy);time.sleep(.25);user.mouse_event(0x0002,0,0,0,0)
    time.sleep(.2);before=rect(main)
    user.SetCursorPos(round(hx+direction*70*scale),hy);time.sleep(2.1)
    after=rect(main)
    check('real held rope makes pet resist',before.left!=after.left)
    capture('tug-pulling.png')
    user.mouse_event(0x0004,0,0,0,0)
    wait(lambda:not windows('大熊的球'),1.5)
    check('release hides rope for celebration')
    wait(lambda:windows('大熊的球'),5)
    check('rope returns for another round')
    capture('tug-ready.png')
except Exception as error:
    report['error']=str(error)
    try:capture('failure.png')
    except Exception as capture_error:report['capture_error']=str(capture_error)
    raise
finally:
    user.mouse_event(0x0004,0,0,0,0);user.SetCursorPos(cursor.x,cursor.y)
    if process:
        process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait()
    (out/'native-smoke.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
