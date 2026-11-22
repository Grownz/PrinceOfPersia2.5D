"""Finds missing pieces: for one tile type (and wall modifier), the image + offset that explains the most still-unexplained
screenshot pixels over all cells of that type in the sample rooms (consistent offset for all cells), repeated while it gains.
usage: greedy.py <dungeon|palace> <kind> [mod]"""
import sys, os, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import render2d
from eval2d import *

def sample(which, kind, mod, n=14):
    levels = (4, 5, 6, 10, 11, 14) if which == 'palace' else (1, 2, 3, 7, 8, 9, 13)
    out = []
    for lv in levels:
        for room in range(1, 25):
            if not have(lv, room): continue
            fgall, fg, bg, links = room_data(lv, room)
            for col, row, k, m in render2d.cells(fg, bg, links, fgall):
                if k == kind and (mod is None or m == mod): out.append((lv, room, col, row))
            if len(out) >= n * 3: break
        if len(out) >= n * 3: break
    return out[:n * 3]

def run(which, kind, mod, rounds=4, ids=None):
    set_ = 'VPALACE' if which == 'palace' else 'VDUNGEON'; p200 = pal(set_, 200)
    cl = sample(which, kind, mod); rooms = {}
    for lv, room, col, row in cl:
        if (lv, room) not in rooms:
            fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room)
            ok, _, _ = explain(img, fg, bg, set_, room, links, fgall); ok = ok | (img.sum(axis=2) == 0); ok[192:] = True
            rooms[(lv, room)] = (img, ok)
    ids = ids or sorted(int(f.split('_')[1][:-4]) for f in os.listdir(TILES) if f.startswith(set_ + '_') and f.endswith('.bin'))
    ids = [i for i in ids if i not in (200, 360)]
    print('cells', len(cl), 'unexplained in cells', sum(int((~rooms[(lv, r)][1][max(0, 65 + 63 * row - 70):65 + 63 * row + 8, max(0, col * 32 - 8):col * 32 + 40]).sum()) for lv, r, col, row in cl))
    for _ in range(rounds):
        best = (0, None)
        for id_ in ids:
            sp = sprite(set_, id_)
            if sp is None: continue
            colr, op = rgb(sp, p200); h, w = op.shape; ys, xs = np.nonzero(op)
            if len(ys) < 12: continue
            gains = {}
            for lv, room, col, row in cl:
                img, ok = rooms[(lv, room)]; yl = 65 + 63 * row
                for dx in range(-16, 41):
                    for dy in range(-8, 9):
                        py, px = ys + yl + dy - h, xs + col * 32 + dx
                        v = (py >= 0) & (py < 192) & (px >= 0) & (px < 320)
                        if v.sum() < 0.85 * len(ys): continue
                        m = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1)
                        if m.mean() < 0.9: continue
                        gains[(dx, dy)] = gains.get((dx, dy), 0) + int((m & ~ok[py[v], px[v]]).sum())
            if gains:
                k, g = max(gains.items(), key=lambda kv: kv[1])
                if g > best[0]: best = (g, (id_, k[0], k[1]))
        print('best', best, flush=True)
        if best[1] is None or best[0] < 40: break
        id_, dx, dy = best[1]; sp = sprite(set_, id_); colr, op = rgb(sp, p200); h, w = op.shape; ys, xs = np.nonzero(op)
        for lv, room, col, row in cl:
            img, ok = rooms[(lv, room)]; py, px = ys + 65 + 63 * row + dy - h, xs + col * 32 + dx
            v = (py >= 0) & (py < 192) & (px >= 0) & (px < 320)
            m = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1)
            ok[py[v][m], px[v][m]] = True

if __name__ == '__main__':
    run(sys.argv[1], int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else None)
