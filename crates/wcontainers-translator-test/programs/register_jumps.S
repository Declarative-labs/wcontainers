// Jumps through a register, ROUNDS of each: a tail call to another function whose address is read from data,
// a jump into its own function to an address no table explains (its start plus an offset worked out from
// data), a switch whose table's address the function keeps in its frame, three tail calls of a function
// to its own start, and a jump into another part of a function cut in parts. Exits with the low bits of what
// the calls gave back.
.text
.globl _start
_start:
  ldr x19, =ROUNDS
  mov x20, #0
1:
  mov x0, x20
  bl apply
  mov x20, x0
  mov x1, #1
  mov x2, #0
  bl wander
  add x20, x20, x0
  mov x0, x19
  bl choose
  add x20, x20, x0
  mov x0, #3
  bl again
  add x20, x20, x0
  mov x1, #1
  bl far
  add x20, x20, x0
  subs x19, x19, #1
  b.ne 1b
  and x0, x20, #0x7f
  mov x8, #93
  svc #0

// A tail call through a register.
apply:
  adr x9, next
  ldr x9, [x9]
  br x9

step:
  add x0, x0, #3
  ret

// With x1 set, jumps to its own second instruction, which gives back 2; x2 is 0.
wander:
  cbnz x1, 2f
  mov x0, #2
  ret
2:
  adr x9, wander
  adr x10, inside
  ldr x10, [x10]
  eor x10, x10, x2
  add x9, x9, x10
  mov x1, #0
  br x9

// Tail calls itself through a register x0 times, then gives back 7.
again:
  cbz x0, 4f
  sub x0, x0, #1
  adr x9, itself
  ldr x9, [x9]
  br x9
4:
  mov x0, #7
  ret

// A switch on the low two bits of x0, its table's address kept at sp + 16.
choose:
  sub sp, sp, #32
  adr x1, cases
  str x1, [sp, #16]
  and x0, x0, #3
  cmp x0, #3
  b.hi 9f
  ldr x1, [sp, #16]
  ldrb w0, [x1, x0]
  adr x1, 3f
  add x0, x1, x0, lsl #2
  br x0
3:
  mov x0, #10
  b 9f
  mov x0, #20
  b 9f
  mov x0, #30
  b 9f
  mov x0, #40
9:
  add sp, sp, #32
  ret

// FAR_BEFORE and then FAR_AFTER blocks counting in x0, more than one part holds. With x1 set, jumps into the
// last FAR_AFTER, at an address no table explains, and gives back FAR_AFTER.
far:
  mov x0, #0
  cbz x1, 5f
  adr x9, far
  adr x10, into_far
  ldr x10, [x10]
  add x9, x9, x10
  br x9
5:
  .rept FAR_BEFORE
  add x0, x0, #1
  cbz x0, 6f
  .endr
far_inside:
  .rept FAR_AFTER
  add x0, x0, #1
  cbz x0, 6f
  .endr
6:
  ret

.data
cases:
  .byte 0, 2, 4, 6
.balign 8
next:
  .quad step
inside:
  .quad 4
itself:
  .quad again
into_far:
  .quad far_inside - far
