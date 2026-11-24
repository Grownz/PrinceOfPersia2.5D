"""Compares pixel-art scaling filters on the game's own tile images (docs/tile_filters.md).

python tools/tile_filters/compare.py <tiles dir from tile_export> <out dir> [path to filt_rs.exe]

Filters (on palette colours, so the palette stays exact): nearest, EPX / Scale2x, MMPX. MMPX is a port of the MIT-licensed `mmpx` crate
(0.2.0, Morgan McGuire / Mara Gagiu's algorithm), checked against it below. xBRZ comes from `filt_rs` (a scratch build around the
GPL-3.0 crate xbrz-rs, never part of this repository): without it the xBRZ columns are left out.
Writes contact sheets (sheet_*.png) and metrics.txt into <out dir>.
"""
import os, struct, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw

SETS = {  # (set, tile id, palette id): the wall and floor faces, the speckled stone, the torch wall, the palace walls
    'dungeon wall 368': ('VDUNGEON', 368, 360), 'dungeon wall 366': ('VDUNGEON', 366, 360), 'dungeon top 373': ('VDUNGEON', 373, 360),
    'dungeon 1301': ('VDUNGEON', 1301, 200), 'dungeon floor 1303': ('VDUNGEON', 1303, 200), 'dungeon 1286': ('VDUNGEON', 1286, 200),
    'dungeon 324': ('VDUNGEON', 324, 200), 'dungeon 344': ('VDUNGEON', 344, 200),
    'palace 1201': ('VPALACE', 1201, 200), 'palace 1210': ('VPALACE', 1210, 200), 'palace 1275': ('VPALACE', 1275, 200),
    'palace 1301': ('VPALACE', 1301, 200), 'palace 1303': ('VPALACE', 1303, 200), 'palace 1305': ('VPALACE', 1305, 200),
}


def load(d, s, i, pal):
    b = open(f'{d}/{s}_{i}.bin', 'rb').read()
    w, h, _ = struct.unpack('<HHB', b[:5])
    idx = np.frombuffer(b[5:5 + w * h], np.uint8).reshape(h, w)
    p = np.array([[int(v) for v in l.split()] for l in open(f'{d}/{s}_pal{pal}.txt')], np.uint8)
    return idx, p


def labels(idx, pal):
    """Colour labels (equal RGB = equal label) and the luma (the image crate's to_luma) of each label."""
    rgb = pal[np.minimum(idx, len(pal) - 1)].astype(np.int64)
    key = rgb[..., 0] << 16 | rgb[..., 1] << 8 | rgb[..., 2]
    u, inv = np.unique(key, return_inverse=True)
    r, g, b = (u >> 16) & 255, (u >> 8) & 255, u & 255
    return inv.reshape(idx.shape), (2126 * r + 7152 * g + 722 * b) // 10000, u


def pad(a, n, wrap):
    return np.pad(a, n, mode='wrap' if wrap else 'edge')


def epx(a, wrap=False):
    h, w = a.shape
    P = pad(a, 1, wrap)
    out = np.empty((h * 2, w * 2), a.dtype)
    for y in range(h):
        for x in range(w):
            p, A, B, C, D = P[y + 1, x + 1], P[y, x + 1], P[y + 1, x + 2], P[y + 1, x], P[y + 2, x + 1]
            out[2 * y, 2 * x] = A if C == A and C != D and A != B else p
            out[2 * y, 2 * x + 1] = B if A == B and A != C and B != D else p
            out[2 * y + 1, 2 * x] = C if D == C and D != B and C != A else p
            out[2 * y + 1, 2 * x + 1] = D if B == D and B != A and D != C else p
    return out


