// The trap flag set by the program: the instruction CASE picks runs with TF set and the thread stops after
// it with SIGTRAP (TRAP_TRACE), as section 5.1 of the plan says. Every register holds a value of its own.
// From the build: CASE, TF (the trap flag's bit in rflags), SYS_write and SYS_exit.
// Bytes below the stack pointer the cases push into and copy within.
#define SCRATCH 256
.text
.globl _start
_start:
  mov $0x1111, %rax
  mov $0x2222, %rbx
  mov $0x3333, %rcx
  mov $0x4444, %rdx
  mov $0x5555, %rsi
  mov $0x6666, %rdi
  mov $0x7777, %rbp
  mov $0x8888, %r8
  mov $0x9999, %r9
  mov $0xaaaa, %r10
  mov $0xbbbb, %r11
  mov $0xcccc, %r12
  mov $0xdddd, %r13
  mov $0xeeee, %r14
  mov $0xffff, %r15
  sub $SCRATCH, %rsp
#if CASE == 8
  // iretq to `landed` with TF in the rflags it loads: the instruction there runs, then the stop.
  mov %ss, %eax
  push %rax
  lea 8(%rsp), %rax
  push %rax
  pushf
  orq $TF, (%rsp)
  mov %cs, %eax
  push %rax
  lea landed(%rip), %rax
  push %rax
  mov $0x1111, %eax
  iretq
landed:
  inc %rax
#else
  // What the stepped instruction needs, made before TF is set.
#if CASE == 3
  lea 1f(%rip), %rdx
  push %rdx
#elif CASE == 4
  // write(1, rsp, 0): the stop comes after the instruction after the system call.
  mov $SYS_write, %eax
  mov $1, %edi
  mov %rsp, %rsi
  xor %edx, %edx
#elif CASE == 5
  pushf
  andq $~TF, (%rsp)
#elif CASE == 6
  mov $3, %ecx
  lea SCRATCH/4(%rsp), %rdi
  lea SCRATCH/2(%rsp), %rsi
#elif CASE == 7
  mov %ss, %eax
#elif CASE == 10
  mov $3, %ecx
  lea written(%rip), %rdi
  mov $0x90, %al
#endif
  pushf
  orq $TF, (%rsp)
  popf
#if CASE == 0
  nop
#elif CASE == 1
  jmp 1f
  ud2
1:
#elif CASE == 2
  call 1f
  ud2
1:
#elif CASE == 3
  ret
  ud2
1:
#elif CASE == 4
  syscall
  inc %r12
#elif CASE == 5
  // A popf that clears TF still stops after itself.
  popf
#elif CASE == 6
  // One element, with rip still on the instruction.
  rep movsb
#elif CASE == 7
  // A load of ss holds the trap back one instruction.
  mov %eax, %ss
  inc %r13
#elif CASE == 9
  // A store into this page of translated code: it is done, then the stop.
  movb $0x90, written(%rip)
#elif CASE == 10
  // A rep pass whose element writes this page of translated code: one element, rip on the instruction.
  rep stosb
#endif
#endif
  inc %r14
  inc %r15
  // Not reached: the single step stops the program first.
  mov $SYS_exit, %eax
  xor %edi, %edi
  syscall
  // Code bytes cases 9 and 10 write.
written:
  nop
  nop
  nop
