/* Jumps through a byte-indexed jump table, copied into memory this program maps itself so that it controls which
   targets can run: a dispatch loop that rewrites its own table while it runs; a target whose code is rewritten
   and announced; a target on a page that is not executable, then executable and run, then not executable again,
   then unmapped, then zero; a table of byte offsets with an odd entry, so a misaligned target; a table of signed
   bytes, one of them reaching back onto a zeroed page; and a byte load that writes back its base, so the register
   added to the base is a pointer, not a byte. Every fault is caught and printed. */

#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#define PAGE 4096
#define CODE ((uintptr_t)0x20000000)
#define DATA ((uintptr_t)0x10000000)
#define LOW ((uintptr_t)0x10000)

extern const uint32_t jt_loop[], jt_loop_head[], jt_loop_base[], jt_loop_rewritable[], jt_loop_end[];
extern const uint32_t jt_bytes[], jt_bytes_end[];
extern const uint32_t jt_back[], jt_back_base[], jt_back_end[];
extern const uint32_t jt_signed[], jt_signed_end[];

__asm__(".text\n.p2align 2\n"
        ".globl jt_loop\njt_loop:\n"
        "  mov x6, x1\n"
        "  mov x5, #0\n"
        ".globl jt_loop_head\njt_loop_head:\n"
        "1: ldrb w3, [x0], #1\n"
        "  ldrb w3, [x6, x3]\n"
        "  adr x1, 2f\n"
        "  add x1, x1, x3, lsl #2\n"
        "  br x1\n"
        ".globl jt_loop_base\njt_loop_base:\n"
        "2: add x5, x5, #1\n  b 1b\n"
        "  add x5, x5, x5\n  b 1b\n"
        "  ldrh w8, [x6]\n  rev16 w8, w8\n  strh w8, [x6]\n  b 1b\n"
        "  mov x0, x5\n  ret\n"
        ".globl jt_loop_rewritable\njt_loop_rewritable:\n"
        "  add x5, x5, #100\n  b 1b\n"
        ".globl jt_loop_end\njt_loop_end:\n"
        ".globl jt_bytes\njt_bytes:\n"
        "  mov x6, x1\n"
        "  ldrb w3, [x0]\n"
        "  ldrb w3, [x6, x3]\n"
        "  adr x1, 3f\n"
        "  add x1, x1, x3\n"
        "  br x1\n"
        "3: mov x0, #77\n  ret\n"
        "  mov x0, #78\n  ret\n"
        ".globl jt_bytes_end\njt_bytes_end:\n"
        ".globl jt_back\njt_back:\n"
        "  mov x3, x0\n"
        "  adr x1, 4f\n"
        "  ldrb w9, [x3], #1\n"
        "  add x1, x1, x3, lsl #2\n"
        "  br x1\n"
        ".globl jt_back_base\njt_back_base:\n"
        "4: mov x0, #55\n  ret\n"
        ".globl jt_back_end\njt_back_end:\n"
        ".globl jt_signed\njt_signed:\n"
        "  mov x6, x1\n"
        "  mov x5, #0\n"
        "  b 5f\n"
        "  add x5, x5, #10\n"
        "  b 5f\n"
        "5: ldrb w3, [x0], #1\n"
        "  ldrb w3, [x6, x3]\n"
        "  adr x1, 6f\n"
        "  add x1, x1, w3, sxtb #2\n"
        "  br x1\n"
        "  add x5, x5, x5\n"
        "6: add x5, x5, #1\n"
        "  b 5b\n"
        "  mov x0, x5\n  ret\n"
        ".globl jt_signed_end\njt_signed_end:\n");

static sigjmp_buf back;
static volatile int caught_signal, caught_code;
static volatile uintptr_t caught_address;

static void on_fault(int signal, siginfo_t *info, void *context) {
    (void)context;
    caught_signal = signal;
    caught_code = info->si_code;
    caught_address = (uintptr_t)info->si_addr;
    siglongjmp(back, 1);
}

static void announce(const void *at, size_t bytes) {
    for (uintptr_t a = (uintptr_t)at & ~(uintptr_t)63; a < (uintptr_t)at + bytes; a += 64)
        __asm__ volatile("dc cvau, %0" : : "r"(a) : "memory");
    __asm__ volatile("dsb ish" : : : "memory");
    for (uintptr_t a = (uintptr_t)at & ~(uintptr_t)63; a < (uintptr_t)at + bytes; a += 64)
        __asm__ volatile("ic ivau, %0" : : "r"(a) : "memory");
    __asm__ volatile("dsb ish\n\tisb" : : : "memory");
}

