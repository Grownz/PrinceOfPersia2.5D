"""Screenshots of every non-empty room of levels 1..14 from the original in DOSBox: the start room in a copy of LEVELS.DAT is
set to the room (start tile = a plain floor tile, so the kid stays in it). Output: out/L<level>_R<room>.png, 320x200.
usage: python all_rooms.py <Original_data> <dump dir of the level_dump example> [levels...]"""
import os, shutil, struct, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dosbox_shot import shot
OUT = os.environ.get('POP_OUT_DIR') or os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
ONLY = {tuple(int(v) for v in x.split(':')) for x in os.environ.get('POP_ONLY', '').split(',') if x}   # 'level:room,...': a second pass for these rooms
FALLBACK = bool(ONLY)                                                          # accept other floor-like tiles when no plain floor is inside

def patched(orig, level, room, tile, tmp):
    d = bytearray(open(os.path.join(orig, 'LEVELS.DAT'), 'rb').read())
    io = struct.unpack_from('<I', d, 0)[0]; n = struct.unpack_from('<H', d, io)[0]
    for i in range(n):
        id_, off, size = struct.unpack_from('<HIH', d, io + 2 + i * 8)
        if id_ == 2000 + level:
            p = off + 1                                   # payload; byte `off` is its checksum
            d[p + 2112], d[p + 2113], d[p + 2114] = room, tile, 0
            d[off] = (sum(d[p:p + size]) & 255) ^ 255
            f = os.path.join(tmp, 'LEVELS.DAT'); open(f, 'wb').write(d); return f
    raise SystemExit('level not found')

def start_tile(fg, room):
    """A floor tile away from the room's left and right edges (a kid at the edge leaves the room): bottom row first, columns
    in the order 4, 5, 3, 6, 2, 7, 1, 8, 0, 9."""
    t = fg[(room - 1) * 30:room * 30]
    if FALLBACK:      # second pass: the middle columns first, plain floor before the other floor-like tiles
        for kinds in ((1,), (1, 3, 6, 14, 15, 19, 21, 22, 30)):
            for cols in ((4, 5, 3, 6, 2, 7), (1, 8, 0, 9)):
                for row in (2, 1, 0):
                    for col in cols:
                        if t[row * 10 + col] & 31 in kinds: return row * 10 + col
        return None
    for row in (2, 1, 0):
        for col in (4, 5, 3, 6, 2, 7, 1, 8, 0, 9):
            if t[row * 10 + col] & 31 == 1:    # a loose floor would fall under the kid
                return row * 10 + col
    if False:      # no plain floor in the middle: any tile the kid can stand on (pillar, buttons, debris, torch base, skeleton), also at the edges
        for row in (2, 1, 0):
            for col in (4, 5, 3, 6, 2, 7, 1, 8, 0, 9):
                if t[row * 10 + col] & 31 in (1, 3, 6, 14, 15, 19, 21, 22, 30): return row * 10 + col
    return None

if __name__ == '__main__':
    orig, dump = sys.argv[1], sys.argv[2]
    levels = [int(x) for x in sys.argv[3:]] or range(1, 15)
    os.makedirs(OUT, exist_ok=True); tmp = tempfile.mkdtemp(prefix='poplv')
    for lv in levels:
        fg = open(os.path.join(dump, f'level{lv}.bin'), 'rb').read()[:720]
        for room in range(1, 25):
            if all(b & 31 == 0 for b in fg[(room - 1) * 30:room * 30]) or (ONLY and (lv, room) not in ONLY): continue
            out = os.path.join(OUT, f'L{lv}_R{room}.png')
            if os.path.exists(out): continue
            t = start_tile(fg, room)
            if t is None: print(f'level {lv} room {room}: no floor tile to start on'); continue
            for _ in range(3):      # a window that was not drawn (screen off, locked) comes out as one flat colour
                try: im = shot(lv, orig, 3.5, patched(orig, lv, room, t, tmp))
                except Exception as e: print('retry:', e); continue
                if len(set(im.resize((32, 20)).getdata())) > 4: break
            else:
                print(f'level {lv} room {room}: only blank pictures'); continue
            im.resize((320, 200), 0).save(out); print(lv, room, flush=True)
