// Events of the progress count, to count by hand. Each of ROUNDS rounds: a call (1) and its return (1), a
// forward conditional jump (none), a backward conditional jump never taken (1) and one taken but in the
// last round (1). Then a jump through a register to a later address (1) and a forward jump (none). So the
// count at the exit is 4 * ROUNDS + 1.
.text
.globl _start
_start:
  mov x19, #ROUNDS
1:
  bl leaf
  cbz x19, 2f
  tbnz x19, #63, 1b
  sub x19, x19, #1
  cbnz x19, 1b
2:
  adr x9, 3f
  br x9
3:
  b 4f
4:
  mov x0, #0
  mov x8, #93
  svc #0

leaf:
  ret
