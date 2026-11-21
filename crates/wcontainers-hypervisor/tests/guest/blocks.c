/* Zeroes blocks (dc zva) and cleans cache lines (dc cvac, cvau, civac) at addresses inside and outside the
   block, on pages never touched, touched, read-only, allowing nothing and unmapped, and says for each whether
   it ran or which signal it took at which offset into the mapping. A zeroed block is checked against its
   neighbours. */

#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

static sigjmp_buf back;
static volatile uintptr_t faulted;

static void on_fault(int signal, siginfo_t *info, void *context) {
    (void)context;
    faulted = (uintptr_t)info->si_addr;
    siglongjmp(back, signal);
}

enum operation { ZERO, CLEAN, CLEAN_UNIFICATION, CLEAN_INVALIDATE };

static void operate(enum operation operation, char *at) {
    switch (operation) {
    case ZERO: __asm__ volatile("dc zva, %0" : : "r"(at) : "memory"); break;
    case CLEAN: __asm__ volatile("dc cvac, %0" : : "r"(at) : "memory"); break;
    case CLEAN_UNIFICATION: __asm__ volatile("dc cvau, %0" : : "r"(at) : "memory"); break;
    case CLEAN_INVALIDATE: __asm__ volatile("dc civac, %0" : : "r"(at) : "memory"); break;
    }
}

static const char *names[] = {"zva", "cvac", "cvau", "civac"};

static char *base;

static void attempt(const char *where, enum operation operation, char *at) {
    int signal = sigsetjmp(back, 1);
    if (signal == 0) {
        operate(operation, at);
        printf("%s %s +%#lx: ran\n", names[operation], where, (unsigned long)(at - base));
    } else {
        printf("%s %s +%#lx: signal %d at +%#lx\n", names[operation], where, (unsigned long)(at - base),
               signal, (unsigned long)(faulted - (uintptr_t)base));
    }
}

int main(void) {
    size_t page = (size_t)sysconf(_SC_PAGESIZE);
    uint64_t id;
    __asm__("mrs %0, dczid_el0" : "=r"(id));
    size_t block = (size_t)4 << (id & 0xf);
    printf("zero block %zu bytes, prohibited %d\n", block, (int)((id >> 4) & 1));

    struct sigaction action = {0};
    action.sa_sigaction = on_fault;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGSEGV, &action, 0);
    sigaction(SIGBUS, &action, 0);

    enum { FRESH, TOUCHED, READ_ONLY_FRESH, READ_ONLY_TOUCHED, NOTHING, UNMAPPED, PAGES };
    size_t operations = sizeof names / sizeof *names;
    size_t length = PAGES * operations * page;
    base = mmap(0, length, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (base == MAP_FAILED) return 1;
    for (size_t o = 0; o < operations; o++) {
        char *pages = base + o * PAGES * page;
        memset(pages + TOUCHED * page, 0x5a, page);
        mprotect(pages + READ_ONLY_FRESH * page, 2 * page, PROT_READ);
        printf("read-only touched holds %d\n", pages[READ_ONLY_TOUCHED * page]);
        mprotect(pages + NOTHING * page, page, PROT_NONE);
        munmap(pages + UNMAPPED * page, page);
    }

    for (size_t o = 0; o < operations; o++) {
        enum operation operation = (enum operation)o;
        char *pages = base + o * PAGES * page;
        size_t inside = block + block / 2 + 8;
        attempt("fresh", operation, pages + FRESH * page + inside);
        attempt("touched", operation, pages + TOUCHED * page + inside);
        attempt("read-only fresh", operation, pages + READ_ONLY_FRESH * page);
        attempt("read-only touched", operation, pages + READ_ONLY_TOUCHED * page + inside);
        attempt("allowing nothing", operation, pages + NOTHING * page + inside);
        attempt("unmapped", operation, pages + UNMAPPED * page + inside);
        char *touched = pages + TOUCHED * page;
        size_t zeroed = 0, kept = 0;
        for (size_t at = 0; at < page; at++) {
            if (touched[at] == 0) zeroed++;
            if (touched[at] == 0x5a) kept++;
        }
        printf("%s: touched page has %zu zero bytes from +%#zx, %zu kept; fresh page reads %d\n", names[operation],
               zeroed, (size_t)(memchr(touched, 0, page) ? (char *)memchr(touched, 0, page) - touched : page), kept,
               pages[FRESH * page]);
    }
    return 0;
}
