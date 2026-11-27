// Every register of the identification space Linux answers for a program (op0 3, op1 0, CRn 0,
// CRm 0 to 7, each op2), read with mrs, and the two hardware capability words: one line each, the
// value in hexadecimal or "undefined" where the read is an undefined instruction.
#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/auxv.h>

static sigjmp_buf undefined;

static void on_undefined(int signal) {
    (void)signal;
    siglongjmp(undefined, 1);
}

#define READER(crm, op2)                                                  \
    static uint64_t read_##crm##_##op2(void) {                            \
        uint64_t value;                                                   \
        __asm__ volatile("mrs %0, S3_0_C0_C" #crm "_" #op2 : "=r"(value)); \
        return value;                                                     \
    }
#define ROW(crm) \
    READER(crm, 0) READER(crm, 1) READER(crm, 2) READER(crm, 3) \
    READER(crm, 4) READER(crm, 5) READER(crm, 6) READER(crm, 7)
ROW(0) ROW(1) ROW(2) ROW(3) ROW(4) ROW(5) ROW(6) ROW(7)

#define ENTRY(crm, op2) {crm, op2, read_##crm##_##op2}
#define ENTRIES(crm)                                              \
    ENTRY(crm, 0), ENTRY(crm, 1), ENTRY(crm, 2), ENTRY(crm, 3), \
    ENTRY(crm, 4), ENTRY(crm, 5), ENTRY(crm, 6), ENTRY(crm, 7)

static const struct {
    int crm, op2;
    uint64_t (*read)(void);
} registers[] = {ENTRIES(0), ENTRIES(1), ENTRIES(2), ENTRIES(3),
                 ENTRIES(4), ENTRIES(5), ENTRIES(6), ENTRIES(7)};

int main(void) {
    struct sigaction action = {0};
    action.sa_handler = on_undefined;
    action.sa_flags = SA_NODEFER;
    sigaction(SIGILL, &action, 0);
    for (unsigned i = 0; i < sizeof registers / sizeof registers[0]; i++) {
        printf("S3_0_C0_C%d_%d ", registers[i].crm, registers[i].op2);
        fflush(stdout);
        if (sigsetjmp(undefined, 1) == 0) {
            printf("%016llx\n", (unsigned long long)registers[i].read());
        } else {
            printf("undefined\n");
        }
    }
    printf("AT_HWCAP %016llx\n", (unsigned long long)getauxval(AT_HWCAP));
    printf("AT_HWCAP2 %016llx\n", (unsigned long long)getauxval(AT_HWCAP2));
    return 0;
}
