"""python show.py <level> <room> [out.png]: screenshot | unexplained pixels in magenta (2x)."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from eval2d import *
from PIL import Image
lv, room = int(sys.argv[1]), int(sys.argv[2]); fgall, fg, bg, links = room_data(lv, room); img = shot(lv, room)
ok, wrong, _ = explain(img, fg, bg, 'VPALACE' if lv in PALACE else 'VDUNGEON', room, links, fgall)
from compare2d import asked_room
nb = ~(img.sum(axis=2) == 0); nb[192:] = False; fm = figure_mask(lv, room, kid=asked_room(lv, room) == room); nb[:192] &= ~fm; o = img.copy(); o[~ok & nb] = [255, 0, 255]; o[:192][fm & ~ok[:192]] = [60, 60, 120]
print('explained', round(100 * (1 - (~ok & nb).sum() / nb.sum()), 1), '%')
W = Image.new('RGB', (640, 192)); W.paste(Image.fromarray(img[:192]), (0, 0)); W.paste(Image.fromarray(o[:192]), (320, 0))
W.resize((1280, 384), 0).save(sys.argv[3] if len(sys.argv) > 3 else os.path.join(os.environ['TEMP'], 'cmp', 'show.png'))
