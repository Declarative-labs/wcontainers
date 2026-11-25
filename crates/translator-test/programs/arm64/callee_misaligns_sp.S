// A callee returns with the stack pointer no longer a multiple of 16; the caller's next access through sp
// is an alignment fault.
.text
.globl _start
_start:
  sub sp, sp, #32
  str xzr, [sp, #8]
  bl f
  mov x5, #77
  str x5, [sp, #8]
  mov x0, x5
  mov x8, #93
  svc #0
f:
  sub sp, sp, #8
  ret
