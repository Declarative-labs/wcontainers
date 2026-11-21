// Switches on an unsigned int as gcc 9.4 and clang 10 write them at -O2 with -fPIC: the guard on the 32-bit
// index, then a move that zero-extends it into the register the table's entry is read through. Each case
// leaves a different value; the program prints a hash of them over indices in and out of range.

#include "syscall.h"

__asm__(
    ".text\n"
    // gcc: the table's address, then `movl %edi, %edi`, the sum in a third register, `notrack jmp`.
    "by_gcc:\n"
    "  endbr64\n"
    "  cmpl $6, %edi\n"
    "  ja .Lgcc_out\n"
    "  leaq .Lgcc_table(%rip), %rdx\n"
    "  movl %edi, %edi\n"
    "  movslq (%rdx,%rdi,4), %rax\n"
    "  addq %rdx, %rax\n"
    "  notrack jmp *%rax\n"
    "  .section .rodata\n"
    "  .p2align 2\n"
    ".Lgcc_table:\n"
    "  .long .Lgcc_0-.Lgcc_table\n"
    "  .long .Lgcc_1-.Lgcc_table\n"
    "  .long .Lgcc_2-.Lgcc_table\n"
    "  .long .Lgcc_3-.Lgcc_table\n"
    "  .long .Lgcc_4-.Lgcc_table\n"
    "  .long .Lgcc_5-.Lgcc_table\n"
    "  .long .Lgcc_6-.Lgcc_table\n"
    "  .text\n"
    ".Lgcc_0:\n"
    "  leal 1(%rsi,%rsi,2), %eax\n"
    "  ret\n"
    ".Lgcc_1:\n"
    "  movl %esi, %eax\n"
    "  xorl $85, %eax\n"
    "  ret\n"
    ".Lgcc_2:\n"
    "  leal -17(%rsi), %eax\n"
    "  ret\n"
    ".Lgcc_3:\n"
    "  leal 0(,%rsi,8), %eax\n"
    "  ret\n"
    ".Lgcc_4:\n"
    "  leal (%rsi,%rsi,4), %eax\n"
    "  ret\n"
    ".Lgcc_5:\n"
    "  movl %esi, %eax\n"
    "  shrl $3, %eax\n"
    "  ret\n"
    ".Lgcc_6:\n"
    "  movl %esi, %eax\n"
    "  notl %eax\n"
    "  ret\n"
    ".Lgcc_out:\n"
    "  xorl %eax, %eax\n"
    "  ret\n"
    // clang: `movl %edi, %ecx` before the table's address, the sum in the register the entry is read into.
    "by_clang:\n"
    "  cmpl $6, %edi\n"
    "  ja .Lclang_out\n"
    "  movl %esi, %eax\n"
    "  movl %edi, %ecx\n"
    "  leaq .Lclang_table(%rip), %rdx\n"
    "  movslq (%rdx,%rcx,4), %rcx\n"
    "  addq %rdx, %rcx\n"
    "  jmpq *%rcx\n"
    ".Lclang_0:\n"
    "  leal (%rax,%rax,2), %eax\n"
    "  addl $1, %eax\n"
    "  retq\n"
    ".Lclang_out:\n"
    "  xorl %eax, %eax\n"
    "  retq\n"
    ".Lclang_1:\n"
    "  xorl $85, %eax\n"
    "  retq\n"
    ".Lclang_2:\n"
    "  addl $-17, %eax\n"
    "  retq\n"
    ".Lclang_3:\n"
    "  shll $3, %eax\n"
    "  retq\n"
    ".Lclang_4:\n"
    "  leal (%rax,%rax,4), %eax\n"
    "  retq\n"
    ".Lclang_5:\n"
    "  shrl $3, %eax\n"
    "  retq\n"
    ".Lclang_6:\n"
    "  notl %eax\n"
    "  retq\n"
    "  .section .rodata\n"
    "  .p2align 2\n"
    ".Lclang_table:\n"
    "  .long .Lclang_0-.Lclang_table\n"
    "  .long .Lclang_1-.Lclang_table\n"
    "  .long .Lclang_2-.Lclang_table\n"
    "  .long .Lclang_3-.Lclang_table\n"
    "  .long .Lclang_4-.Lclang_table\n"
    "  .long .Lclang_5-.Lclang_table\n"
    "  .long .Lclang_6-.Lclang_table\n"
    "  .text\n");

unsigned by_gcc(unsigned x, unsigned y);
unsigned by_clang(unsigned x, unsigned y);

static char line[32];

static int main(void) {
    unsigned long s = 0;
#pragma clang loop vectorize(disable) interleave(disable) unroll(disable)
    for (unsigned r = 0; r < 100; r++) {
#pragma clang loop vectorize(disable) interleave(disable) unroll(disable)
        for (unsigned x = 0; x < 9; x++) {
            s = s * 31 + by_gcc(x, r) + 7 * by_clang(x, r + 3);
        }
    }
    int k = 0;
#pragma clang loop vectorize(disable) interleave(disable) unroll(disable)
    for (int shift = 60; shift >= 0; shift -= 4) {
        line[k++] = "0123456789abcdef"[(s >> shift) & 15];
    }
    line[k++] = '\n';
    syscall3(SYS_write, 1, (long)line, k);
    return 0;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
