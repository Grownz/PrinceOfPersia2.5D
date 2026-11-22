"""2D reference of the original's room drawing, built from the pixel evidence of learn.py: for each tile type the images that
are drawn and where (bottom-left corner relative to the cell's left edge and floor line 65 + 63 * row). Not the original's draw
order: pieces are painted in table order, and `explain` only asks whether each screenshot pixel is some piece's pixel.
Used by compare2d.py."""
import os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tiles_lib import *

W, H = 320, 192
BGALL = None      # set by eval2d.room_data: the modifiers of the whole level (for the left neighbour of column 0)
LINKS = None
DECOR_DX = 0

FLOOR = [(241, 0, -2), (243, 0, 1), (242, 0, 0)]
# kind -> [(image id, dx, bottom dy)]; wall and chomper are filled by pieces()
PIECES = {
    1: FLOOR, 2: [(1327, 0, -2), (243, 0, 1)], 3: [(292, 0, -2), (294, 32, -16), (295, 8, -2), (243, 0, 1)],
    4: [(246, 0, -2), (249, 24, -2), (243, 0, 1)], 5: FLOOR, 6: [(241, 0, -2), (296, 0, 1)],
    7: [(246, 0, -2), (249, 24, -2), (243, 0, 1)], 8: [(1286, 0, -2), (1288, 8, -2), (243, 0, 1)], 9: [(1289, 32, 1), (1291, 8, 1)],
    10: FLOOR, 11: FLOOR, 12: [(285, 0, 0), (249, 24, 0)], 13: FLOOR, 14: [(297, 0, -2), (300, 0, -2), (243, 0, 1)],
    15: [(347, 0, -2), (349, 0, 1)], 16: [(241, 0, -2), (243, 0, 1), (237, 32, -2), (238, 32, -62), (234, 40, -66), (299, -2, -3)],
    17: [(243, 0, 1), (240, 32, -62)], 19: FLOOR + [(346, 32, -27)], 21: [(1230, 0, -2), (243, 0, 1)], 22: FLOOR, 30: [(297, 0, -2), (300, 0, -2), (243, 0, 1), (346, 32, -27)],
    23: FLOOR + [(1210, 32, -2), (1211, 32, -62)], 24: [(243, 0, 1), (1213, 32, -62)],
    25: [(292, 0, -2), (295, 8, -2), (243, 0, 1), (1201, 0, -65)], 26: [(1201, 0, -2), (1202, 0, 1), (1209, 0, -55)],
    27: [(1203, 0, -12), (1209, 0, -55)], 28: [(1204, 0, -12), (1209, 0, -55)], 29: [(1205, 0, -12), (1209, 0, -55)],
}
DOOR_SLICES = 13
# PRINCE.DAT items (ids 10000 + resource id): the torch flame frames 151..159 above the holder, the potion bottles 162 / 163
# (palace 164 / 165) and the floor sword 160 / 161
PIECES[19] = PIECES[19] + [(10000 + f, 40, -42) for f in range(151, 160)]
PIECES[30] = PIECES[30] + [(10000 + f, 40, -42) for f in range(151, 160)]
PIECES[10] = PIECES[10] + [(10000 + f, 22, -5) for f in (162, 163, 164, 165)]
PIECES[22] = PIECES[22] + [(10160, 0, -5), (10161, 0, -5)]
CHOMPER = [3, 2, 0, 1, 4, 3, 3]          # frame k by modifier (tiles.rs CHOMPER_FRAME)
WALL_FACE = [368, 370, 366, 364]
WALL_BACK = [367, 369, 365, 363]      # ds:27B6[modifier]: 32x3 pieces above and below the face
FLOOR_RIGHT = (242, 0, 0)

LEARNED = {}       # set name -> {kind: {mod or '*': [[id, dx, dy]]}} from learn_pieces.py
CURRENT_SET = 'VDUNGEON'
def learned(set_):
    if set_ not in LEARNED:
        import json
        f = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'learned_' + ('palace' if set_ == 'VPALACE' else 'dungeon') + '.json')
        LEARNED[set_] = json.load(open(f)) if os.path.exists(f) and not os.environ.get('POP_NO_LEARNED') else {}
    return LEARNED[set_]

