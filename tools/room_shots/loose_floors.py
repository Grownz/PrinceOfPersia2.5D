"""Captures a loose floor in the original while it shakes, falls and lands: the start room of a patched LEVELS.DAT gets a floor row
(row 2) and a row of loose floors above it (row 1, columns 2..7), the kid starts on the loose floor of column 4 and it gives way.
Frames go to out_loose/L<level>/ (only changes). usage: python loose_floors.py <Original_data> <level> [seconds]"""
import sys, os, struct, time, shutil, subprocess, tempfile
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dosbox_shot as D

def patch(orig, level, tmp):
    d = bytearray(open(os.path.join(orig, 'LEVELS.DAT'), 'rb').read())
    io = struct.unpack_from('<I', d, 0)[0]; n = struct.unpack_from('<H', d, io)[0]
    for i in range(n):
        id_, off, size = struct.unpack_from('<HIH', d, io + 2 + i * 8)
        if id_ == 2000 + level:
            p = off + 1; room = d[p + 2112]; o = (room - 1) * 30
            for t in range(30): d[p + o + t] = 0; d[p + 720 + o + t] = 0
            for c in range(10): d[p + o + 20 + c] = 1                  # floor row at the bottom
            for c in range(2, 8): d[p + o + 10 + c] = 11               # loose floors above it
            d[p + 2113] = 14                                           # start tile: row 1, column 4 (a loose floor)
            d[off] = (sum(d[p:p + size]) & 255) ^ 255
            f = os.path.join(tmp, 'LEVELS.DAT'); open(f, 'wb').write(d); return f, room
    raise SystemExit('level not found')

def main(orig, level, seconds):
    tmp = tempfile.mkdtemp(prefix='poploose'); lv, room = patch(orig, level, tmp)
    run = os.path.join(tmp, 'c'); shutil.copytree(orig, run); shutil.copy(lv, os.path.join(run, 'LEVELS.DAT'))
    cf = os.path.join(tmp, 'dosbox.conf')
    open(cf, 'w').write(f"[sdl]\nfullscreen=false\nwindowresolution=original\noutput=surface\nautolock=false\n[dosbox]\nmachine=vgaonly\n[cpu]\ncycles=max\n[sblaster]\nsbtype=none\n[midi]\nmpu401=none\n[speaker]\npcspeaker=false\n[joystick]\njoysticktype=none\n[autoexec]\n@echo off\nmount c \"{run}\"\nc:\nprince megahit {level}\nexit\n")
    p = subprocess.Popen([D.DOSBOX, '-conf', cf, '-noconsole']); out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out_loose', f'L{level}'); os.makedirs(out, exist_ok=True)
    try:
        t0 = time.time(); h = None
        while not h and time.time() - t0 < 20: h = D.window(p.pid); time.sleep(0.2)
        # frames from the very start of the level: the HUD triangles appear with the first frame of the game screen
        t1 = time.time(); last = None; k = 0; started = None
        while time.time() - t1 < 60:
            img = D.grab(h); a = np.array(img.resize((320, 200))).astype(int)
            if started is None:
                if ((a[190:, :40, 0] > 150) & (a[190:, :40, 1] < 60)).any(): started = time.time()
                else: continue
            if time.time() - started > seconds: break
            if last is None or (np.abs(a - last).sum() > 0):
                img.save(os.path.join(out, f'f{k:03d}_{int((time.time() - started) * 1000):05d}.png')); k += 1; last = a
        print('room', room, 'frames', k, '->', out)
    finally:
        p.kill(); p.wait(); shutil.rmtree(tmp, ignore_errors=True)

if __name__ == '__main__':
    main(sys.argv[1], int(sys.argv[2]), float(sys.argv[3]) if len(sys.argv) > 3 else 6)
