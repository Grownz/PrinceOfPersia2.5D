"""Zoom on the wall cells of one room: screenshot | model for the wall cell's own pieces only | difference. usage: wallview.py lv room col row"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from PIL import Image
lv, room, col, row = [int(x) for x in sys.argv[1:5]]
fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room); set_ = 'VPALACE' if lv in PALACE else 'VDUNGEON'
render2d.CURRENT_SET = set_
mod = [m for c, r, k, m in render2d.cells(fg, bg, links, fgall) if (c, r) == (col, row)][0]
print('kind', fg[row * 10 + col] & 31, 'mod', mod)
yl = 65 + 63 * row; x0, y0 = col * 32, yl - 62
canvas = np.zeros((64, 32, 3), np.uint8)
def paint(id_, dx, dy):
    sp, pl = render2d.spr(set_, id_)
    if sp is None: return
    colr, op = rgb(sp, pl); h, w = op.shape
    for y in range(h):
        for x in range(w):
            if op[y, x]:
                py, px = yl + dy - h + y - y0, col * 32 + dx + x - x0
                if 0 <= py < 64 and 0 <= px < 32: canvas[py, px] = colr[y, x]
if set_ == 'VPALACE':
    c, op = render2d.palace_wall_px(room, row, col, render2d.palace_pattern(room), set_)
    for y in range(63):
        for x in range(32):
            py = yl + render2d.WALL_BASE - 63 + y - y0
            if 0 <= py < 64: canvas[py, x] = c[y, x]
else:
    for id_, dx, dy in render2d.pieces(20, mod)[:3]: paint(id_, dx, dy)
    for id_, dx, up in render2d.dungeon_marks(room, row, col, mod):
        sp, pl = render2d.spr(set_, id_); paint(id_, dx, render2d.MARK_BASE - up)
real = img[y0:y0 + 64, x0:x0 + 32]
diff = real.copy(); bad = (np.abs(real.astype(int) - canvas.astype(int)) > TOL).any(axis=2); diff[bad] = [255, 0, 255]
W = Image.new('RGB', (32 * 3 * 8 + 20, 64 * 8)); 
for i, a in enumerate((real, canvas, diff)): W.paste(Image.fromarray(a).resize((256, 512), 0), (i * 266, 0))
W.save(os.path.join(os.environ['TEMP'], 'cmp', 'wall.png')); print('bad px', int(bad.sum()))
