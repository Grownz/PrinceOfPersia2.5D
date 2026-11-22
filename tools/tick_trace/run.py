"""usage: run.py <level> <seed> [ticks]: records the original with scenarios.script(seed), replays it in the sim and compares."""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record, compare, scenarios
orig = os.path.join(compare.ROOT, 'Original_data'); dump = os.path.join(os.environ['TEMP'], 'popdump')
level, seed = int(sys.argv[1]), int(sys.argv[2]); ticks = int(sys.argv[3]) if len(sys.argv) > 3 else 150
sc = scenarios.script(seed, ticks); print('level', level, 'seed', seed, sc[:100])
r = record.record(orig, level, sc, dump); r['seed'] = seed
f = os.path.join(os.environ['TEMP'], f'trace_L{level}_S{seed}.json'); json.dump(r, open(f, 'w'))
compare.compare(r, os.environ['TEMP'])
