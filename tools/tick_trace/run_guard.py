"""usage: run_guard.py <level> <guard index> <seed> [ticks]: the kid starts in the guard's room (patched LEVELS.DAT, same file for the sim),
a few tiles from the guard, facing away from him (so that he stands as in the other traces); a seeded random fight script; compares kid and guard tick by tick."""
import sys, os, json, random, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'room_shots'))
import record, compare
import all_rooms
orig = os.path.join(compare.ROOT, 'Original_data'); dump = os.path.join(os.environ['TEMP'], 'popdump')
level, gi, seed = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]); ticks = int(sys.argv[4]) if len(sys.argv) > 4 else 200
guards = [tuple(int(x) for x in l.split()) for l in open(os.path.join(dump, f'level{level}.guards'))]
room, tile, gdir = guards[gi]
fg = open(os.path.join(dump, f'level{level}.bin'), 'rb').read()[:720]
row, gcol = tile // 10, tile % 10
cand = [c for c in (gcol - 4, gcol + 4, gcol - 3, gcol + 3, gcol - 5, gcol + 5) if 0 <= c < 10 and fg[(room - 1) * 30 + row * 10 + c] & 31 == 1]
if not cand: raise SystemExit('no floor tile for the kid near the guard')
kcol = cand[0]
tmp = tempfile.mkdtemp(prefix='popguard')
f = all_rooms.patched(orig, level, room, row * 10 + kcol, tmp)
rnd = random.Random(seed); vocab = ['R', 'L', 'U', 'S', 'US', 'RS', 'LS', 'D', 'RU', '-', '-']; segs = []; n = 0
while n < ticks:
    k = rnd.choice(vocab); m = rnd.randint(1, 6); segs.append(f'{k}:{m}'); n += m
sc = ','.join(segs); print('level', level, 'guard', (room, tile, gdir), 'kid col', kcol, sc[:90])
os.environ['POP_TRACE_LEVELS'] = f
r = record.record(orig, level, sc, dump, f); r['seed'] = seed; r['levels_dat'] = f
out = os.path.join(os.environ['TEMP'], f'trace_G{level}_{gi}_S{seed}.json'); json.dump(r, open(out, 'w'))
compare.compare(r, os.environ['TEMP'])
