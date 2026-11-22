"""Reading the original's data segment from a running DOSBox 0.74 (see tools/level_ram): the level array (ds:50C2) sits at a fixed
offset of the 16 MB RAM block, so the data segment is at block + OFF - 0x50C2."""
import ctypes, os, sys, time
from ctypes import wintypes as w
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'level_ram'))
import level_ram as L
SIZE, OFF = 0x1001000, 0x21b22          # RAM block size and the level array's offset in it (calibrated by level_ram.py)

def find_base(h):
    """Base of the RAM block whose level array looks like a level (any block of that size with the 2305 bytes starting at OFF non-zero)."""
    cands = [b for b, s in L.regions(h, SIZE) if s == SIZE]
    return cands

def ds_base(h, level_fg_prefix):
    for b in find_base(h):
        d = L.read(h, b + OFF, 90)
        if d and d == level_fg_prefix: return b + OFF - 0x50C2
    return None

def rd(h, ds, off, n):
    return L.read(h, ds + off, n)
