// Flags a callee leaves for its caller to read after the return: set by the callee's last arithmetic, by an
// instruction before its pops, by a callee of its own, after a tail jump, through a register and through
// memory. The caller reads them with setcc, so every look-ahead must hand them back as the CPU does.

#include "syscall.h"

// Each test sets flags in the caller (cmp of equal values: ZF, PF), calls, and stores CF, PF, ZF, SF and OF
// after the return as five bytes at rdi.
__asm__(
    ".text\n"
    "sets:\n"
    "  mov $0x7f, %eax\n"
    "  add $1, %al\n"
    "  ret\n"
    "pops:\n"
    "  push %rbx\n"
    "  push %r12\n"
    "  sub $0x18, %rsp\n"
    "  mov $3, %ecx\n"
    "  sub $5, %rcx\n"
    "  add $0x18, %rsp\n"
    "  pop %r12\n"
    "  pop %rbx\n"
    "  ret\n"
    "nested:\n"
    "  push %rbx\n"
    "  call sets\n"
    "  pop %rbx\n"
    "  ret\n"
    "tail:\n"
    "  push %rbx\n"
    "  pop %rbx\n"
    "  jmp pops\n"
    "stack_last:\n"
    "  push %rbx\n"
    "  sub $0x40, %rsp\n"
    "  add $0x40, %rsp\n"
    "  pop %rbx\n"
    "  ret\n"
    "branchy:\n"
    "  push %rbx\n"
    "  test %rdi, %rdi\n"
    "  je 1f\n"
    "  mov (%rdi), %rax\n"
    "  sub (%rdi), %rax\n"
    "  je 1f\n"
    "  nop\n"
    "1:\n"
    "  pop %rbx\n"
    "  ret\n"
    "capture:\n"
    "  setc (%rdi)\n"
    "  setp 1(%rdi)\n"
    "  setz 2(%rdi)\n"
    "  sets 3(%rdi)\n"
    "  seto 4(%rdi)\n"
    "  ret\n"
    // run(rdi = out, rsi = callee): flags set, the call, the capture.
    "run_direct:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  cmp %rsi, %rsi\n"
    "  call *%rsi\n"
    "  mov %rbx, %rdi\n"
    "  call capture\n"
    "  pop %rbx\n"
    "  ret\n"
    "run_memory:\n"
    "  push %rbx\n"
    "  push %rsi\n"
    "  mov %rdi, %rbx\n"
    "  cmp %rsi, %rsi\n"
    "  call *(%rsp)\n"
    "  mov %rbx, %rdi\n"
    "  call capture\n"
    "  pop %rsi\n"
    "  pop %rbx\n"
    "  ret\n"
    "run_named:\n"
    "  push %rbx\n"
    "  mov %rdi, %rbx\n"
    "  cmp %rsi, %rsi\n"
    "  call tail\n"
    "  mov %rbx, %rdi\n"
    "  call capture\n"
    "  xor %eax, %eax\n"
    "  call stack_last\n"
    "  lea 5(%rbx), %rdi\n"
    "  call capture\n"
    "  xor %eax, %eax\n"
    "  lea 16(%rbx), %rdi\n"
    "  call branchy\n"
    "  lea 10(%rbx), %rdi\n"
    "  call capture\n"
    "  pop %rbx\n"
    "  ret\n");

void sets(void), pops(void), nested(void), tail(void), stack_last(void), branchy(void);
void run_direct(char *out, void (*callee)(void));
void run_memory(char *out, void (*callee)(void));
void run_named(char *out);

static char line[64];

static void show(const char *name, const char *flags, int n) {
    int k = 0;
    while (name[k]) {
        line[k] = name[k];
        k++;
    }
    line[k++] = ' ';
    for (int i = 0; i < n; i++) line[k++] = (char)('0' + flags[i]);
    line[k++] = '\n';
    syscall3(SYS_write, 1, (long)line, k);
}

static int main(void) {
    // Two words the branchy callee subtracts: equal, so ZF.
    static char out[32] = {0};
    void (*callees[])(void) = {sets, pops, nested, tail};
    const char *names[] = {"sets", "pops", "nested", "tail"};
    for (int i = 0; i < 4; i++) {
        run_direct(out, callees[i]);
        show(names[i], out, 5);
        run_memory(out, callees[i]);
        show(names[i], out, 5);
    }
    run_named(out);
    show("named", out, 15);
    return 0;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