def pieces(kind, mod):
    p = list(PIECES.get(kind, []))
    if kind == 18:
        k = CHOMPER[min(mod & 0x7F, 6)]
        p = [(1301 + k, 0, -2), (1306 + k, 0, -2), (243, 0, 1)] + ([(1311 + k - 2, 0, -49)] if k >= 2 else [])
    if kind == 16:
        p += [(233, 40, -15 - 4 * k) for k in range(DOOR_SLICES if mod < 43 else 0)]   # the closed door's panel: slices stacked every 4 px (the mesh is 4 rows tall; the 2 border rows show only on the lowest)
    if kind == 20:
        p = [(WALL_FACE[mod & 3], 0, -2), (WALL_BACK[mod & 3], 0, -62), (WALL_BACK[mod & 3], 0, 1)]
    ex = learned(CURRENT_SET).get(str(kind), {})
    p += [tuple(q) for q in ex.get('*', []) + (ex.get(str(mod & 3), []) if kind == 20 else [])]
    return p

def spr(set_, id_):
    """(sprite, palette) of a piece id; ids >= 10000 are PRINCE.DAT items (10000 + resource id, palette 150)."""
    if id_ >= 10000: return sprite('PRINCE', id_ - 10000), pal('PRINCE', 150)
    if 361 <= id_ <= 377: return sprite(set_, id_), pal(set_, 360)       # chtab 7 (wall pieces) uses palette 360
    return sprite(set_, id_), pal(set_, 200)

class Rnd:
    """The original's random(n) (0C83:0412), as tiles.rs `Rnd`."""
    def __init__(self, seed): self.s = seed & 0xFFFFFFFF
    def next(self, n):
        self.s = (self.s * 214013 + 2531011) & 0xFFFFFFFF
        return (self.s >> 16) % (n + 1)

def wall_rnd(room, row, col):
    v = ([0, 10, 20][min(row, 2)] + room + col) & 0xFFFF
    r = Rnd(v - 65536 if v >= 32768 else v); r.next(1); return r

def dungeon_marks(room, row, col, m):
    """tiles.rs dungeon_wall_marks: [(image id, x, px above the block's bottom line)]."""
    m &= 0x7F
    if m > 3: return []
    r = wall_rnd(room, row, col); a1, a4, b1, b4 = r.next(1), r.next(4), r.next(1), r.next(4); v = []
    mid, bot, top = (371 + a1, 8 + a4, 21), (371 + b1, b4, 0), (373, 0, 42)
    edge = lambda i: (376 + (i & 1), 24 if i < 2 else 8 + a4 - 3, [52, 42, 31, 21][i])
    def crack(i):
        x = 0 if i < 2 else (8 + (a4 - a1) + 6 if i < 4 else (b4 - b1) + 6)
        return (374 + (i & 1), x, [58, 41, 37, 20, 16][i])
    if m == 0:
        if r.next(6) == 0: v.append(crack(r.next(1)))
    elif m == 1:
        if r.next(4) == 0: v.append(top)
        v.append(mid)
        if r.next(4) == 0: v.append(edge(r.next(3)))
        if r.next(4) == 0: v.append(crack(r.next(3)))
    elif m == 2:
        v += [mid, bot]
        if r.next(4) == 0: v.append(edge(r.next(1) + 2))
        if r.next(4) == 0: v.append(crack(r.next(4)))
    else:
        if r.next(4) == 0: v.append(top)
        v += [mid, bot]
        if r.next(4) == 0: v.append(edge(r.next(3)))
        if r.next(4) == 0: v.append(crack(r.next(4)))
    return v

def palace_pattern(room):
    r = Rnd(room); r.next(1); t = []
    for _ in range(3):
        row = []
        for course in range(4):
            base, last, bricks = (0x66 if course % 2 == 0 else 0x61), 255, []
            for _ in range(11):
                c = base + r.next(3)
                while c == last: c = base + r.next(3)
                bricks.append(c); last = c
            row.append(bricks)
        t.append(row)
    return t

