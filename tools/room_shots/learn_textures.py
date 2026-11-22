"""Fixed bitmaps the original draws that are not a single image of the DAT files (the palace wall's side face next to an empty cell, ...):
for every context (tile type, tile type to the left, row) the per-pixel colour that >= AGREE of >= MINN instances share inside the
cell's 32 x 63 block. Written to textures_<set>.npz; render2d.explain uses them last and reports them separately, because they are
measured, not traced. usage: learn_textures.py"""
import sys, os, collections
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from compare2d import asked_room
AGREE, MINN = 0.85, 2

def run():
    blocks = {'VDUNGEON': collections.defaultdict(list), 'VPALACE': collections.defaultdict(list)}
    for lv in range(1, 15):
        set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
        for room in range(1, 25):
            if not have(lv, room): continue
            fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room); fm = figure_mask(lv, room, asked_room(lv, room) == room)
            for row in range(3):
                for col in range(10):
                    kind = fg[row * 10 + col] & 31
                    if kind == 20: continue
                    lk = render2d.left_kind(fg, fgall, links, room, col, row)
                    yl = 65 + 63 * row; y0 = yl - 62
                    if fm[max(y0, 0):y0 + 63, col * 32:col * 32 + 32].any(): continue
                    blocks[set_][(kind, lk, row)].append(img[y0:y0 + 63, col * 32:col * 32 + 32])
    for set_, d in blocks.items():
        out = {}
        for key, v in d.items():
            if len(v) < MINN: continue
            a = np.stack(v); n = len(v); tex = np.zeros((63, 32, 3), np.uint8); valid = np.zeros((63, 32), bool)
            flat = a.reshape(n, -1, 3)
            for p in range(flat.shape[1]):
                c, k = collections.Counter(map(tuple, flat[:, p].tolist())).most_common(1)[0]
                if k / n >= AGREE and c != (0, 0, 0): tex.reshape(-1, 3)[p] = c; valid.reshape(-1)[p] = True
            if valid.sum() >= 40: out['%d_%d_%d' % key + '_t'] = tex; out['%d_%d_%d' % key + '_v'] = valid
        np.savez_compressed(os.path.join(os.path.dirname(os.path.abspath(__file__)), f'textures_{"palace" if set_ == "VPALACE" else "dungeon"}.npz'), **out)
        print(set_, len(out) // 2, 'textures from', sum(len(v) for v in d.values()), 'cells')

if __name__ == '__main__': run()
