"""Compares the level arrays of PRINCE.EXE running in DOSBox 0.74 with pop-core's parse of LEVELS.DAT.
usage: python level_ram.py <Original_data dir> <dump dir from the level_dump example> [levels...]

The emulated RAM is a block inside DOSBox.exe, read with ReadProcessMemory. Where the level array lies in it is the same
every run, so the script finds it once (slow scan, level 1) and then, for each level, polls that spot every millisecond. When
the load-time conversion is done (gates, loose floors, potions, walls, as in docs/tiles.md 1.2) it freezes the process
(NtSuspendProcess) before the game has run a tick, and compares *everything* with what the parser produced: tile types, every
modifier (also torches, spikes, exit doors, chompers, swords, buttons), room links and start. Wall drawing bits are
recomputed here from the original's rule (load_alter_mod 0000:B6DE..B7B9).
Needs DOSBox 0.74-3 (path in DOSBOX). Exit code 1 if any level differs."""
import ctypes, os, shutil, subprocess, sys, tempfile, time
from ctypes import wintypes as w
DOSBOX = r'C:\Program Files (x86)\DOSBox-0.74-3\DOSBox.exe'
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.restype = w.HANDLE
nt = ctypes.WinDLL('ntdll')
WALL, GATE, POTION, LOOSE = 20, 4, 10, 11

class MBI(ctypes.Structure):
    _fields_ = [('Base', ctypes.c_void_p), ('AllocBase', ctypes.c_void_p), ('AllocProt', w.DWORD), ('Part', w.WORD),
                ('Size', ctypes.c_size_t), ('State', w.DWORD), ('Protect', w.DWORD), ('Type', w.DWORD)]

def regions(h, min_size=2305):
    a = 0; m = MBI()
    while k.VirtualQueryEx(h, ctypes.c_void_p(a), ctypes.byref(m), ctypes.sizeof(m)):
        b = m.Base or 0
        if m.State == 0x1000 and m.Protect & 0xEE and not m.Protect & 0x101 and m.Size >= min_size:
            yield b, m.Size
        a = b + m.Size
        if a >= 1 << 47: break

def read(h, addr, n=2305):
    buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
    return buf.raw if k.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)) and got.value == n else None

def find(h, anchor):
    hits = []
    for base, size in regions(h):
        data = read(h, base, size)
        if data is None: continue
        i = data.find(anchor)
        while i >= 0:
            hits.append(base + i); i = data.find(anchor, i + 1)
    return hits

def wall_bits(exp, raw, got_style):
    """Expected runtime modifier of every wall tile (load_alter_mod 0000:B6DE): (m << 7) | 3 with wall on both sides, |2 with a
    wall on the left only, |1 on the right only. Outside the level (no neighbour room) counts as wall. Rooms above
    used_rooms are not altered."""
    fg, bg, links = exp[:720], exp[720:1440], exp[1440:1536]
    used = min(raw[2048], 24); out = {}
    wall = lambda r, t: fg[(r - 1) * 30 + t] & 31 == WALL
    for r in range(1, used + 1):
        for t in range(30):
            if fg[(r - 1) * 30 + t] & 31 != WALL: continue
            v = (bg[(r - 1) * 30 + t] << 7) & 255
            if got_style in (1, 2): v |= 3
            else:
                if t % 10: left = wall(r, t - 1)
                else: L = links[(r - 1) * 4]; left = wall(L, t + 9) if L else True
                if t % 10 != 9: right = wall(r, t + 1)
                else: R = links[(r - 1) * 4 + 1]; right = wall(R, t - 9) if R else True
                v |= 3 if left and right else 2 if left else 1 if right else 0
            out[(r - 1) * 30 + t] = v
    return out

