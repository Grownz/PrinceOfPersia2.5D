"""Which images explain the unexplained pixels of one room? Whole-picture search of every image (also PRINCE.DAT items) at every
position: usage residual_sprites.py <level> <room> [n]"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from compare2d import asked_room
from scipy.signal import fftconvolve
lv, room = int(sys.argv[1]), int(sys.argv[2]); n = int(sys.argv[3]) if len(sys.argv) > 3 else 12
fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room); set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
ok, _, _ = explain(img, fg, bg, set_, room, links, fgall)
nb = ~(img.sum(axis=2) == 0); nb[192:] = False; nb[:192] &= ~figure_mask(lv, room, kid=asked_room(lv, room) == room)
un = ~ok & nb; print('unexplained', int(un.sum()))
res = []
for id_ in list(range(200, 400)) + list(range(1200, 1345)) + [10000 + i for i in range(150, 176)]:
    sp, pl = render2d.spr(set_, id_)
    if sp is None or sp[1] != 4: continue
    idx = sp[0]; h, w = idx.shape; k = int((idx != 0).sum())
    if k < 20: continue
    mt = 0; gn = 0
    for c in np.unique(idx):
        if c == 0 or c >= len(pl): continue
        A = (np.abs(img[:192].astype(int) - pl[c].astype(int)) <= TOL).all(axis=2)
        m = (idx == c).astype(float)[::-1, ::-1]
        mt = mt + fftconvolve(np.pad(A, ((h, h), (w, w))), m, mode='valid')
        gn = gn + fftconvolve(np.pad(A & un[:192], ((h, h), (w, w))), m, mode='valid')
    good = np.where(mt >= 0.97 * k - 0.5, gn, 0); i = np.unravel_index(int(good.argmax()), good.shape)
    if good[i] >= 30: res.append((int(round(good[i])), id_, int(i[1]) - w, int(i[0]) - h, h, w))
for g, id_, x, y, h, w in sorted(res, reverse=True)[:n]:
    print(f'id {id_}: gains {g} at x {x}, top y {y} (bottom {y + h}); cell col {x / 32:.2f}, floor lines {[65 + 63 * r for r in range(3)]}')
