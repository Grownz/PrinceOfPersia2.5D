"""usage: batch.py <levels> <seeds> [ticks]   e.g. batch.py 1,2,3 1,2,3 150 -- writes one summary line per run to stdout."""
import sys, os, json, io, contextlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record, compare, scenarios
orig = os.path.join(compare.ROOT, 'Original_data'); dump = os.path.join(os.environ['TEMP'], 'popdump')
levels = [int(x) for x in sys.argv[1].split(',')]; seeds = [int(x) for x in sys.argv[2].split(',')]; ticks = int(sys.argv[3]) if len(sys.argv) > 3 else 150
for lv in levels:
    for seed in seeds:
        sc = scenarios.script(seed, ticks)
        try:
            r = record.record(orig, lv, sc, dump); r['seed'] = seed
            json.dump(r, open(os.path.join(os.environ['TEMP'], f'trace_L{lv}_S{seed}.json'), 'w'))
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf): bad = compare.compare(r, os.environ['TEMP'], show=3)
            print(f'L{lv} S{seed}: {bad} differing ticks;', buf.getvalue().strip().replace('\n', ' | '), flush=True)
        except BaseException as e:
            print(f'L{lv} S{seed}: error {e!r}'[:300], flush=True)
