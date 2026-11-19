// Flags of shifts, rotates and bit tests live across calls: a shift by cl of zero that keeps the flags of a
// compare before it, read after a call; a shift by an immediate whose CF and ZF are read after a call; a
// rotate and a bt whose CF and OF a callee leaves for the caller; stc and cmc in a callee. Each stores what
// it read at rdi, one byte a flag.

#include "syscall.h"

__asm__(
    ".text\n"
    // Moves and lea only: the flags pass through.
    "keep:\n"
    "  lea 3(%rsi), %rax\n"
    "  mov %rax, %rdx\n"
    "  ret\n"
    // CF set, then complemented: the caller reads CF clear, the rest of the flags as they came in.
    "carry_twice:\n"
    "  stc\n"
    "  cmc\n"
    "  ret\n"
    // kept(rdi = out, rsi = count): 3 < 9 sets CF and SF; a shift by cl = count keeps them when it is zero.
    "kept:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  mov %esi, %ecx\n"
    "  mov $3, %eax\n"
    "  mov $0x80000001, %edx\n"
    "  cmp $9, %eax\n"
    "  shl %cl, %edx\n"
    "  call keep\n"
    "  setc 0(%rbx)\n"
    "  sets 1(%rbx)\n"
    "  setz 2(%rbx)\n"
    "  setp 3(%rbx)\n"
    "  seto 4(%rbx)\n"
    "  pop %rbx\n"
    "  ret\n"
    // shifted(rdi = out): shr by 1 of an odd number sets CF, sar by 4 of 0xf gives ZF; read after calls.
    "shifted:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  mov $0x15, %eax\n"
    "  shr $1, %eax\n"
    "  call keep\n"
    "  setc 0(%rbx)\n"
    "  setz 1(%rbx)\n"
    "  setp 2(%rbx)\n"
    "  mov $0xf, %eax\n"
    "  sar $4, %eax\n"
    "  call keep\n"
    "  setz 3(%rbx)\n"
    "  setc 4(%rbx)\n"
    "  sets 5(%rbx)\n"
    "  pop %rbx\n"
    "  ret\n"
    // turned(rdi = out): ZF from a sub survives a rotate and a bt, CF and OF come from them, and a callee's
    // stc and cmc leave CF clear for the caller.
    "turned:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  mov $5, %eax\n"
    "  sub $5, %eax\n"
    "  mov $0x40000000, %edx\n"
    "  rol $1, %edx\n"
    "  call keep\n"
    "  setz 0(%rbx)\n"
    "  setc 1(%rbx)\n"
    "  seto 2(%rbx)\n"
    "  mov $6, %edx\n"
    "  bt $2, %edx\n"
    "  call keep\n"
    "  setc 3(%rbx)\n"
    "  setz 4(%rbx)\n"
    "  call carry_twice\n"
    "  setc 5(%rbx)\n"
    "  setz 6(%rbx)\n"
    "  pop %rbx\n"
    "  ret\n");

void kept(char *out, unsigned count);
void shifted(char *out);
void turned(char *out);

static char line[64];

static void show(const char *name, const char *bytes, int n) {
    int k = 0;
    while (name[k]) {
        line[k] = name[k];
        k++;
    }
#pragma clang loop vectorize(disable) interleave(disable) unroll(disable)
    for (int i = 0; i < n; i++) {
        line[k++] = ' ';
        line[k++] = (char)('0' + bytes[i]);
    }
    line[k++] = '\n';
    syscall3(SYS_write, 1, (long)line, k);
}

static int main(void) {
    static char out[8];
    kept(out, 0);
    show("kept 0", out, 5);
    kept(out, 1);
    show("kept 1", out, 5);
    kept(out, 32);
    show("kept 32", out, 5);
    shifted(out);
    show("shifted", out, 6);
    turned(out);
    show("turned", out, 7);
    return 0;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
