// One function with a frame larger than the engine stack budget at full look-ahead: 150 loops, each loading
// and storing through rax to r14, so it keeps thousands of locals. It calls itself three levels deep, then
// exits with the byte its loops copied.
.text
.globl _start
_start:
  lea area(%rip), %rax
  .irp r, rbx,rcx,rdx,rsi,rdi,r8,r9,r10,r11,r12,r13,r14
  mov %rax, %\r
  .endr
  call work
  mov 8(%rax), %rdi
  mov $60, %eax
  syscall
work:
  push %rbp
  .rept 150
  mov $2, %r15d
1:
  .irp r, rax,rbx,rcx,rdx,rsi,rdi,r8,r9,r10,r11,r12,r13,r14
  mov (%\r), %rbp
  mov %rbp, 8(%\r)
  .endr
  dec %r15d
  jnz 1b
  .endr
  mov 16(%rax), %rbp
  dec %rbp
  mov %rbp, 16(%rax)
  jz 2f
  call work
2:
  pop %rbp
  ret
.data
.balign 4096
area:
  .quad 42
  .quad 0
  .quad 3