def palace_wall_px(room, row, col, pat, set_):
    """tiles.rs palace_wall: (rgb 63x32, mask) of one palace wall tile."""
    p360 = pal(set_, 360); mono = np.array([192, 152, 80])   # the speckle strips' colour on screen (not palette 200's colour 6, which the port uses)
    c = pat[min(row, 2)]; col = min(col, 9)
    px = np.zeros((63, 32), np.uint8)
    def fill(y0, y1, x0, x1, colour): px[y0:y1, x0:x1] = colour - 96
    fill(0, 20, 0, 32, c[0][col]); fill(20, 41, 0, 16, c[1][col]); fill(20, 41, 16, 32, c[1][col + 1])
    fill(41, 60, 0, 8, c[2][col]); fill(41, 60, 8, 32, c[2][col + 1]); fill(60, 63, 0, 32, c[3][col])
    out = p360[np.minimum(px, len(p360) - 1)].copy(); r = wall_rnd(room, row, col)
    for base, x, bottom in ((363, 24, 6), (366, 0, 25), (369, 0, 46), (372, 0, 59), (375, 0, 62)):
        sp = sprite(set_, base + r.next(2))
        if sp is None: continue
        idx, _ = sp; h, w = idx.shape
        for y in range(h):
            for xx in range(w):
                dx, dy = x + xx, bottom - h + 1 + y
                if idx[y, xx] and 0 <= dx < 32 and 0 <= dy < 63: out[dy, dx] = mono
    return out, np.ones((63, 32), bool)

def decor(room, col, row, fg, bg, fgall, palace):
    """tiles.rs back_decor: the background piece a cell gets from its left neighbour: (resource id, px above the cell's floor line)."""
    if fg[row * 10 + col] & 31 == 20: return None
    if col > 0: lk, lm = fg[row * 10 + col - 1] & 31, bg[row * 10 + col - 1]
    else:
        n = LINKS[(room - 1) * 4] if LINKS is not None else 0
        if not n: lk, lm = 20, 0
        else: lk, lm = fgall[(n - 1) * 30 + row * 10 + 9] & 31, BGALL[(n - 1) * 30 + row * 10 + 9]
    if lk == 0 and lm <= 3:
        v, up = [0, 124, 125, 126][lm], [0, 20, 20, 0][lm]
    elif lk == 1:
        m = 0 if lm > 3 else lm
        if m == int(palace): return None
        v, up = [44, 44, 45, 45][m], 20
    else: return None
    return (200 + v, -(up + 2)) if v else None

def left_kind(fg, fgall, links, room, col, row):
    """Tile type left of a cell (through the room link at column 0; 99 = no room)."""
    if col > 0: return fg[row * 10 + col - 1] & 31
    n = links[0] if links else 0
    return (fgall[(n - 1) * 30 + row * 10 + 9] & 31) if n and fgall is not None else 99

def cells(fg, bg, links=None, fgall=None, room=1):
    """Yield (col, row, kind, modifier) of one room; a wall's modifier is the runtime one from its neighbours
    (bit 0: wall on the right, bit 1: on the left; across a room border through `links`, no room = wall)."""
    def wall(c, r):
        if c < 0 or c > 9:
            n = (links or [0, 0, 0, 0])[0 if c < 0 else 1] if links is not None else 0
            if not n or fgall is None: return True
            return fgall[(n - 1) * 30 + r * 10 + (c % 10)] & 31 == 20
        return fg[r * 10 + c] & 31 == 20
    for i in range(30):
        c, r = i % 10, i // 10
        m = bg[i]
        if fg[i] & 31 == 20:
            m = (1 if wall(c + 1, r) else 0) | (2 if wall(c - 1, r) else 0)
        yield c, r, fg[i] & 31, m

