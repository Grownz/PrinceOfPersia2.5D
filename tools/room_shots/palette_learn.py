"""The palette the original shows on screen for each palette index of the tile images (palette 200 of VDUNGEON / VPALACE, 150 of
PRINCE.DAT): for every model piece that matches a screenshot, the exact screen colour seen at each of its pixel indices, mode over
all rooms. Writes screen_pal.json; tiles_lib.pal() uses it. (The game's colours differ from the resources by a 6-bit step here
and there.)  usage: palette_learn.py"""
import sys, os, json, collections
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ['POP_NO_SCREEN_PAL'] = '1'
import render2d
from eval2d import *

TOL = 9      # matched against the resource palettes, which differ from the screen by a step or so

def run():
    cnt = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    for lv in range(1, 15):
        set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
        for room in range(1, 25):
            if not have(lv, room): continue
            fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room); render2d.CURRENT_SET = set_
            for col, row, kind, mod in render2d.cells(fg, bg, links, fgall):
                if kind == 20 and set_ == 'VPALACE': continue
                extra = [(i, dx, render2d.MARK_BASE - up) for i, dx, up in render2d.dungeon_marks(room, row, col, mod)] if kind == 20 else []
                for id_, dx, dy in render2d.pieces(kind, mod) + extra:
                    sp, pl = render2d.spr(set_, id_)
                    if sp is None or sp[1] != 4: continue
                    idx = sp[0]; h, w = idx.shape; ys, xs = np.nonzero(idx); py, px = ys + 65 + 63 * row + dy - h, xs + col * 32 + dx
                    v = (py >= 0) & (py < 192) & (px >= 0) & (px < 320)
                    if v.sum() < 0.85 * len(ys): continue
                    seen = img[py[v], px[v]].astype(int); exp = pl[np.minimum(idx[ys[v], xs[v]], len(pl) - 1)].astype(int)
                    m = (np.abs(seen - exp) <= TOL).all(axis=1)
                    if m.mean() < 0.9: continue
                    key = 'PRINCE150' if id_ >= 10000 else set_ + ('360' if 361 <= id_ <= 377 else '200')
                    for i, c in zip(idx[ys[v], xs[v]][m], seen[m]): cnt[key][int(i)][tuple(int(t) for t in c)] += 1
    out = {k: {str(i): list(c.most_common(1)[0][0]) for i, c in d.items() if sum(c.values()) >= 30} for k, d in cnt.items()}
    f = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'screen_pal.json')
    old = json.load(open(f)) if os.path.exists(f) else {}
    for k, d in out.items(): old.setdefault(k, {}).update(d)
    json.dump(old, open(f, 'w'), indent=1)
    for k, d in out.items(): print(k, d)

if __name__ == '__main__': run()
