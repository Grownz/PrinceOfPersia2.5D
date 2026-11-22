"""Screenshots of every non-empty room of levels 1..14 from the original in DOSBox: the start room in a copy of LEVELS.DAT is
set to the room (start tile = a floor tile, so the kid stays in it). Output: out/L<level>_R<room>.png, 320x200.
usage: python all_rooms.py <Original_data> <dump dir of the level_dump example> [levels...]"""
import os, shutil, struct, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dosbox_shot import shot
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')

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
    t = fg[(room - 1) * 30:room * 30]
    for idx in list(range(20, 30)) + list(range(10, 20)) + list(range(0, 10)):    # bottom row first
        if t[idx] & 31 in (1, 11):
            return idx
    return None

if __name__ == '__main__':
    orig, dump = sys.argv[1], sys.argv[2]
    levels = [int(x) for x in sys.argv[3:]] or range(1, 15)
    os.makedirs(OUT, exist_ok=True); tmp = tempfile.mkdtemp(prefix='poplv')
    for lv in levels:
        fg = open(os.path.join(dump, f'level{lv}.bin'), 'rb').read()[:720]
        for room in range(1, 25):
            if all(b & 31 == 0 for b in fg[(room - 1) * 30:room * 30]): continue
            out = os.path.join(OUT, f'L{lv}_R{room}.png')
            if os.path.exists(out): continue
            t = start_tile(fg, room)
            if t is None: print(f'level {lv} room {room}: no floor tile to start on'); continue
            im = shot(lv, orig, 3.5, patched(orig, lv, room, t, tmp))
            im.resize((320, 200), 0).save(out); print(lv, room, flush=True)
