import sys, os, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import render2d
from render2d import *
PALACE = (4, 5, 6, 10, 11, 14)
D = os.path.join(os.environ['TEMP'], 'popdump')

def room_data(lv, room):
    d = open(os.path.join(D, f'level{lv}.bin'), 'rb').read(); fgall = list(d[:720]); render2d.BGALL = list(d[720:1440]); render2d.LINKS = list(d[1440:1536])
    return fgall, fgall[(room - 1) * 30:room * 30], list(d[720 + (room - 1) * 30:720 + room * 30]), list(d[1440 + (room - 1) * 4:1440 + room * 4])

def figure_boxes(lv, room, kid=True):
    """Boxes (x0, y0, x1, y1) where the kid (start tile of the screenshot, see all_rooms.start_tile) and the room's guards stand: their
    pixels are left out of the comparison (the figures are not modelled)."""
    import all_rooms
    fgall = room_data(lv, room)[0]; boxes = []
    def box(tile):
        col, row = tile % 10, tile // 10; yl = 65 + 63 * row
        return (col * 32 - 36, yl - 70, col * 32 + 76, yl + 6)
    t = all_rooms.start_tile(fgall, room) if kid else None
    if t is not None: boxes.append(box(t))
    f = os.path.join(D, f'level{lv}.guards')
    if os.path.exists(f):
        for line in open(f):
            r, tl, _ = (int(x) for x in line.split())
            if r == room: boxes.append(box(tl))
    return boxes

def figure_mask(lv, room, kid=True):
    m = np.zeros((192, 320), bool)
    for x0, y0, x1, y1 in figure_boxes(lv, room, kid): m[max(y0, 0):max(y1, 0), max(x0, 0):max(x1, 0)] = True
    return m

def evaluate(rooms, save=None):
    tot = 0; res = []
    for lv, room in rooms:
        fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room)
        ok, wrong, log = explain(img, fg, bg, 'VPALACE' if lv in PALACE else 'VDUNGEON', room, links, fgall)
        un = ~ok & ~(img.sum(axis=2) == 0); un[192:] = False
        res.append((lv, room, int(un.sum()), wrong)); tot += un.sum()
        if save:
            from PIL import Image
            out = img.copy(); out[un] = [255, 0, 255]; Image.fromarray(out).resize((640, 384), 0).save(os.path.join(save, f'un_L{lv}_R{room}.png'))
    return tot, res

if __name__ == '__main__':
    rooms = [(1, 3), (1, 4), (3, 3), (4, 3), (4, 2), (7, 4), (9, 4), (12, 4)]
    print(evaluate(rooms))
