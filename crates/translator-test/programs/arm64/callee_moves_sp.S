// A callee returns with the stack pointer 64 bytes further down the stack; the caller then uses its frame
// through sp, and exits with what it read back.
.text
.globl _start
_start:
  sub sp, sp, #32
  str xzr, [sp, #8]
  bl f
  mov x5, #77
  str x5, [sp, #8]
  ldr x0, [sp, #8]
  mov x8, #93
  svc #0
f:
  sub sp, sp, #64
  ret
