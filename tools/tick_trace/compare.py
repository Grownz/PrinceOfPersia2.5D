"""Replays a recorded run in the sim and compares the kid's state per tick. usage: compare.py <trace.json> (from record.py)"""
import sys, os, json, subprocess
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
GUARD = True
FIELDS = ['frame', 'x', 'y', 'dir', 'row', 'action', 'room', 'fall_x', 'fall_y', 'alive', 'sword', 'g_frame', 'g_x', 'g_y', 'g_dir', 'g_row', 'g_action', 'g_room', 'g_sword', 'g_alive']

ALIGN = os.environ.get("POP_TRACE_ALIGN", "fixed")   # "fixed": tick j at counter stand + 5 j, planned keys; "ticks": instants() and the observed keys (for runs with slow ticks)

def instants(r):
    """The original's logic ticks as counters: the stand tick, then every state change (a tick), idle ticks every 5 counts while the kid
    stands, and a late tick (a room change takes longer than 5 counts in the emulation: gap 7..14) counts as one tick."""
    st = r['stand']; cs = sorted(int(c) for c in r['states'] if int(c) >= st)
    if ALIGN == 'fixed': return [st + 5 * j for j in range(len(r['keys']) + 1)]
    def key(c):
        k = r['states'][str(c)]; b = bytes.fromhex(k[0]); return (b[0], b[1], b[2], b[6], b[9])
    changes = [c for p, c in zip(cs, cs[1:]) if key(c) != key(p)]
    out = [st]; t = st
    for nxt in changes + [cs[-1] + 5]:
        while nxt - t > 14 or (nxt - t > 6 and bytes.fromhex(r['states'][str(t)][0])[0] in (15, 109)):
            t += 5
            if t >= nxt - 2: break
            out.append(t)
        if nxt > t: out.append(nxt); t = nxt
    return out

def original(r):
    out = []; ins = instants(r)
    for n, t in enumerate(ins):
        nxt = ins[n + 1] if n + 1 < len(ins) else t + 5
        k = r['states'].get(str(nxt - 1)) or r['states'].get(str(t + 4))
        if not k: break
        b = bytes.fromhex(k[0])
        s8 = lambda v: v - 256 if v > 127 else v
        g = bytes.fromhex(k[1]); gl = [0] * 9 if g[3] == 0x56 else [g[0], g[1], g[2], g[3], s8(g[5]), g[6], g[9], g[12], 255 if g[13] > 127 else 0]
        out.append([b[0], b[1], b[2], b[3], s8(b[5]), b[6], b[9], s8(b[7]), s8(b[8]), 255 if b[13] > 127 else 0, b[12]] + gl)
    return out

def observed_keys(r):
    """Keys in effect at each tick instant after the stand tick (from the key table sampled just before the tick), as the sim's key strings."""
    if ALIGN == 'fixed': return r['keys']
    out = []
    for t in instants(r)[1:]:
        k = r['states'].get(str(t - 1)) or r['states'].get(str(t))
        b = bytes.fromhex(k[4]) if k and len(k) > 4 else bytes(5)
        out.append(''.join(ch for ch, v in zip('LRUDS', b) if v))
    return out

def sim(r, tmp):
    kin = os.path.join(tmp, 'trace_keys.txt'); kout = os.path.join(tmp, 'trace_sim.txt')
    open(kin, 'w').write('\n'.join(k or '-' for k in r['keys']) + '\n')
    env = dict(os.environ, POP_TRACE_IN=kin, POP_TRACE_OUT=kout, POP_TRACE_LEVEL=str(r['level']))
    if r.get('levels_dat'): env['POP_TRACE_LEVELS'] = r['levels_dat']      # a LEVELS.DAT with another start room (run_guard.py)
    k0 = r['states'].get(str(r['stand'] + 4))
    if k0 and len(k0) > 2: env['POP_TRACE_SEED'] = str(int.from_bytes(bytes.fromhex(k0[2]), 'little'))   # the original's prandom state after its stand tick
    p = subprocess.run(['cargo', 'test', '-q', '-p', 'pop-core', 'trace_replay', '--', '--ignored', '--nocapture'], cwd=ROOT, env=env, capture_output=True, text=True)
    if not os.path.exists(kout): raise SystemExit(p.stdout + p.stderr)
    lines = open(kout).read().split(chr(10))
    pre = [x for x in lines if x.startswith('pre ')]; lines = [x for x in lines if not x.startswith('pre ')]
    sim.pre = [(int(x.split()[1]), int(x.split()[2])) for x in pre]
    stand = int(lines[0].split()[1])
    rows = [[int(x) for x in l.split()] for l in lines[1:] if l]
    return stand, [row[:11] + row[18:27] + row[11:18] for row in rows]      # fields, guard, then seq seed flags (debug)

def compare(r, tmp, show=8):
    o = original(r); stand, s = sim(r, tmp); bad = 0
    print(f'sim reaches the stand frame after {stand} ticks; original: counter {r["stand"]}')
    first = {}
    for j in range(min(len(o), len(s))):
        if j and (o[j - 1][9] == 0 or s[j - 1][9] == 0):    # the kid died: the original restarts the level on the next key, the sim does not
            print(f'the kid died at tick {j - 1}; compared up to there'); break
        diff = [f'{FIELDS[i]} {o[j][i]} != {s[j][i]}' for i in range(len(FIELDS)) if o[j][i] != s[j][i] and (GUARD or i < 11)]
        if diff:
            bad += 1
            if bad <= show: print(f'tick {j} keys {r["keys"][j - 1] if j else "-"!r}: original vs sim:', ', '.join(diff))
            for d in diff: first.setdefault(d.split()[0], j)
    print(f'{len(o)} ticks compared, {bad} differ; first difference per field: {first}')
    return bad

if __name__ == '__main__':
    r = json.load(open(sys.argv[1])); compare(r, os.environ.get('TEMP', '/tmp'))