def mmpx(a, luma, wrap=False):
    h, w = a.shape
    P = pad(a, 3, wrap)
    out = np.empty((h * 2, w * 2), a.dtype)
    for y in range(h):
        for x in range(w):
            g = lambda dx, dy: P[y + 3 + dy, x + 3 + dx]
            A, B, C, D, E, F, G, H, I = g(-1, -1), g(0, -1), g(1, -1), g(-1, 0), g(0, 0), g(1, 0), g(-1, 1), g(0, 1), g(1, 1)
            Pp, Q, R, S = g(0, -2), g(-2, 0), g(2, 0), g(0, 2)
            Bl, Dl, El, Fl, Hl = luma[B], luma[D], luma[E], luma[F], luma[H]
            J = K = L = M = E
            if D == B and D != H and D != F and (El >= Dl or E == A) and (E == A or E == C or E == G) and (El < Dl or A != D or E != Pp or E != Q): J = D
            if B == F and B != D and B != H and (El >= Bl or E == C) and (E == A or E == C or E == I) and (El < Bl or C != B or E != Pp or E != R): K = B
            if H == D and H != F and H != B and (El >= Hl or E == G) and (E == A or E == G or E == I) and (El < Hl or G != H or E != S or E != Q): L = H
            if F == H and F != B and F != D and (El >= Fl or E == I) and (E == C or E == G or E == I) and (El < Fl or I != H or E != R or E != S): M = F
            if E != F and E == C and E == I and E == D and E == Q and F == B and F == H and F != g(3, 0): K = M = F
            if E != D and E == A and E == G and E == F and E == R and D == B and D == H and D != g(-3, 0): J = L = D
            if E != H and E == G and E == I and E == B and E == Pp and H == D and H == F and H != g(0, 3): L = M = H
            if E != B and E == A and E == C and E == H and E == S and B == D and B == F and B != g(0, -3): J = K = B
            if Bl < El and E == G and E == H and E == I and E == S and E not in (A, D, C, F): J = K = B
            if Hl < El and E == A and E == B and E == C and E == Pp and E not in (D, G, I, F): L = M = H
            if Fl < El and E == A and E == D and E == G and E == Q and E not in (B, C, I, H): K = M = F
            if Dl < El and E == C and E == F and E == I and E == R and E not in (B, A, G, H): J = L = D
            if H != B:
                if H != A and H != E and H != C:
                    if H == G and H == F and H == R and H != D and H != g(2, -1): L = M
                    if H == I and H == D and H == Q and H != F and H != g(-2, -1): M = L
                if B != I and B != G and B != E:
                    if B == A and B == F and B == R and B != D and B != g(2, 1): J = K
                    if B == C and B == D and B == Q and B != F and B != g(-2, 1): K = J
            if F != D:
                if D != I and D != E and D != C:
                    if D == A and D == H and D == S and D != B and D != g(1, 2): J = L
                    if D == G and D == B and D == Pp and D != H and D != g(1, -2): L = J
                if F != E and F != A and F != G:
                    if F == C and F == H and F == S and F != B and F != g(-1, 2): K = M
                    if F == I and F == B and F == Pp and F != H and F != g(-1, -2): M = K
            out[2 * y, 2 * x], out[2 * y, 2 * x + 1], out[2 * y + 1, 2 * x], out[2 * y + 1, 2 * x + 1] = J, K, L, M
    return out


def nn(a, f):
    return np.repeat(np.repeat(a, f, 0), f, 1)


def to_rgb(lab, u):
    k = u[lab]
    return np.stack([(k >> 16) & 255, (k >> 8) & 255, k & 255], -1).astype(np.uint8)


def write_rgba(path, rgb):
    h, w, _ = rgb.shape
    open(path, 'wb').write(struct.pack('<HH', w, h) + np.dstack([rgb, np.full((h, w, 1), 255, np.uint8)]).tobytes())


def read_rgba(path):
    b = open(path, 'rb').read()
    w, h = struct.unpack('<HH', b[:4])
    return np.frombuffer(b[4:], np.uint8).reshape(h, w, 4)[..., :3]


def isolated(a):
    P = np.pad(a, 1, mode='edge')
    return (a != P[:-2, 1:-1]) & (a != P[2:, 1:-1]) & (a != P[1:-1, :-2]) & (a != P[1:-1, 2:])