def compare(exp, got, raw, style, level=0, load_only=False):
    """Differences (what, room, tile, expected, found). exp/got = fg 720, bg 720, links 96, start 3.
    load_only: only what the load-time conversion decides (used to see that it is done)."""
    out = []; fg = exp[:720]; walls = wall_bits(exp, raw, style)
    sr = exp[1536]; door = next((i for i in range((sr - 1) * 30, sr * 30) if fg[i] & 31 == 16), None) if 1 <= sr <= 24 else None
    for i in range(720):
        r, t, kind = i // 30 + 1, i % 30, fg[i] & 31
        if got[i] != exp[i]: out.append(('fg', r, t, exp[i], got[i]))
        e, g = exp[720 + i], got[720 + i]
        if kind == WALL and i in walls: e = walls[i]
        if load_only and kind != WALL: continue
        if kind == POTION: e, g = e & 0xE0, g & 0xE0      # the bubble frame (low bits) animates from the first tick
        # what the game does to the room it shows at the start, before the first picture (docs/tiles.md 8.1, 8.2, 5):
        if kind in (19, 30) and g < 9: e = g                            # torches run through flame frames 0..8 (a random one at the start)
        if i == door and g in (0, 43): e = g                            # start_level_door sets the first exit door of the start room to 43 (0 = not yet)
        if level == 13 and kind == LOOSE and t >= 22 and g >= 240: e = g  # check_fall_flo: floors above room 23/16 get a random fall delay
        if e != g: out.append((f'bg(tile type {kind})', r, t, e, g))
    for i in range(96):
        if exp[1440 + i] != got[1952 + i]: out.append(('link', i // 4 + 1, i % 4, exp[1440 + i], got[1952 + i]))
    if tuple(exp[1536:1539]) != tuple(got[2112:2115]): out.append(('start', 0, 0, tuple(exp[1536:1539]), tuple(got[2112:2115])))
    return out

def launch(level, orig, levels_dat=None):
    tmp = tempfile.mkdtemp(prefix='popram')
    run_dir = os.path.join(tmp, 'c'); shutil.copytree(orig, run_dir)
    if levels_dat: shutil.copy(levels_dat, os.path.join(run_dir, 'LEVELS.DAT'))      # e.g. a copy with another start room (tools/room_shots/all_rooms.patched)
    cf = os.path.join(tmp, 'dosbox.conf')
    open(cf, 'w').write(f"[sdl]\nfullscreen=false\nwindowresolution=original\noutput=surface\nautolock=false\n[dosbox]\nmachine=vgaonly\n[cpu]\ncycles=max\n[sblaster]\nsbtype=none\n[midi]\nmpu401=none\n[speaker]\npcspeaker=false\n[joystick]\njoysticktype=none\n[autoexec]\n@echo off\nmount c \"{run_dir}\"\nc:\nprince megahit {level}\nexit\n")
    p = subprocess.Popen([DOSBOX, '-conf', cf, '-noconsole'])
    return p, k.OpenProcess(0x0410 | 0x0800, False, p.pid), tmp

def stop(p, tmp):
    p.kill(); p.wait(); shutil.rmtree(tmp, ignore_errors=True)

def calibrate(orig, dump):
    """(size of the RAM block, offset of the level array in it) from a slow scan of level 1."""
    exp = open(os.path.join(dump, 'level1.bin'), 'rb').read(); raw = open(os.path.join(dump, 'level1.raw'), 'rb').read()
    p, h, tmp = launch(1, orig)
    try:
        time.sleep(6)
        for addr in find(h, exp[:90]):
            data = read(h, addr)
            if data and data != raw[:2305]:      # DOSBox's own file buffer holds the unaltered resource
                for base, size in regions(h):
                    if base <= addr < base + size: return size, addr - base
    finally:
        stop(p, tmp)
    raise SystemExit('level array not found in the DOSBox process')

def run(level, orig, dump, where, wait=30):
    exp = open(os.path.join(dump, f'level{level}.bin'), 'rb').read(); raw = open(os.path.join(dump, f'level{level}.raw'), 'rb').read()
    size, off = where
    p, h, tmp = launch(level, orig)
    try:
        t0 = time.time(); last = None; bases = None
        while time.time() - t0 < wait:
            if bases is None or time.time() - t0 < 3: bases = [b for b, s in regions(h, size) if s == size]   # the RAM block is one of the blocks of that size
            for base in bases:
                got = read(h, base + off)
                style = read(h, base + off - 0x50C2 + 0x3035, 1)
                if not (got and style and got[:720] == exp[:720]): continue
                d = compare(exp, got, raw, style[0], level, load_only=True); last = (style[0], d)
                if not d:
                    nt.NtSuspendProcess(h)            # frozen: nothing the game does after the load can change the picture
                    got = read(h, base + off); style = read(h, base + off - 0x50C2 + 0x3035, 1)
                    return style[0], compare(exp, got, raw, style[0], level)
    finally:
        stop(p, tmp)
    if os.environ.get('DEBUG') and last: print('last state:', last[0], len(last[1]), last[1][:12])
    return None, None

if __name__ == '__main__':
    orig, dump = sys.argv[1], sys.argv[2]
    levels = [int(x) for x in sys.argv[3:]] or range(1, 15)
    where = calibrate(orig, dump); print(f'RAM block {where[0]:#x} bytes, level array at +{where[1]:#x}')
    bad = 0
    for n in levels:
        style, d = run(n, orig, dump, where)
        if d is None: print(f'level {n}: load conversion not seen'); bad += 1; continue
        print(f'level {n}: [ds:3035] = {style}, {len(d)} differences'); bad += bool(d)
        for x in d[:40]: print('   ', *x)
    sys.exit(1 if bad else 0)
