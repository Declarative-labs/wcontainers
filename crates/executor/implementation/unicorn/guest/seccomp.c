// Calls Docker's default seccomp profile stops, with every argument -1 or with the arguments its rules
// look at, and prints what each returned.

#include <stdio.h>
#include <sys/syscall.h>

static long raw(long n, long a) {
    register long x8 __asm__("x8") = n;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = -1;
    register long x2 __asm__("x2") = -1;
    register long x3 __asm__("x3") = -1;
    register long x4 __asm__("x4") = -1;
    register long x5 __asm__("x5") = -1;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x5) : "memory");
    return x0;
}

int main(void) {
    static const long refused[] = {
        SYS_lookup_dcookie, SYS_umount2, SYS_mount, SYS_pivot_root, SYS_nfsservctl, SYS_quotactl,
        SYS_unshare, SYS_kexec_load, SYS_delete_module, SYS_clock_settime, SYS_settimeofday, SYS_add_key,
        SYS_request_key, SYS_keyctl, SYS_swapon, SYS_mbind, SYS_get_mempolicy, SYS_set_mempolicy,
        SYS_migrate_pages, SYS_move_pages, SYS_perf_event_open, SYS_open_by_handle_at, SYS_setns,
        SYS_kcmp, SYS_bpf, SYS_userfaultfd, SYS_kexec_file_load, 425, 426, 427, 428, 431, 435, 438, 440,
        442, 443, 450, 457, 470, 1000, 244, 400, SYS_clone};
    for (unsigned i = 0; i < sizeof refused / sizeof refused[0]; i++) {
        printf("%ld %ld\n", refused[i], raw(refused[i], -1));
    }
    static const long domains[] = {40, 46, 1000, -1, (1L << 32) | 1};
    for (unsigned i = 0; i < sizeof domains / sizeof domains[0]; i++) {
        printf("socket %lx %ld\n", domains[i], raw(SYS_socket, domains[i]));
    }
    static const long personas[] = {-1, 0x1ffffffffL, 0x100000008L, 0x400000, 0xffffffffL, 0};
    for (unsigned i = 0; i < sizeof personas / sizeof personas[0]; i++) {
        printf("personality %lx %ld\n", personas[i], raw(SYS_personality, personas[i]));
    }
    return 0;
}