def metrics(src_rgb, iso, out_rgb):
    """Share of isolated source pixels that stay (>= 3 of the 2x2 sub-pixels), share of sub-pixels that differ from a plain 2x copy,
    number of colours the source did not have."""
    ref = nn(src_rgb, 2)
    same = (out_rgb == ref).all(-1)
    blk = same.reshape(src_rgb.shape[0], 2, src_rgb.shape[1], 2).sum((1, 3))
    kept = (blk[iso] >= 3).mean() if iso.any() else float('nan')
    cols = {tuple(c) for c in src_rgb.reshape(-1, 3)}
    new = len({tuple(c) for c in out_rgb.reshape(-1, 3)} - cols)
    return kept, 1 - same.mean(), new


def main():
    d, o = sys.argv[1], sys.argv[2]
    exe = sys.argv[3] if len(sys.argv) > 3 else None
    os.makedirs(o, exist_ok=True)
    tiles = {}
    for name, (s, i, p) in SETS.items():
        idx, pal = load(d, s, i, p)
        lab, luma, u = labels(idx, pal)
        tiles[name] = (lab, luma, u)
        key = name.replace(' ', '_')
        write_rgba(f'{o}/{key}.rgba', to_rgb(lab, u))
        write_rgba(f'{o}/{key}_t3.rgba', to_rgb(np.tile(lab, (3, 3)), u))
    t = ''
    if exe:
        t = subprocess.run([exe, o], capture_output=True, text=True, check=True).stdout.strip()
    lines = ['tile | iso px | EPX kept/dev/new | MMPX kept/dev/new | xBRZ kept/dev/new | seam(clamp vs 3x3) EPX MMPX xBRZ', '-' * 100]
    agg = {k: [] for k in ('epx', 'mmpx', 'xbrz')}
    crate_ok = []
    for name, (lab, luma, u) in tiles.items():
        key = name.replace(' ', '_')
        rgb, iso = to_rgb(lab, u), isolated(lab)
        e2, m2 = epx(lab), mmpx(lab, luma)
        row = {'epx': to_rgb(e2, u), 'mmpx': to_rgb(m2, u)}
        if exe:
            row['xbrz'] = read_rgba(f'{o}/{key}.xbrz2.rgba')
            crate_ok.append(int((read_rgba(f'{o}/{key}.mmpx2.rgba') != row['mmpx']).any(-1).sum()))
        cells = []
        for k in ('epx', 'mmpx', 'xbrz'):
            if k in row:
                kept, dev, new = metrics(rgb, iso, row[k])
                agg[k].append((kept, dev, new))
                cells.append(f'{kept:4.0%}/{dev:4.0%}/{new:3d}')
            else:
                cells.append('   -   ')
        # seams: the tile filtered alone (edges clamped) against the same tile inside a 3x3 tiling (its centre), outer 2 px band
        h, w = lab.shape
        t3 = np.tile(lab, (3, 3))
        band = np.zeros((h * 2, w * 2), bool); band[:2] = band[-2:] = True; band[:, :2] = band[:, -2:] = True
        sm = []
        for k, f in (('epx', lambda a: epx(a)), ('mmpx', lambda a: mmpx(a, luma)), ('xbrz', None)):
            if k == 'xbrz':
                if not exe: sm.append('  -'); continue
                a, c = row['xbrz'], read_rgba(f'{o}/{key}_t3.xbrz2.rgba')[h * 2:h * 4, w * 2:w * 4]
                diff = (a != c).any(-1)
            else:
                a = to_rgb(f(lab), u); c = to_rgb(f(t3)[h * 2:h * 4, w * 2:w * 4], u)
                diff = (a != c).any(-1)
                if k == 'mmpx':  # wrap mode must equal the tiled result exactly
                    assert (to_rgb(mmpx(lab, luma, True), u) == c).all(), 'wrap mode differs from the tiled result'
            sm.append(f'{diff[band].mean():3.0%}')
        lines.append(f'{name:20s} | {iso.sum():4d} | ' + ' | '.join(cells) + ' | ' + ' '.join(sm))
    lines.append('')
    for k, v in agg.items():
        if v:
            a = np.nanmean(np.array(v, float), 0)
            lines.append(f'mean {k:5s}: isolated kept {a[0]:.0%}, sub-pixels changed {a[1]:.0%}, new colours {a[2]:.1f} per tile')
    if exe:
        # the crate (0.2.0) reads the neighbours P and S (two rows up / down) only at the start of each row, not per pixel: with that
        # change the port equals it on every tile here; the port follows the algorithm
        lines.append(f'MMPX port vs the mmpx crate: {sum(c > 0 for c in crate_ok)} of {len(crate_ok)} tiles differ, output px per tile {crate_ok}')
        lines.append(t)
    open(f'{o}/metrics.txt', 'w').write('\n'.join(lines) + '\n')
    print('\n'.join(lines))
    sheets(tiles, o, exe)


