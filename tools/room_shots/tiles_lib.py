"""Tile images (exported by the tile_export example) and a template matcher for the DOSBox room screenshots."""
import os, numpy as np
from PIL import Image
TILES = os.environ.get('POP_TILES_DIR') or os.path.join(os.environ.get('TEMP', '/tmp'), 'tiles')
TOL = 3   # the screen palettes are learned (palette_learn.py), so colours must match almost exactly
SHOTS = os.environ.get('POP_SHOTS_DIR') or os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')

_SCREEN = None
def screen_pal():
    """{'VDUNGEON200': {index: rgb}, ...} learned by palette_learn.py (the colours the game really shows), or {}."""
    global _SCREEN
    if _SCREEN is None:
        import json
        f = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'screen_pal.json')
        _SCREEN = json.load(open(f)) if os.path.exists(f) and not os.environ.get('POP_NO_SCREEN_PAL') else {}
    return _SCREEN

def pal(set_, id_):
    # DOSBox shows a 6-bit VGA value v as 4 * v (the export scaled it to 0..255)
    a = np.array([[int(v) for v in l.split()] for l in open(os.path.join(TILES, f'{set_}_pal{id_}.txt'))])
    out = (np.rint(a * 63 / 255) * 4).astype(np.uint8)
    for i, c in screen_pal().get(('PRINCE150' if set_ == 'PRINCE' else f'{set_}{id_}'), {}).items(): out[int(i)] = c
    return out

def sprite(set_, id_):
    """(index array h x w, bpp) or None."""
    f = os.path.join(TILES, f'{set_}_{id_}.bin')
    if not os.path.exists(f): return None
    d = open(f, 'rb').read(); w, h, bpp = d[0] | d[1] << 8, d[2] | d[3] << 8, d[4]
    return np.frombuffer(d[5:], dtype=np.uint8).reshape(h, w), bpp

_REAL = None
def _dirs():
    """Shot folders: out/ and, if it exists, out2/ (a second pass for rooms the first never showed)."""
    d = [SHOTS]; second = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out2')
    return d + ([second] if os.path.isdir(second) and not os.environ.get('POP_SHOTS_DIR') else [])

def _real():
    """{(level, room shown): (file, share, folder)}: a shot can show another room than its name says (which_room_<folder>.json)."""
    global _REAL
    if _REAL is None:
        import json, glob
        _REAL = {}
        for folder in _dirs():
            f = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'which_room_' + os.path.basename(os.path.normpath(folder)) + '.json')
            if os.path.exists(f) and not os.environ.get('POP_DIRECT_SHOTS'):
                m = {k: (v[1], v[2]) for k, v in json.load(open(f)).items()}
            else:
                m = {os.path.basename(p): (int(os.path.basename(p).split('_R')[1][:-4]), 1.0) for p in glob.glob(os.path.join(folder, 'L*_R*.png'))}
            for k, (best, sh) in m.items():
                lv = int(k[1:].split('_')[0])
                if (lv, best) not in _REAL or sh > _REAL[(lv, best)][1]: _REAL[(lv, best)] = (k, sh, folder)
    return _REAL

def have(level, room):
    return (level, room) in _real()

def shot(level, room):
    k, sh, folder = _real()[(level, room)]
    return np.array(Image.open(os.path.join(folder, k)).convert('RGB'))

def rgb(sp, palette):
    idx, _ = sp
    return palette[np.minimum(idx, len(palette) - 1)], idx != 0

def matches(img, sp, palette, min_frac=0.97, area=None):
    """Positions (x, y, matching opaque pixels, opaque pixels) where the sprite's opaque pixels equal the screenshot's."""
    col, op = rgb(sp, palette); h, w = op.shape; H, W = img.shape[:2]
    n = op.sum(); out = []
    x0, y0, x1, y1 = area or (-w + 1, -h + 1, W, H)
    ys, xs = np.nonzero(op)
    for y in range(y0, y1):
        for x in range(x0, x1):
            m = 0; tot = 0
            py, px = ys + y, xs + x
            ok = (py >= 0) & (py < H) & (px >= 0) & (px < W)
            if ok.sum() < n * min_frac: continue
            m = (np.abs(img[py[ok], px[ok]].astype(int) - col[ys[ok], xs[ok]].astype(int)) <= TOL).all(axis=1).sum()
            if m >= n * min_frac: out.append((x, y, int(m), int(n)))
    return out
