// A callee returns with the stack pointer moved by 8, no longer a multiple of 16 where the caller's was; x86
// has no alignment fault here, so the caller's frame through rsp works and it exits with what it read back.
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
  sub $8, %rsp
  mov 8(%rsp), %rax
  mov %rax, (%rsp)
  ret
