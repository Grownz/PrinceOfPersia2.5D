"""Where is the rest? Unexplained pixels (figures masked) by tile type of the cell they lie in (and by row), over all rooms."""
import sys, os, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from compare2d import asked_room, NOT_A_ROOM
by = collections.Counter(); tot = collections.Counter(); rooms = {}
for lv in [int(x) for x in sys.argv[1:]] or range(1, 15):
    for room in range(1, 25):
        if not have(lv, room) or (lv, room) in NOT_A_ROOM: continue
        fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room); set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
        ok, _, _ = explain(img, fg, bg, set_, room, links, fgall)
        nb = ~(img.sum(axis=2) == 0); nb[192:] = False; nb[:192] &= ~figure_mask(lv, room, kid=asked_room(lv, room) == room)
        un = ~ok & nb; n = 0
        for i in range(30):
            col, row = i % 10, i // 10; yl = 65 + 63 * row
            blk = un[max(0, yl - 62):yl + 2, col * 32:col * 32 + 32].sum()
            by[(set_, fg[i] & 31)] += int(blk); n += int(blk)
        rooms[(lv, room)] = (int(un.sum()), int(nb.sum()))
print(sorted(((v, k) for k, v in by.items()), reverse=True)[:25])
low = sorted(((1 - u / max(t, 1), k) for k, (u, t) in rooms.items()))
print('rooms < 95 %:', sum(1 for s, k in low if s < 0.95), 'of', len(low))
