// A callee returns with the stack pointer on a page that allows nothing; the caller's next store through rsp,
// inside what was its frame, faults.
.text
.globl _start
_start:
  sub $32, %rsp
  movq $0, 8(%rsp)
  call f
  movq $77, 8(%rsp)
  mov $77, %edi
  mov $60, %eax
  syscall
f:
  pop %rax
  mov $0x10000, %esp
  jmp *%rax
