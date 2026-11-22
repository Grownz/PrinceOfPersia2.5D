import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from calib import *
levels = [1, 2, 3, 7, 8, 9, 12, 13]
out = {}
for kind, ids, mf in ((11, list(range(232, 270)) + [292, 293, 297, 298], 0.9), (20, list(range(360, 378)) + [244, 245, 250, 251], 0.8)):
    seen, hist = calibrate(kind, ids, levels, win=(-40, 40, -75, 20), min_frac=mf, limit=30)
    out[kind] = {str(i): hist[i].most_common(3) for i in ids if hist[i]}
    print(kind, seen, {i: hist[i].most_common(2) for i in ids if hist[i]}, flush=True)
json.dump(out, open(os.path.join(os.environ['TEMP'], 'learn2.json'), 'w'))
