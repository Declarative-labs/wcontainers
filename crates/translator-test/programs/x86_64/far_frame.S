// A function moves its stack pointer down by DESCENT at once, stores LOW at the new stack pointer and HIGH
// halfway up, and returns their sum, which the program exits with; a descent past its stack faults.
// From the build: DESCENT, LOW, HIGH and SYS_exit.
.text
.globl _start
_start:
  call far_frame
  mov %eax, %edi
  mov $SYS_exit, %eax
  syscall

far_frame:
  push %rbp
  mov %rsp, %rbp
  sub $DESCENT, %rsp
  movl $LOW, (%rsp)
  movl $HIGH, DESCENT / 2(%rsp)
  mov (%rsp), %eax
  add DESCENT / 2(%rsp), %eax
  leave
  ret
