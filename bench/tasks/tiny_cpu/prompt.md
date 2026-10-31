Write an interpreter for a tiny assembly language. stdin holds a program, then a line `---`, then input numbers (whole numbers, possibly negative, separated by spaces or newlines).

Each program line is blank, a comment (from `;` to the end of the line; a comment may also follow an instruction), a label `NAME:` (a word, alone on its line), or one instruction. Operands are separated by `, `. A register is `r0` to `r7`; a value is a register or a whole number (possibly negative). Instructions:

- `MOV r, v` / `ADD r, v` / `SUB r, v` / `MUL r, v`: r = v, r + v, r - v, r * v.
- `DIV r, v` / `MOD r, v`: r divided by v rounded toward zero, and the remainder with the sign of r (so `DIV` then `MUL` then `ADD` of the remainder gives r back).
- `JMP L`, `JZ r, L` (jump if r is 0), `JNZ r, L`, `JLT v, v, L` (jump if the first value is less than the second).
- `CALL L` pushes the return position on the call stack and jumps; `RET` returns to it.
- `PUSH v` and `POP r` use a data stack (separate from the call stack).
- `READ r` sets r to the next input number. `PRINT v` prints the value on its own line.
- `HALT` stops.

All registers start at 0. The program also stops when it runs past its last instruction.

Before running, check the whole program. Each bad line prints `line N: bad instruction` (an unknown instruction, wrong number or kind of operands, or a malformed label); a jump or call to a label that does not exist prints `line N: unknown label L`; a label defined twice prints `line N: duplicate label L` at its second definition. N is the line number in stdin. If any of these occur, print them in line order and do not run.

While running, these stop the program with an error line: `error line N: division by zero`, `error line N: stack empty` (POP or RET with nothing on its stack), `error line N: no input` (READ with no numbers left), and `error: too many steps` after 10000 instructions have run without stopping.

Finally (also after a run-time error) print `steps S` (the number of instructions that ran, counting the one that failed) and then `regs` followed by ` rI=V` for each register that is not 0, in order, or `regs none`.

Write the program as 3 files in the folder you are given: `main`, `parse`, `machine`. `main` is the entry point; each file imports what it needs from the others. `parse` reads and checks the program (instructions, operands, labels) and reports the check errors; `machine` holds the registers and stacks and runs the checked program; `main` splits stdin, calls them and prints the result.
