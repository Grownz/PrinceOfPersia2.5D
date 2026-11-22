"""Best (dx, bottom dy) of a sprite relative to the cells of the given kinds: for every candidate offset the mean fraction of the
sprite's opaque pixels equal to the screenshot, over the cells. usage: offset_search.py <id> <kinds> <levels> [dx0 dx1 dy0 dy1]"""
import sys, os, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *

def search(id_, kinds, levels, dxr=(-8, 41), dyr=(-8, 8), maxcells=40):
    cellsl = []
    for lv in levels:
        for room in range(1, 25):
            if not have(lv, room): continue
            fgall, fg, bg, links = room_data(lv, room); img = None
            for i in range(30):
                if fg[i] & 31 in kinds:
                    if img is None: img = shot(lv, room)
                    cellsl.append((lv, img, i % 10, i // 10))
            if len(cellsl) >= maxcells: break
        if len(cellsl) >= maxcells: break
    res = {}
    for lv, img, col, row in cellsl:
        set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'; sp = sprite(set_, id_)
        colr, op = rgb(sp, pal(set_, 200)); h, w = op.shape; ys, xs = np.nonzero(op)
        for dx in range(*dxr):
            for dy in range(*dyr):
                py, px = ys + 65 + 63 * row + dy - h, xs + col * 32 + dx
                v = (py >= 0) & (py < 192) & (px >= 0) & (px < 320)
                if v.sum() < 0.8 * len(ys): continue
                m = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1).mean()
                res.setdefault((dx, dy), []).append(m)
    out = sorted(((np.mean(v), len(v), k) for k, v in res.items() if len(v) >= len(cellsl) * 0.8), reverse=True)[:6]
    return len(cellsl), out

if __name__ == '__main__':
    a = sys.argv
    print(search(int(a[1]), [int(x) for x in a[2].split(',')], [int(x) for x in a[3].split(',')], *([(int(a[4]), int(a[5])), (int(a[6]), int(a[7]))] if len(a) > 7 else [])))
