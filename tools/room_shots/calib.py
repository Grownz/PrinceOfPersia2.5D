"""Where does sprite <id> sit relative to the cell it belongs to? Histogram over all rooms of the given levels."""
import sys, os, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tiles_lib import *
DUMP = os.path.join(os.environ['TEMP'], 'popdump')
PALACE = (4, 5, 6, 10, 11, 14)

def fg_of(level): return open(os.path.join(DUMP, f'level{level}.bin'), 'rb').read()[:720]

def calibrate(kind, ids, levels, win=(-12, 12, -40, 12), min_frac=0.98, limit=40):
    """For every cell of tile type `kind`: best match of each id near (col*32, row line)."""
    hist = {i: collections.Counter() for i in ids}; seen = 0
    for lv in levels:
        set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'; p = pal(set_, 200); fg = fg_of(lv)
        for room in range(1, 25):
            f = os.path.join(SHOTS, f'L{lv}_R{room}.png')
            if not os.path.exists(f): continue
            img = shot(lv, room)
            for idx in range(30):
                if fg[(room - 1) * 30 + idx] & 31 != kind: continue
                col, row = idx % 10, idx // 10
                x0, yl = col * 32, 65 + 63 * row
                seen += 1
                for i in ids:
                    sp = sprite(set_, i)
                    if sp is None: continue
                    h, w = sp[0].shape
                    ms = matches(img, sp, p, min_frac, (x0 + win[0], yl + win[2] - h, x0 + win[1] + 1, yl + win[3] - h + 1))
                    if ms:
                        best = max(ms, key=lambda m: (m[2], -abs(m[0] - x0) - abs(m[1] + h - yl)))
                        hist[i][(best[0] - x0, best[1] + h - yl)] += 1
                if seen >= limit: break
            if seen >= limit: break
        if seen >= limit: break
    return seen, hist

if __name__ == '__main__':
    kind = int(sys.argv[1]); ids = [int(x) for x in sys.argv[2].split(',')]
    seen, hist = calibrate(kind, ids, [int(x) for x in sys.argv[3].split(',')], limit=int(sys.argv[4]) if len(sys.argv) > 4 else 40)
    print('cells', seen)
    for i in ids: print(i, hist[i].most_common(6))