MARK_BASE = 1      # bottom line of the block the dungeon wall marks are measured from, relative to the floor line
WALL_BASE = 1      # row of the wall block's bottom line relative to the floor line (the 3-row strip ends at yl + 1)
LAST_RULES_OK = None   # the explained mask before the learned textures were applied (set by explain)
_TEX = {}
def textures(set_):
    if set_ not in _TEX:
        f = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'textures_' + ('palace' if set_ == 'VPALACE' else 'dungeon') + '.npz')
        _TEX[set_] = dict(np.load(f)) if os.path.exists(f) and not os.environ.get('POP_NO_TEX') else {}
    return _TEX[set_]

def explain(img, fg, bg, set_, room=1, links=None, fgall=None):
    """(explained mask, wrong sprite pixels, list of (kind, col, row, id, matching, opaque)) for one room screenshot."""
    global CURRENT_SET
    CURRENT_SET = set_
    pal200 = pal(set_, 200); ok = np.zeros(img.shape[:2], bool); wrong = 0; log = []
    palace = set_ == 'VPALACE'; pat = palace_pattern(room) if palace else None
    def patch(colr, op, x0, y0):
        nonlocal wrong
        ys, xs = np.nonzero(op); py, px = ys + y0, xs + x0
        v = (py >= 0) & (py < img.shape[0]) & (px >= 0) & (px < img.shape[1])
        m = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1)
        ok[py[v][m], px[v][m]] = True; wrong += int((~m).sum())
    for col, row, kind, mod in cells(fg, bg, links, fgall):
        yl = 65 + 63 * row
        if kind == 20 and palace:
            colr, op = palace_wall_px(room, row, col, pat, set_); patch(colr, op, col * 32, yl + WALL_BASE - 63)
        elif kind == 20:
            for id_, dx, up in dungeon_marks(room, row, col, mod):
                sp, pl = spr(set_, id_)
                if sp is None: continue
                colr, op = rgb(sp, pl); h = op.shape[0]
                patch(colr, op, col * 32 + dx, yl + MARK_BASE - up - h)
        dc = decor(room, col, row, fg, bg, fgall, palace) if fgall is not None else None
        for id_, dx, dy in pieces(kind, mod) + ([(dc[0], DECOR_DX, dc[1])] if dc else []):
            sp, pl = spr(set_, id_)
            if sp is None: continue
            colr, op = rgb(sp, pl); h, w = op.shape
            x0, y0 = col * 32 + dx, yl + dy - h
            ys, xs = np.nonzero(op); py, px = ys + y0, xs + x0
            v = (py >= 0) & (py < img.shape[0]) & (px >= 0) & (px < img.shape[1])
            m = (np.abs(img[py[v], px[v]].astype(int) - colr[ys[v], xs[v]].astype(int)) <= TOL).all(axis=1)
            ok[py[v][m], px[v][m]] = True; wrong += int((~m).sum()); log.append((kind, col, row, id_, int(m.sum()), int(v.sum())))
    global LAST_RULES_OK
    LAST_RULES_OK = ok.copy()
    tex = textures(set_)
    if tex and fgall is not None:
        for col, row, kind, mod in cells(fg, bg, links, fgall):
            lk = left_kind(fg, fgall, links, room, col, row)
            key = '%d_%d_%d' % (kind, lk, row)
            if key + '_t' in tex:
                yl = 65 + 63 * row; y0 = yl - 62; blk = img[y0:y0 + 63, col * 32:col * 32 + 32]
                m = tex[key + '_v'] & (np.abs(blk.astype(int) - tex[key + '_t'].astype(int)) <= TOL).all(axis=2)
                ok[y0:y0 + 63, col * 32:col * 32 + 32] |= m
    # some rooms have a coloured back wall instead of black (level 4 vizier hall: red): the colour at the centre of the empty cells
    from collections import Counter
    bgc = Counter(tuple(int(v) for v in img[65 + 63 * row - 40, col * 32 + 16]) for col, row, kind, mod in cells(fg, bg, links, fgall) if kind in (0, 1, 3, 19, 11, 2, 15, 6, 10))
    if bgc:
        c, n = bgc.most_common(1)[0]
        if c != (0, 0, 0) and n >= 5: ok |= (img == np.array(c)).all(axis=2)
    return ok, wrong, log
