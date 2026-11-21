// One function with a frame larger than the engine stack budget at full look-ahead: 150 loops, each loading
// and storing through x0 to x27, so it keeps thousands of locals. It calls itself three levels deep, then
// exits with the byte its loops copied.
.text
.globl _start
_start:
  adrp x0, area
  add x0, x0, :lo12:area
  .irp r, x1,x2,x3,x4,x5,x6,x7,x8,x9,x10,x11,x12,x13,x14,x15,x16,x17,x18,x19,x20,x21,x22,x23,x24,x25,x26,x27
  mov \r, x0
  .endr
  bl work
  ldr x0, [x0, #8]
  mov x8, #93
  svc #0
work:
  stp x29, x30, [sp, #-16]!
  .rept 150
  mov x28, #2
1:
  .irp r, x0,x1,x2,x3,x4,x5,x6,x7,x8,x9,x10,x11,x12,x13,x14,x15,x16,x17,x18,x19,x20,x21,x22,x23,x24,x25,x26,x27
  ldr x29, [\r]
  str x29, [\r, #8]
  .endr
  subs x28, x28, #1
  b.ne 1b
  .endr
  ldr x29, [x0, #16]
  subs x29, x29, #1
  str x29, [x0, #16]
  b.eq 2f
  bl work
2:
  ldp x29, x30, [sp], #16
  ret
.data
.balign 4096
area:
  .quad 42
  .quad 0
  .quad 3
