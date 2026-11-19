// A callee returns with the stack pointer 64 bytes further down the stack (its return address copied down
// first); the caller then uses its frame through rsp, and exits with what it read back.
.text
.globl _start
_start:
  sub $32, %rsp
  movq $0, 8(%rsp)
  call f
  movq $77, 8(%rsp)
  mov 8(%rsp), %rdi
  mov $60, %eax
  syscall
f:
  sub $64, %rsp
  mov 64(%rsp), %rax
  mov %rax, (%rsp)
  ret
