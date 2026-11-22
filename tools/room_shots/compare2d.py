"""Share of the non-black screenshot pixels (HUD strip, the kid and the guards left out) that the 2D reference model explains, per
level, once with and once without the learned textures (render2d.textures), and the rooms below 95 %.
usage: python compare2d.py [levels...]   (needs out/ and which_room_out.json, the level_dump and tile_export outputs)"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from tiles_lib import _real

NOT_A_ROOM = {(6, 1), (14, 5), (14, 11), (14, 15), (14, 18), (14, 24)}   # level 14's ending screens and a level 6 picture no room explains

def asked_room(lv, room):
    r = _real()
    return int(r[(lv, room)][0].split('_R')[1][:-4]) if r else room

def room_share(lv, room):
    fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room)
    ok, wrong, log = explain(img, fg, bg, 'VPALACE' if lv in PALACE else 'VDUNGEON', room, links, fgall)
    nb = ~(img.sum(axis=2) == 0); nb[192:] = False; nb[:192] &= ~figure_mask(lv, room, kid=asked_room(lv, room) == room)
    rules = render2d.LAST_RULES_OK
    return int((~ok & nb).sum()), int((~rules & nb).sum()), int(nb.sum())

if __name__ == '__main__':
    levels = [int(x) for x in sys.argv[1:]] or range(1, 15); below = []; T = [0, 0, 0]
    for lv in levels:
        un = ur = nb = 0; worst = []
        for room in range(1, 25):
            if not have(lv, room) or (lv, room) in NOT_A_ROOM: continue
            a, b, n = room_share(lv, room); un += a; ur += b; nb += n; worst.append((a, room))
            if n and 1 - a / n < 0.95: below.append((lv, room, round(100 * (1 - a / n), 1)))
        T[0] += un; T[1] += ur; T[2] += nb
        print(f'level {lv}: explained {100 * (1 - un / max(nb, 1)):.1f} % (rules only {100 * (1 - ur / max(nb, 1)):.1f} %) of {nb} non-black pixels; worst rooms {sorted(worst, reverse=True)[:3]}', flush=True)
    print(f'all: {100 * (1 - T[0] / T[2]):.1f} % (rules only {100 * (1 - T[1] / T[2]):.1f} %); rooms below 95 %: {len(below)}')
    print(below)
