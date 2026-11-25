// Flags live across calls: set before a call to a callee that leaves them alone and read after it (adc on
// CF, cmov on ZF), set partly by a callee (sub then inc, which keeps CF) and read in parts after the return,
// and a loop whose branch reads the ZF its callee's dec leaves. Each run stores what it read at rdi.

#include "syscall.h"

__asm__(
    ".text\n"
    // Moves, lea, push and pop only: the flags pass through.
    "quiet:\n"
    "  push %rbx\n"
    "  lea 7(%rdi), %rbx\n"
    "  mov %rbx, %rax\n"
    "  pop %rbx\n"
    "  ret\n"
    // CF from the sub (3 - 5 borrows), the rest from the inc of the difference.
    "partial:\n"
    "  mov $3, %eax\n"
    "  sub $5, %eax\n"
    "  inc %eax\n"
    "  ret\n"
    // ZF (and SF) from counting the word at rsi down by one.
    "count:\n"
    "  decq (%rsi)\n"
    "  ret\n"
    // through(rdi = out): CF and ZF set, the quiet call, then adc and cmovz read them.
    "through:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  xor %ecx, %ecx\n"
    "  mov $1, %edx\n"
    "  cmp %rdx, %rcx\n"
    "  call quiet\n"
    "  mov $40, %eax\n"
    "  adc $1, %eax\n"
    "  mov %al, (%rbx)\n"
    "  xor %ecx, %ecx\n"
    "  cmp %rcx, %rcx\n"
    "  call quiet\n"
    "  mov $9, %eax\n"
    "  mov $5, %edx\n"
    "  cmovz %edx, %eax\n"
    "  mov %al, 1(%rbx)\n"
    "  pop %rbx\n"
    "  ret\n"
    // halves(rdi = out): the flags partial leaves, each read separately after the return.
    "halves:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  call partial\n"
    "  setc (%rbx)\n"
    "  setz 1(%rbx)\n"
    "  sets 2(%rbx)\n"
    "  seto 3(%rbx)\n"
    "  setp 4(%rbx)\n"
    "  call partial\n"
    "  jc 1f\n"
    "  movb $7, 5(%rbx)\n"
    "1:\n"
    "  pop %rbx\n"
    "  ret\n"
    // loop(rdi = out, rsi = counter word): the call's ZF ends the loop, its SF picks a value after.
    "loop:\n"
    "  push %rbx\n"
    "  push %r12\n"
    "  mov %rdi, %rbx\n"
    "  xor %r12d, %r12d\n"
    "2:\n"
    "  inc %r12d\n"
    "  call count\n"
    "  jnz 2b\n"
    "  mov %r12b, (%rbx)\n"
    "  call count\n"
    "  mov $1, %eax\n"
    "  mov $2, %edx\n"
    "  cmovs %edx, %eax\n"
    "  mov %al, 1(%rbx)\n"
    "  pop %r12\n"
    "  pop %rbx\n"
    "  ret\n");

void through(char *out);
void halves(char *out);
void loop(char *out, long *counter);

static char line[64];

static void show(const char *name, const char *bytes, int n) {
    int k = 0;
    while (name[k]) {
        line[k] = name[k];
        k++;
    }
    for (int i = 0; i < n; i++) {
        line[k++] = ' ';
        line[k++] = (char)('0' + bytes[i] / 10);
        line[k++] = (char)('0' + bytes[i] % 10);
    }
    line[k++] = '\n';
    syscall3(SYS_write, 1, (long)line, k);
}

static int main(void) {
    static char out[8];
    static long counter = 6;
    through(out);
    show("through", out, 2);
    halves(out);
    show("halves", out, 6);
    loop(out, &counter);
    show("loop", out, 2);
    return 0;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
