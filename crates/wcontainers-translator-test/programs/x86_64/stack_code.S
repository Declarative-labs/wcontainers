// Code on the stack: a function stores `mov eax, FIRST; ret` at its stack pointer and calls it there, then
// stores `mov eax, SECOND; ret` at the same place and calls it again; the program exits with the first result
// shifted left by SHIFT, plus the second. The stack is the program's own, on pages that allow running code.
// CASE says how the second store is made: 0 by a push, 1 through rsp, 2 through rbp.
// From the build: CASE, FIRST, SECOND, SHIFT, PAGE_SIZE and SYS_exit.

#define WORD 8
#define BYTE_BITS 8
// `mov eax, imm32` and `ret` as the bytes of one word: the opcode, the immediate, then ret.
#define MOV_EAX 0xb8
#define MOV_EAX_LENGTH 5
#define RET 0xc3
#define CODE(value) (MOV_EAX | ((value) << BYTE_BITS) | (RET << (MOV_EAX_LENGTH * BYTE_BITS)))

.text
.globl _start
_start:
  lea stack_top(%rip), %rsp
  call run_twice
  mov %eax, %edi
  mov $SYS_exit, %eax
  syscall

run_twice:
  push %rbp
  mov %rsp, %rbp
  push %rbx
  movabs $CODE(FIRST), %rax
#if CASE == 0
  push %rax
#else
  sub $WORD, %rsp
  mov %rax, (%rsp)
#endif
  call *%rsp
  mov %eax, %ebx
  movabs $CODE(SECOND), %rax
#if CASE == 0
  pop %rcx
  push %rax
#elif CASE == 1
  mov %rax, (%rsp)
#else
  mov %rax, -2 * WORD(%rbp)
#endif
  call *%rsp
  shl $SHIFT, %ebx
  add %ebx, %eax
  add $WORD, %rsp
  pop %rbx
  pop %rbp
  ret

.bss
.balign PAGE_SIZE
stack:
  .skip 2 * PAGE_SIZE
stack_top:
