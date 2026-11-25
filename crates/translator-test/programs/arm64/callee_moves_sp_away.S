// A callee returns with the stack pointer on a page that allows nothing; the caller's next store through sp,
// inside what was its frame, faults.
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
  mov x9, #0x10000
  mov sp, x9
  ret
