Run Conway's Game of Life on a grid that wraps around at the edges (the left edge touches the right, the top touches the bottom). The first line of stdin is `W H G`: the width, height and number of generations. The next H lines have W characters each: `#` for a live cell, `.` for a dead one.

Each generation, every cell counts its live neighbours among the 8 around it (with wrapping). A live cell with 2 or 3 live neighbours stays alive; a dead cell with exactly 3 becomes alive; every other cell is dead in the next generation.

Run up to G generations. The starting grid is generation 0. If generation T is the same as an earlier generation S, stop there and print `generation T repeats generation S` (use the earliest such S). Then print the grid you stopped at (generation G if nothing repeated) in the same format as the input, then `population P`, the number of live cells in it.
