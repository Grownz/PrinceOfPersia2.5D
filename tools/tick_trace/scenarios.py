"""Seeded random key scripts: segments of 1..8 ticks of a move from a vocabulary, plus short pauses (script format of record.py)."""
import random
VOCAB = ['R', 'L', 'U', 'D', 'RU', 'LU', 'RS', 'LS', 'RD', 'LD', 'US', 'DS', 'RUS', 'LUS', '-', '-']

def script(seed, ticks=150):
    rnd = random.Random(seed); out = []; n = 0
    while n < ticks:
        k = rnd.choice(VOCAB); d = rnd.randint(1, 8); out.append(f'{k}:{d}'); n += d
    return ','.join(out)
