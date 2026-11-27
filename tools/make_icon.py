"""Draws assets/icon.ico: the original hourglass (PV 953, 19x28) on the title-screen night sky and frame.
Usage: python tools/make_icon.py   (needs Pillow). The sprite is hard-coded so no game data is read."""
from PIL import Image
P = dict(n='#100061', g='#e2a100', r='#810000', cr='#e2c271', w='#f2f2f2')
H = ['..abbcccccccccbba..','aacccccccccccccccbd','abaaacccccccccbbbdd','..bbbaaaaaaabbddd..','.a..abedddddddd..a.','.ab.f.........f.ab.','.bdf...........fbd.','.bdghhhhhhhhhhhgbc.','.bdgihhhhhhhhhigbc.','.bd.ghhhhhhhhhg.bc.','.bd.gihhhhhhhig.bc.','.bd..giihhhiig..bc.','.be...giiiiig...be.','bdhh...ggigg...bdhh','.aa.....ghg.....aa.','.ae....ggfgg....ae.','.bd...gff.ffg...bc.','.bd..gf.....fg..bc.','.bd.gf.......fg.bc.','.bc.gf.......fg.bc.','.bcgf.........fgbc.','.bcgf.........fgbc.','abcgf.........fgbcd','aacagf.......fgabed','abeaafffffffffaabhd','.eeeeaaaaaaaaachhd.','..aeeeedddcccccda..','....abeddddddda....']
HC = dict(a='#79593c',b='#a1794c',c='#f6ca7d',d='#e6b271',e='#c29559',f='#4c4038',g='#ffffff',h='#ffeaaa',i='#d2bea1')
im = Image.new('RGB', (32, 32), P['n'])
px = im.load()
for i in range(32):
    for k, c in enumerate(('g', 'r')):
        for a, b in ((i, k), (i, 31 - k), (k, i), (31 - k, i)):
            px[a, b] = Image.new('RGB', (1, 1), P[c]).getpixel((0, 0))
for i, (a, b) in enumerate([(4,4),(27,5),(5,14),(27,16),(4,25),(26,26),(9,3),(23,3)]):
    px[a, b] = Image.new('RGB', (1, 1), P['cr' if i % 2 else 'w']).getpixel((0, 0))
for j, row in enumerate(H):
    for i, ch in enumerate(row):
        if ch != '.':
            px[6 + i, 2 + j] = Image.new('RGB', (1, 1), HC[ch]).getpixel((0, 0))
im.save('assets/icon.png')
big = im.resize((256, 256), Image.NEAREST)  # integer steps only, so the pixels stay sharp
big.save('assets/icon.ico', sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
