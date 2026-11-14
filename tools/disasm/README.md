# pop-disasm

A small 16-bit disassembler for `Original_data/PRINCE.EXE` (DOS v1.1), built on `iced-x86`.
It is a standalone Cargo project (it has its own empty `[workspace]`), so it is not part of the main workspace.

```
cd tools/disasm
cargo build --release
./target/release/pop-disasm <command>
cargo test --release      # self-check, needs Original_data/PRINCE.EXE (the test does nothing if the file is missing)
```

The EXE path defaults to `<repo>/Original_data/PRINCE.EXE`. Set `POP_EXE` to use another file.

## Addresses

- `SEG:OFF` is hex and relative to the load segment (the segment values the MZ relocations use).
  File offset = 0x890 (the MZ header) + SEG*16 + OFF.
- Every output line shows `SEG:OFF [file offset] bytes  instruction`.
- Data segment DS = `1B03`, so file offset = ds offset + 112832.
- Code segments are taken from the relocation fixups. Game logic is in segment `0000` (Pascal calling convention:
  `ret far N`, and same-segment calls are `push cs; call near`). Runtime, graphics and sound are in `0C83` and the small segments after it.

## Commands

| command | what it does |
|---|---|
| `info` | header size, DS, list of code segments |
| `dis SEG:OFF [N]` | disassemble N instructions. Without N it stops at a `ret`/`retf` that is followed by `push bp` (the next function) |
| `farcalls` | every `call far`/`jmp far` target, with how many times it is called |
| `callers SEG:OFF` | far calls to SEG:OFF, plus near calls to OFF from inside SEG (this catches `push cs; call`) |
| `xref VALUE [mem\|imm]` | instructions that use VALUE as a memory displacement (`[1234h]`, `[bx+1234h]`) and/or as an immediate |
| `ds OFF LEN` | hex dump of the data segment |

VALUE, OFF and LEN can be decimal or `0x..` hex.

## Limits

- `farcalls`, `callers` and `xref` do a linear sweep from each segment base to the next one. Data embedded in code
  (for example jump tables such as `0000:70F6`) decodes as junk instructions, and the sweep can lose sync for a few
  instructions after it. `dis` from a known function start is always exact.
- There are no symbols. The names in `docs/mechanics.md` are ours, chosen to match SDLPoP where the code matches.
