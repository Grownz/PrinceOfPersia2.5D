"""Which room does each screenshot really show? Shots whose requested room explains < 60 % of the pixels are matched against all
rooms of the level (the kid at the left or right edge leaves the room). Writes which_room.json {file: [requested, best, share]}."""
import sys, os, glob, json
import numpy as np
from PIL import Image
os.environ['POP_DIRECT_SHOTS'] = '1'
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
out = {}
for f in sorted(glob.glob(os.path.join(SHOTS, 'L*_R*.png'))):
    lv = int(os.path.basename(f)[1:].split('_')[0]); room = int(os.path.basename(f).split('_R')[1][:-4])
    img = np.array(Image.open(f).convert('RGB')); nb = ~(img.sum(axis=2) == 0); nb[192:] = False; set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
    def share(r):
        fgall, fg, bg, links = room_data(lv, r); ok, _, _ = explain(img, fg, bg, set_, r, links, fgall)
        return 1 - (~ok & nb).sum() / max(nb.sum(), 1)
    s = share(room); best = room
    if s < 0.8:            # the kid left the room: the shot shows a neighbour (or its neighbour)
        links = room_data(lv, room)[3]; near = {n for n in links if n}
        for n in list(near): near |= {m for m in room_data(lv, n)[3] if m}
        cand = [(share(r), r) for r in sorted(near) if r != room and any(b & 31 for b in room_data(lv, r)[1])]
        if cand:
            bs, br = max(cand)
            if bs > s + 0.1: s, best = bs, br
    out[os.path.basename(f)] = [room, best, round(float(s), 3)]
    print(os.path.basename(f), room, best, round(float(s), 3), flush=True)
json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'which_room_' + os.path.basename(os.path.normpath(SHOTS)) + '.json'), 'w'))
