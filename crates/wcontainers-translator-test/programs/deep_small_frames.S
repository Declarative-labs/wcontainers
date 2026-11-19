// A million calls deep, each keeping its return address in 8 bytes of a stack of its own through x28, then
// exits with the low byte of the sum of the depths.
.text
.globl _start
_start:
  mov x28, sp
  movz x0, #0x4240
  movk x0, #0xf, lsl #16
  mov x1, #0
  bl down
  and x0, x1, #0xff
  mov x8, #93
  svc #0
down:
  cbz x0, 1f
  str x30, [x28, #-8]!
  add x1, x1, x0
  sub x0, x0, #1
  bl down
  ldr x30, [x28], #8
1:
  ret
