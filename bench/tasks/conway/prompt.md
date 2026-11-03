Simulate Conway's Game of Life on a grid that wraps around at the edges (a torus). Read one line from stdin: `W H G SEED D` (1 <= W, H <= 600, 0 <= G <= 500, 0 <= D <= 100).

Random numbers come from this generator: start with `x = SEED`; each next number is `x = (x * 1103515245 + 12345) % 2147483648`, and the number used is the new `x`.

The grid has H rows of W cells. Fill it row by row, left to right: a cell starts alive when the next random number `% 100` is less than D.

Each generation, every cell counts its 8 neighbours (wrapping around the edges); a live cell with 2 or 3 live neighbours stays alive, a dead cell with exactly 3 becomes alive, and every other cell is dead in the next generation. All cells change at once.

Print `gen K: N alive` for K = 0 (the start), for every K that is a multiple of 50, and for K = G (once, even if G is a multiple of 50). Then print `checksum S`, where S is the sum of `row * W + column` (both from 0) over the live cells after generation G.
