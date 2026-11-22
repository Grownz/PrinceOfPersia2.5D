"""Records the original in DOSBox tick by tick while keys are injected into its key table (ds:28A0 + scancode), so the same keys can
be replayed in the sim. usage: record.py <Original_data> <level> <script> <out.json>
script: segments "KEYS:TICKS,KEYS:TICKS,...", KEYS from L R U D S (or '-' for none), e.g. "R:6,-:4,RU:1".
The game's 60 Hz timer is at ds:3374 (a logic tick every 5 counts); the kid record at ds:4396, the guard record at ds:431C."""
import sys, os, time, struct, json, ctypes
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ramlib, level_ram as L
SC = {'L': 0x4B, 'R': 0x4D, 'U': 0x48, 'D': 0x50, 'S': 0x2A}
KEYS = 0x28A0
SEED = 0x13572468        # prandom seed written into the original and given to the sim (POP_TRACE_START_SEED)

def parse(script):
    out = []
    for seg in script.split(','):
        k, n = seg.split(':'); out += [('' if k == '-' else k)] * int(n)
    return out

def record(orig, level, script, dump, levels_dat=None):
    exp = open(os.path.join(dump, f'level{level}.bin'), 'rb').read()
    keys = parse(script)
    p, h, tmp = L.launch(level, orig, levels_dat)
    h2 = L.k.OpenProcess(0x0010 | 0x0020 | 0x0008 | 0x0400, False, p.pid)
    def wr(addr, data):
        n = ctypes.c_size_t(); buf = ctypes.create_string_buffer(data); L.k.WriteProcessMemory(h2, ctypes.c_void_p(addr), buf, len(data), ctypes.byref(n))
    try:
        t0 = time.time(); ds = None
        while time.time() - t0 < 30 and ds is None: ds = ramlib.ds_base(h, exp[:90]); time.sleep(0.1)
        if ds is None: raise SystemExit('level array not found')
        rd = lambda off, n: ramlib.rd(h, ds, off, n)
        states = {}                    # counter -> last (kid, guard, room) sample seen at that counter
        phase = None; stand = None; cur = ''; applied = -1
        prev_c = None; seen_changes = []; lastk = None
        t1 = time.time()
        while time.time() - t1 < 60:
            c = struct.unpack('<H', rd(0x3374, 2))[0]; kid = rd(0x4396, 16); guard = rd(0x431C, 16)
            states[c] = (kid.hex(), guard.hex(), rd(0x40C4, 4).hex(), (rd(0x42E8, 1) + rd(0x40C8, 1) + rd(0x4362, 1) + rd(0x40DA, 1) + rd(0x4644, 1)).hex(), (rd(KEYS + 0x4B, 1) + rd(KEYS + 0x4D, 1) + rd(KEYS + 0x48, 1) + rd(KEYS + 0x50, 1) + ramlib.L.read(h, ds + 0x50C2 - ramlib.OFF + 0x20 + 0x417, 1)).hex())
            if lastk is not None and kid[:3] != lastk[:3]: seen_changes.append(c)
            lastk = kid
            if len(seen_changes) >= 6:
                import collections
                phase = collections.Counter(x % 5 for x in seen_changes[-6:]).most_common(1)[0][0]
            if stand is None and phase is not None and kid[0] == 15 and kid[6] == 0 and c % 5 == phase:
                stand = c                       # the tick whose result is the first stand frame
                stand_start = time.time()
            if stand is not None:
                # tick j (j = 1..) is the one whose result shows at counter stand + 5 j; its keys are read at its start: poke in the previous tick
                # tick j (1, 2, ...) shows its result at counter stand + 5 j and reads its keys at its start: poke them in the tick before
                j = (c - stand) // 5 + 1
                if (c - stand) % 5 in (2, 3, 4):
                    want = keys[j - 1] if 1 <= j <= len(keys) else ''
                    if want != cur:
                        for ch, sc in SC.items(): wr(ds + KEYS + sc, bytes([1 if ch in want else 0]))
                        wr(ds + 0x50C2 - ramlib.OFF + 0x20 + 0x417, bytes([1 if 'S' in want else 0]))      # the shift state is read from the BIOS data area (0040:0017)
                        cur = want
                if c > stand + 5 * (len(keys) + 6): break
        return {'level': level, 'phase': phase, 'stand': stand, 'keys': keys, 'states': {str(c): v for c, v in states.items()}}
    finally:
        L.stop(p, tmp)

if __name__ == '__main__':
    orig, level, script, out = sys.argv[1:5]
    dump = os.path.join(os.environ['TEMP'], 'popdump')
    r = record(orig, int(level), script, dump)
    json.dump(r, open(out, 'w')); print('stand', r['stand'], 'phase', r['phase'], 'counters', len(r['states']))
