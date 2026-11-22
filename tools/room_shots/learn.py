"""Learns, from the DOSBox room screenshots, where each image of a tile type's record is drawn (offset of its bottom-left corner
from the cell's floor line) and how often it is found there. usage: python learn.py <Original_data> <dungeon|palace> <out.json>"""
import sys, os, json, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from calib import *
EXTRA = {  # ids the docs name for a kind (texture_inventory.md 8.1, 12.1), besides its record
    1: [244, 245, 250], 2: [1327, 1328, 1329, 1330, 1331, 1332, 1333, 1334, 1335, 1336], 4: [251, 252, 260, 261, 262, 263, 264, 265, 266, 267, 268],
    16: [233, 234, 299, 344, 237, 238, 239], 18: [1301, 1302, 1303, 1304, 1305, 1306, 1307, 1308, 1309, 1310, 1311, 1312, 1313],
    19: [346, 250], 20: [362, 363, 364, 365, 366, 367, 368, 369, 370, 373, 374, 375, 376, 377], 0: [324, 325, 326, 244, 245, 362, 268, 284],
    30: [346], 12: [268], 9: [268],
}

def records(exe):
    d = open(exe, 'rb').read(); base = 112832 + 0x25B0
    return [list(d[base + 12 * t: base + 12 * t + 12]) for t in range(32)]

if __name__ == '__main__':
    orig, which, out = sys.argv[1:4]
    levels = [4, 5, 6, 10, 11, 14] if which == 'palace' else [1, 2, 3, 7, 8, 9, 12, 13]
    recs = records(os.path.join(orig, 'PRINCE.EXE')); res = {}
    for kind in range(32):
        r = recs[kind]; ids = set()
        for v in (r[0], r[3], r[4], r[7], r[8], r[9]):
            if v: ids |= {200 + v, 1200 + v}
        ids |= set(EXTRA.get(kind, []))
        seen, hist = calibrate(kind, sorted(ids), levels, win=(-40, 40, -75, 20), limit=30)
        res[kind] = {'cells': seen, 'record': r, 'ids': {str(i): hist[i].most_common(3) for i in sorted(ids) if hist[i]}}
        print(kind, seen, {i: hist[i].most_common(2) for i in sorted(ids) if hist[i]}, flush=True)
        json.dump(res, open(out, 'w'))