def sheets(tiles, o, exe):
    names = list(tiles)
    for n0 in range(0, len(names), 5):
        grp = names[n0:n0 + 5]
        cols = ['source 1x/nearest 4x', 'EPX 4x', 'MMPX 4x'] + (['xBRZ 4x'] if exe else [])
        S = 128
        img = Image.new('RGB', (S * len(cols) + 8 * len(cols), (S + 14) * len(grp)), (40, 40, 40))
        dr = ImageDraw.Draw(img)
        for r, name in enumerate(grp):
            lab, luma, u = tiles[name]
            # x4 of EPX / MMPX = the filter twice; each panel shows the top-left 128 x 128 output px
            outs = [to_rgb(nn(lab, 4), u), to_rgb(epx(epx(lab)), u), to_rgb(mmpx(mmpx(lab, luma), luma), u)]
            if exe:
                outs.append(read_rgba(f'{o}/{name.replace(" ", "_")}.xbrz4.rgba'))
            for c, a in enumerate(outs):
                img.paste(Image.fromarray(a[:S, :S]), (c * (S + 8), r * (S + 14) + 14))
            dr.text((2, r * (S + 14)), f'{name}: ' + ' | '.join(cols), fill=(230, 230, 230))
        img.save(f'{o}/sheet_{n0 // 5}.png')
    # as the GPU shows it: a fractional 2.5x scale of four tiles (plain nearest, sharp bilinear = nearest 4x then bilinear, MMPX and xBRZ 4x then bilinear)
    pick = [n for n in names if n in ('dungeon 1301', 'dungeon wall 368', 'palace 1210', 'palace 1301')]
    S = 160
    img = Image.new('RGB', (S * 4 + 24, (S + 14) * len(pick)), (40, 40, 40))
    dr = ImageDraw.Draw(img)
    for r, name in enumerate(pick):
        lab, luma, u = tiles[name]
        h, w = lab.shape
        src = Image.fromarray(to_rgb(lab, u))
        sz = (int(w * 2.5), int(h * 2.5))
        m4 = Image.fromarray(to_rgb(mmpx(mmpx(lab, luma), luma), u))
        panels = [src.resize(sz, Image.NEAREST), src.resize((w * 4, h * 4), Image.NEAREST).resize(sz, Image.BILINEAR), m4.resize(sz, Image.BILINEAR)]
        if exe:
            panels.append(Image.fromarray(read_rgba(f'{o}/{name.replace(" ", "_")}.xbrz4.rgba')).resize(sz, Image.BILINEAR))
        for c, p in enumerate(panels):
            img.paste(p.crop((0, 0, S, S)), (c * (S + 8), r * (S + 14) + 14))
        dr.text((2, r * (S + 14)), f'{name}: nearest | sharp bilinear | MMPX+bilinear | xBRZ+bilinear (2.5x)', fill=(230, 230, 230))
    img.save(f'{o}/sheet_fractional.png')


if __name__ == '__main__':
    main()
