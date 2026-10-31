Run a program for a small stack machine read from stdin, one instruction per line. `#` starts a comment to the end of the line; blank lines are ignored. A line `name:` defines a label at the next instruction. Integers are 64-bit.

Instructions: `push N` (N may be negative), `pop`, `dup`, `swap` (swap the top two), `add`, `sub`, `mul`, `div`, `mod` (for these the top value is the right operand: `push 7`, `push 2`, `sub` gives 5; `div` and `mod` round toward zero), `jmp L`, `jz L` (pop a value and jump if it is 0), `jnz L` (pop and jump if not 0), `call L` (push the return position on a separate call stack and jump), `ret` (return; with an empty call stack, stop), `print` (pop and print the value), `halt`.

Before running, check the program: the first line with an unknown instruction gives `error line N: unknown instruction X`; otherwise the first jump or call to an undefined label gives `error line N: unknown label L`. N is the line number in the input (1-based). In that case run nothing.

While running, stop with `error line N: stack underflow` when an instruction needs more values than the stack has, `error line N: division by zero` for `div` or `mod` by 0, or `error: step limit` once 10000 instructions have run without stopping. Output printed before an error stays. The program stops normally after its last instruction or at `halt`.