static void *map_at(uintptr_t at, size_t bytes, int protection) {
    void *p = mmap((void *)at, bytes, protection, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    return p == MAP_FAILED ? 0 : p;
}

typedef uint64_t (*dispatch)(const uint8_t *ops, uint8_t *table);

static uint32_t *copy(uintptr_t at, const uint32_t *start, const uint32_t *end) {
    memcpy((void *)at, start, (size_t)((const char *)end - (const char *)start));
    announce((void *)at, (size_t)((const char *)end - (const char *)start));
    return (uint32_t *)at;
}

static uint32_t branch_to(const uint32_t *from, const uint32_t *to) {
    return 0x14000000u | ((uint32_t)(to - from) & 0x03ffffffu);
}

static void report(const char *what, uint64_t value) {
    printf("%s: %llu\n", what, (unsigned long long)value);
}

static void report_fault(const char *what) {
    uintptr_t a = caught_address;
    printf("%s: signal %d code %d address %s%#lx\n", what, caught_signal, caught_code,
           a >= CODE ? "CODE+" : "", (unsigned long)(a >= CODE ? a - CODE : a));
}

/* Runs `ops` through dispatcher `f` with `table`, printing its result or the fault it takes. */
static void run(const char *what, void *f, const uint8_t *ops, size_t n_ops, uint8_t *table) {
    uint8_t *stream = (uint8_t *)DATA;
    memcpy(stream, ops, n_ops);
    if (sigsetjmp(back, 1) == 0) {
        report(what, ((dispatch)f)(stream, table));
    } else {
        report_fault(what);
    }
}

int main(void) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_sigaction = on_fault;
    action.sa_flags = SA_SIGINFO | SA_NODEFER;
    sigaction(SIGSEGV, &action, 0);
    sigaction(SIGBUS, &action, 0);
    sigaction(SIGILL, &action, 0);

    if (!map_at(DATA, 2 * PAGE, PROT_READ | PROT_WRITE)) {
        puts("no data");
        return 1;
    }
    if (!map_at(CODE, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC) ||
        !map_at(CODE + 2 * PAGE, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC)) {
        puts("no code");
        return 1;
    }
    /* Page 1 of the code, not executable at first. */
    if (!map_at(CODE + PAGE, PAGE, PROT_READ | PROT_WRITE)) {
        puts("no code page 1");
        return 1;
    }
    /* The loop's base sits 40 words before page 1, so entry 40 is page 1's first word. */
    size_t base_offset = (size_t)(jt_loop_base - jt_loop);
    uint32_t *loop = copy(CODE + PAGE - 40 * 4 - base_offset * 4, jt_loop, jt_loop_end);
    uint32_t *loop_head = loop + (jt_loop_head - jt_loop);
    uint32_t *loop_rewritable = loop + (jt_loop_rewritable - jt_loop);
    uint32_t *loop_base = loop + base_offset;
    uint32_t *bytes = copy(CODE, jt_bytes, jt_bytes_end);
    uint32_t *back_jump = copy(CODE + 256, jt_back, jt_back_end);
    uint32_t *back_base = back_jump + (jt_back_base - jt_back);
    uint32_t *signed_jump = copy(CODE + 2 * PAGE, jt_signed, jt_signed_end);

    uint8_t *table = (uint8_t *)DATA + PAGE;
    /* op -> word offset from the base: 0 inc, 1 double, 2 swap entries 0 and 1, 3 halt, 4 the rewritable
       handler, 5 page 1's first word, 6 the last entry a byte reaches (page 1, never written: zero). */
    const uint8_t entries[] = {0, 2, 4, 8, (uint8_t)(loop_rewritable - loop_base), 40, 255};
    memcpy(table, entries, sizeof entries);

    const uint8_t loop_ops[] = {0, 1, 0, 2, 1, 0, 0, 2, 1, 1, 0, 2, 0, 1, 3};
    for (int round = 0; round < 3; round++) {
        memcpy(table, entries, sizeof entries);
        run("dispatch loop rewriting its table", loop, loop_ops, sizeof loop_ops, table);
    }
    const uint8_t rewritable_ops[] = {4, 4, 3};
    run("rewritable handler", loop, rewritable_ops, sizeof rewritable_ops, table);
    loop_rewritable[0] = 0x91032000u | (5 << 5) | 5; /* add x5, x5, #200 */
    announce(loop_rewritable, 4);
    run("rewritable handler after rewriting", loop, rewritable_ops, sizeof rewritable_ops, table);

    const uint8_t page1_ops[] = {5, 3};
    run("target on a page never executable", loop, page1_ops, sizeof page1_ops, table);
    uint32_t *page1 = (uint32_t *)(CODE + PAGE);
    mprotect(page1, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC);
    page1[0] = 0x910fa000u | (5 << 5) | 5; /* add x5, x5, #1000 */
    page1[1] = branch_to(page1 + 1, loop_head);
    announce(page1, 8);
    const uint8_t page1_twice[] = {5, 5, 3};
    run("target on page 1 made executable", loop, page1_twice, sizeof page1_twice, table);
    run("target on page 1 again (linked)", loop, page1_twice, sizeof page1_twice, table);
    mprotect(page1, PAGE, PROT_READ | PROT_WRITE);
    run("target on page 1 made not executable", loop, page1_ops, sizeof page1_ops, table);
    mprotect(page1, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC);
    run("target on page 1 executable again", loop, page1_ops, sizeof page1_ops, table);
    munmap(page1, PAGE);
    run("target on page 1 unmapped", loop, page1_ops, sizeof page1_ops, table);
    map_at(CODE + PAGE, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC);
    const uint8_t last_ops[] = {6, 3};
    run("last entry, a zero word", loop, last_ops, sizeof last_ops, table);

    /* Byte offsets: 0 gives 77, 8 gives 78, 2 is misaligned. */
    uint8_t *bytes_table = table + 64;
    const uint8_t byte_entries[] = {0, 8, 2};
    memcpy(bytes_table, byte_entries, sizeof byte_entries);
    for (uint8_t op = 0; op < 3; op++) {
        char what[64];
        snprintf(what, sizeof what, "byte offsets, op %u", op);
        run(what, bytes, &op, 1, bytes_table);
        run(what, bytes, &op, 1, bytes_table);
    }

    /* Signed bytes: 0 inc, -1 double then inc, -8 add ten, 2 halt, -128 back onto page 1, which is zero. */
    uint8_t *signed_table = table + 128;
    const uint8_t signed_entries[] = {0, 0xff, 0xf8, 2, 0x80};
    memcpy(signed_table, signed_entries, sizeof signed_entries);
    const uint8_t signed_ops[] = {0, 1, 2, 0, 1, 3};
    for (int round = 0; round < 2; round++)
        run("signed bytes", signed_jump, signed_ops, sizeof signed_ops, signed_table);
    const uint8_t signed_back_ops[] = {0, 4};
    run("signed bytes reaching back", signed_jump, signed_back_ops, sizeof signed_back_ops, signed_table);

    /* Writeback: the register added is the pointer after the load. A pointer to page LOW makes the target
       back_base + 4 * (LOW + 1), where a function giving 66 is put; a pointer into the data makes it unmapped. */
    uintptr_t landing = (uintptr_t)back_base + 4 * (LOW + 1);
    int low = map_at(LOW, PAGE, PROT_READ | PROT_WRITE) != 0;
    int landing_mapped = map_at(landing & ~(uintptr_t)(PAGE - 1), PAGE, PROT_READ | PROT_WRITE | PROT_EXEC) != 0;
    printf("writeback: low page %d, landing page %d\n", low, landing_mapped);
    if (landing_mapped) {
        uint32_t *l = (uint32_t *)landing;
        l[0] = 0xd2800840u; /* mov x0, #66 */
        l[1] = 0xd65f03c0u; /* ret */
        announce(l, 8);
    }
    for (int k = 0; k < 2; k++) {
        if (low) {
            if (sigsetjmp(back, 1) == 0) {
                report("writeback from the low page", ((uint64_t (*)(uintptr_t))back_jump)(LOW));
            } else {
                report_fault("writeback from the low page");
            }
        }
        if (sigsetjmp(back, 1) == 0) {
            report("writeback from the data", ((uint64_t (*)(uintptr_t))back_jump)(DATA + 16));
        } else {
            report_fault("writeback from the data");
        }
    }
    puts("done");
    return 0;
}
