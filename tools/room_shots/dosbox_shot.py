"""Starts PRINCE.EXE in DOSBox 0.74 and captures its window with PrintWindow (no focus or keystrokes needed).
Library: shot(level, orig_dir, seconds, levels_dat=None) -> PIL image of the 320x200 client area."""
import ctypes, os, shutil, subprocess, tempfile, time
from ctypes import wintypes as w
import numpy as np
from PIL import Image
DOSBOX = r'C:\Program Files (x86)\DOSBox-0.74-3\DOSBox.exe'
u, g = ctypes.windll.user32, ctypes.windll.gdi32

def window(pid):
    res = []
    def cb(h, _):
        p = w.DWORD(); u.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u.IsWindowVisible(h):
            n = u.GetWindowTextLengthW(h); b = ctypes.create_unicode_buffer(n + 1); u.GetWindowTextW(h, b, n + 1)
            if b.value.startswith('DOSBox'): res.append(h)
        return True
    u.EnumWindows(ctypes.WINFUNCTYPE(ctypes.c_bool, w.HWND, w.LPARAM)(cb), 0)
    return res[0] if res else None

def grab(h):
    r = w.RECT(); u.GetClientRect(h, ctypes.byref(r)); W, H = r.right, r.bottom
    wdc = u.GetDC(h); mdc = g.CreateCompatibleDC(wdc); bmp = g.CreateCompatibleBitmap(wdc, W, H); g.SelectObject(mdc, bmp)
    u.PrintWindow(h, mdc, 3)                                  # PW_CLIENTONLY | PW_RENDERFULLCONTENT
    class BI(ctypes.Structure):
        _fields_ = [('sz', w.DWORD), ('w', w.LONG), ('h', w.LONG), ('pl', w.WORD), ('bc', w.WORD), ('cp', w.DWORD), ('si', w.DWORD), ('x', w.LONG), ('y', w.LONG), ('cu', w.DWORD), ('ci', w.DWORD)]
    bi = BI(ctypes.sizeof(BI), W, -H, 1, 32, 0, 0, 0, 0, 0, 0)
    buf = ctypes.create_string_buffer(W * H * 4); g.GetDIBits(mdc, bmp, 0, H, buf, ctypes.byref(bi), 0)
    g.DeleteObject(bmp); g.DeleteDC(mdc); u.ReleaseDC(h, wdc)
    return Image.frombuffer('RGBA', (W, H), buf.raw, 'raw', 'BGRA', 0, 1).convert('RGB')

def shot(level, orig, seconds, levels_dat=None):
    tmp = tempfile.mkdtemp(prefix='popshot'); run = os.path.join(tmp, 'c'); shutil.copytree(orig, run)
    if levels_dat: shutil.copy(levels_dat, os.path.join(run, 'LEVELS.DAT'))
    cf = os.path.join(tmp, 'dosbox.conf')
    open(cf, 'w').write(f"[sdl]\nfullscreen=false\nwindowresolution=original\noutput=surface\nautolock=false\n[dosbox]\nmachine=vgaonly\n[cpu]\ncycles=max\n[sblaster]\nsbtype=none\n[midi]\nmpu401=none\n[speaker]\npcspeaker=false\n[joystick]\njoysticktype=none\n[autoexec]\n@echo off\nmount c \"{run}\"\nc:\nprince megahit {level}\nexit\n")
    p = subprocess.Popen([DOSBOX, '-conf', cf, '-noconsole'])
    try:
        t0 = time.time(); h = None
        while not h and time.time() - t0 < 20: h = window(p.pid); time.sleep(0.2)
        # cutscenes (before levels 2, 4, 6, 8, 9, 12) play first: wait for the health triangles of the game screen
        t1 = time.time(); hud = False
        while time.time() - t1 < 45 and not hud:
            time.sleep(0.5); a = np.array(grab(h).convert('RGB')).astype(int)[380:, :80]
            hud = ((a[..., 0] > 150) & (a[..., 1] < 60)).any()
        time.sleep(seconds if hud else 0)
        return grab(h)
    finally:
        p.kill(); p.wait(); shutil.rmtree(tmp, ignore_errors=True)

if __name__ == '__main__':
    import sys
    shot(int(sys.argv[2]), sys.argv[1], float(sys.argv[3])).save(sys.argv[4])
