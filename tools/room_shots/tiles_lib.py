"""Tile images (exported by the tile_export example) and a template matcher for the DOSBox room screenshots."""
import os, numpy as np
from PIL import Image
TILES = os.environ.get('POP_TILES_DIR') or os.path.join(os.environ.get('TEMP', '/tmp'), 'tiles')
TOL = 9   # the game's palette differs from the resource by one 6-bit step here and there
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')

def pal(set_, id_):
    # DOSBox shows a 6-bit VGA value v as 4 * v (the export scaled it to 0..255)
    a = np.array([[int(v) for v in l.split()] for l in open(os.path.join(TILES, f'{set_}_pal{id_}.txt'))])
    return (np.rint(a * 63 / 255) * 4).astype(np.uint8)

def sprite(set_, id_):
    """(index array h x w, bpp) or None."""
    f = os.path.join(TILES, f'{set_}_{id_}.bin')
    if not os.path.exists(f): return None
    d = open(f, 'rb').read(); w, h, bpp = d[0] | d[1] << 8, d[2] | d[3] << 8, d[4]
    return np.frombuffer(d[5:], dtype=np.uint8).reshape(h, w), bpp

def shot(level, room):
    return np.array(Image.open(os.path.join(SHOTS, f'L{level}_R{room}.png')).convert('RGB'))

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
