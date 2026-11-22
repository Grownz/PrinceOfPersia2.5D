"""Learns the pieces the model still lacks, per tile type (walls per modifier): the image + offset that explains most of the
still-unexplained pixels over all sample cells, repeated while it gains >= MIN pixels. Writes learned_<set>.json
{kind: {mod or "*": [[id, dx, dy], ...]}}, which render2d reads. usage: learn_pieces.py <dungeon|palace> [kinds...]"""
import sys, os, json
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import render2d
from eval2d import *
MIN = int(os.environ.get('POP_MIN', 120))
DXR = (-16, 40); DYR = tuple(int(v) for v in os.environ.get("POP_DYR", "-10,10").split(","))

def cells_of(which, kind, mod, limit=45):
    levels = (4, 5, 6, 10, 11, 14) if which == 'palace' else (1, 2, 3, 7, 8, 9, 12, 13)
    out = []
    for lv in levels:
        for room in range(1, 25):
            if not have(lv, room): continue
            fgall, fg, bg, links = room_data(lv, room)
            for col, row, k, m in render2d.cells(fg, bg, links, fgall):
                if k == kind and (mod is None or m == mod): out.append((lv, room, col, row))
        if len(out) >= limit: break
    return out[:limit]

def learn(which, kind, mod):
    set_ = 'VPALACE' if which == 'palace' else 'VDUNGEON'; p200 = pal(set_, 200)
    cl = cells_of(which, kind, mod)
    if len(cl) < 3: return []
    state = {}
    for lv, room, col, row in cl:
        if (lv, room) not in state:
            fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room)
            ok, _, _ = explain(img, fg, bg, set_, room, links, fgall); ok = ok | (img.sum(axis=2) == 0); ok[192:] = True
            state[(lv, room)] = (img, ok)
    ids = sorted(int(f.split('_')[1][:-4]) for f in os.listdir(TILES) if f.startswith(set_ + '_') and f.endswith('.bin'))
    ids = [i for i in ids if i not in (200, 360)]; found = []
    from scipy.signal import fftconvolve
    P = 24   # padding of the screen maps (offsets reach outside the picture)
    p150 = pal('PRINCE', 150)
    def maps(key):
        img, ok = state[key]; out = []
        for pl in (p200, p150):
            A = {c: np.pad((np.abs(img.astype(int) - pl[c].astype(int)) <= TOL).all(axis=2), P) for c in range(1, len(pl))}
            out.append((A, {c: A[c] & np.pad(~ok, P) for c in A}))
        return out
    M = {k: maps(k) for k in state}
    sprites = {}
    for id_ in ids:
        sp = sprite(set_, id_)
        if sp is not None and sp[1] == 4 and (sp[0] != 0).sum() >= 12: sprites[id_] = sp[0]
    for id_ in range(150, 176):                      # PRINCE.DAT items: flames 151..159, sword 160/161, bottles 162..165, bubbles 166..172
        sp = sprite('PRINCE', id_)
        if sp is not None and sp[1] == 4 and (sp[0] != 0).sum() >= 6: sprites[10000 + id_] = sp[0]
    nx, ny = DXR[1] - DXR[0] + 1, DYR[1] - DYR[0] + 1
    for _ in range(int(os.environ.get('POP_ROUNDS', 6))):
        best = (0, None)
        for id_, idx in sprites.items():
            h, w = idx.shape; n = int((idx != 0).sum()); gains = np.zeros((ny, nx))
            cs = [c for c in np.unique(idx) if c != 0 and c < len(p200)]
            masks = {c: (idx == c).astype(float)[::-1, ::-1] for c in cs}
            for lv, room, col, row in cl:
                A, AU = M[(lv, room)][1 if id_ >= 10000 else 0]; yl = 65 + 63 * row
                # sprite top-left for offset (dx, dy): x = col*32 + dx, y = yl + dy - h
                x0, y0 = col * 32 + DXR[0] + P, yl + DYR[0] - h + P
                crop = (slice(y0, y0 + h + ny - 1), slice(x0, x0 + w + nx - 1))
                if y0 < 0 or x0 < 0 or y0 + h + ny - 1 > 192 + 2 * P or x0 + w + nx - 1 > 320 + 2 * P: continue
                mt = sum(fftconvolve(A[c][crop], masks[c], mode='valid') for c in cs)
                gn = sum(fftconvolve(AU[c][crop], masks[c], mode='valid') for c in cs)
                gains += np.where(mt >= 0.9 * n - 0.5, gn, 0)
            i = np.unravel_index(int(gains.argmax()), gains.shape)
            if gains[i] > best[0]: best = (int(round(gains[i])), (id_, DXR[0] + int(i[1]), DYR[0] + int(i[0])))
        if best[1] is None or best[0] < MIN: break
        found.append(list(best[1])); print(which, kind, mod, best, flush=True)
        id_, dx, dy = best[1]; idx = sprites[id_]; h = idx.shape[0]
        for lv, room, col, row in cl:
            img, ok = state[(lv, room)]; colr, op = rgb((idx, 4), p150 if id_ >= 10000 else p200); ys, xs = np.nonzero(op)
            py, px = ys + 65 + 63 * row + dy - h, xs + col * 32 + dx
            v = (py >= 0) & (py < 192) & (px >= 0) & (px < 320)
            mm = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1)
            ok[py[v][mm], px[v][mm]] = True
        M = {k: maps(k) for k in state}
    return found

if __name__ == '__main__':
    which = sys.argv[1]; kinds = [int(x) for x in sys.argv[2:]] or [1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30]
    f = os.path.join(os.path.dirname(os.path.abspath(__file__)), f'learned_{which}.json')
    res = json.load(open(f)) if os.path.exists(f) else {}
    for kind in kinds:
        for mod in ((0, 1, 2, 3) if kind == 20 else (None,)):
            res.setdefault(str(kind), {})['*' if mod is None else str(mod)] = learn(which, kind, mod)
            json.dump(res, open(f, 'w'), indent=1)
