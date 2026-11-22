"""usage: show.py <trace.json> <from> <to>: original and sim kid state side by side"""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compare
def oflags(r, j):
    k = r['states'].get(str(r['stand'] + 5 * j + 4))
    return [v - 256 if v > 127 else v for v in bytes.fromhex(k[3])] if k and len(k) > 3 else []
def oseed(r, j):
    k = r['states'].get(str(r['stand'] + 5 * j + 4))
    return int.from_bytes(bytes.fromhex(k[2]), 'little') if k and len(k) > 2 else 0
r = json.load(open(sys.argv[1])); a, b = int(sys.argv[2]), int(sys.argv[3])
o = compare.original(r); stand, s = compare.sim(r, os.environ['TEMP'])
print('fields', compare.FIELDS)
for j in range(a, min(b, len(o), len(s))):
    mark = '  ' if o[j] == s[j] else '!!'
    print(mark, j, repr(r['keys'][j - 1] if j else '-'), 'orig', o[j][:9], 'sim', s[j][:9], 'guard', o[j][11:], '/', s[j][11:20], 'seq', s[j][20] if len(s[j]) > 20 else '', 'flags orig', oflags(r, j), 'sim', s[j][22:27] if len(s[j]) > 26 else '')
