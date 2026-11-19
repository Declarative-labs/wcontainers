/* Writes code and runs it, announcing each change with the architecture's sequence (dc cvau, dsb ish, ic ivau,
   dsb ish, isb). The compiler's __builtin___clear_cache is not used: zig's leaves out the dsb after ic ivau, and
   without it real hardware may run the old code (it did in 64 of 10,000 runs in Docker). First in private
   memory: a function that gives 1, rewritten to give 2. Then in memory shared with a child: the parent runs a
   function that gives 3, the child rewrites it to give 4 and announces it, and the parent runs it again: the
   parent's translation must be dropped although the parent announced nothing. Last, in private memory again:
   a function that gives 5 is rewritten to give 6 and only then is its page, the first of two mapped together,
   made not executable, so the kernel learns of changed code that ran; it is announced and made executable
   again before it runs. Then a function whose load's page test is placed after its last instruction and whose
   end branches to a ret on the next page has only an instruction on its second cache line rewritten and that
   line announced: the translation must be dropped although its last host code belongs to its first
   instruction. */

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

typedef int (*function)(void);
typedef int (*loading)(uint64_t *);

/* Makes the code at `at` the one that runs. One line of each cache suffices: the two instructions start a page,
   and no cache line is shorter than four instructions. */
static void announce(uint32_t *at) {
    __asm__ volatile("dc cvau, %0\n\tdsb ish\n\tic ivau, %0\n\tdsb ish\n\tisb" : : "r"(at) : "memory");
}

/* mov w0, #value; ret */
static void write_function(uint32_t *at, uint32_t value) {
    at[0] = 0x52800000u | (value << 5);
    at[1] = 0xd65f03c0u;
    announce(at);
}

static uint32_t *code_memory(int flags, size_t length) {
    void *at = mmap(0, length, PROT_READ | PROT_WRITE | PROT_EXEC, flags | MAP_ANONYMOUS, -1, 0);
    if (at == MAP_FAILED) {
        perror("mmap");
        _exit(1);
    }
    return at;
}

/* Changes what the page at `at` allows. */
static void protect(uint32_t *at, int permissions) {
    if (mprotect(at, 4096, permissions) != 0) {
        perror("mprotect");
        _exit(1);
    }
}

int main(void) {
    uint32_t *private_code = code_memory(MAP_PRIVATE, 4096);
    write_function(private_code, 1);
    int first = ((function)private_code)();
    write_function(private_code, 2);
    int second = ((function)private_code)();
    printf("private: %d %d\n", first, second);

    uint32_t *shared_code = code_memory(MAP_SHARED, 4096);
    write_function(shared_code, 3);
    int before = ((function)shared_code)();
    pid_t child = fork();
    if (child == 0) {
        write_function(shared_code, 4);
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    int after = ((function)shared_code)();
    printf("shared, changed by a child: %d %d\n", before, after);

    uint32_t *unannounced = code_memory(MAP_PRIVATE, 2 * 4096);
    write_function(unannounced, 5);
    int ran = ((function)unannounced)();
    unannounced[0] = 0x52800000u | (6 << 5);
    protect(unannounced, PROT_READ | PROT_WRITE);
    announce(unannounced);
    protect(unannounced, PROT_READ | PROT_WRITE | PROT_EXEC);
    int changed = ((function)unannounced)();
    printf("private, changed before it was made not executable: %d %d\n", ran, changed);

    /* ldr x1, [x0]; b to the next cache line, where mov w0, #7 and a b to the next page's ret are. Only that
       line is announced when the mov is rewritten. */
    uint32_t *loads = code_memory(MAP_PRIVATE, 2 * 4096);
    uint32_t words = 4096 / sizeof(uint32_t);
    uint32_t line = 64 / sizeof(uint32_t);
    loads[0] = 0xf9400001u;
    loads[1] = 0x14000000u | (line - 1);
    loads[line] = 0x52800000u | (7 << 5);
    loads[line + 1] = 0x14000000u | (words - line - 1);
    loads[words] = 0xd65f03c0u;
    announce(loads);
    announce(loads + line);
    announce(loads + words);
    uint64_t loaded = 0;
    int seven = ((loading)loads)(&loaded);
    loads[line] = 0x52800000u | (8 << 5);
    announce(loads + line);
    int eight = ((loading)loads)(&loaded);
    printf("private, middle rewritten after a load: %d %d\n", seven, eight);
    return 0;
}
