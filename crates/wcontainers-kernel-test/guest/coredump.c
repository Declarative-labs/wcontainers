// Core dumps: a child killed by a signal that dumps core, under several core size limits, with
// other threads, with a umask the file cannot keep, over a directory named core, and by a fault;
// the wait status and what the core file holds. The notes of pointer authentication (0x406 to
// 0x408 and 0x40a) are left out: the reference's CPU has it and the described machine does not.
#define _GNU_SOURCE
#include <elf.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static char marker[8192] = "marker-bytes-here";

static void *sleeper(void *arg) {
    (void)arg;
    for (;;) pause();
}

static void describe(pid_t child) {
    int fd = open("core", O_RDONLY);
    if (fd < 0) {
        printf("  no core file\n");
        return;
    }
    struct stat st;
    fstat(fd, &st);
    if (!S_ISREG(st.st_mode)) {
        printf("  core is not a file\n");
        close(fd);
        return;
    }
    char *core = malloc(st.st_size + 1);
    long got = 0;
    while (got < st.st_size) {
        long n = read(fd, core + got, st.st_size - got);
        if (n <= 0) break;
        got += n;
    }
    close(fd);
    printf("  mode %o nlink %ld uid %d empty %d\n", st.st_mode & 07777, (long)st.st_nlink, st.st_uid,
           st.st_size == 0);
    if (st.st_size < (long)sizeof(Elf64_Ehdr)) return;
    Elf64_Ehdr *e = (Elf64_Ehdr *)core;
    printf("  ident %.4s %d %d %d %d type %d machine %d version %d entry %lx phoff %lu shoff %lu flags %x"
           " ehsize %d phentsize %d shentsize %d shnum %d shstrndx %d\n",
           e->e_ident, e->e_ident[4], e->e_ident[5], e->e_ident[6], e->e_ident[7], e->e_type, e->e_machine,
           e->e_version, (long)e->e_entry, (long)e->e_phoff, (long)e->e_shoff, e->e_flags, e->e_ehsize,
           e->e_phentsize, e->e_shentsize, e->e_shnum, e->e_shstrndx);
    if (st.st_size < (long)(e->e_phoff + e->e_phnum * sizeof(Elf64_Phdr))) {
        printf("  cut before the program headers\n");
        return;
    }
    Elf64_Phdr *ph = (Elf64_Phdr *)(core + e->e_phoff);
    int loads = 0, marker_found = 0, anonymous_found = 0, ordered = 1;
    unsigned long last = 0;
    for (int i = 0; i < e->e_phnum; i++) {
        if (ph[i].p_type == PT_NOTE) {
            printf("  notes right after the headers %d vaddr %lx memsz %lu flags %u align %lu\n",
                   ph[i].p_offset == e->e_phoff + e->e_phnum * sizeof(Elf64_Phdr), (long)ph[i].p_vaddr,
                   (long)ph[i].p_memsz, ph[i].p_flags, (long)ph[i].p_align);
            if (ph[i].p_offset + ph[i].p_filesz > (unsigned long)st.st_size) {
                printf("  notes cut\n");
                continue;
            }
            char *n = core + ph[i].p_offset, *end = n + ph[i].p_filesz;
            while (n < end) {
                Elf64_Nhdr *h = (Elf64_Nhdr *)n;
                char *name = n + sizeof(*h);
                char *desc = name + ((h->n_namesz + 3) & ~3);
                n = desc + ((h->n_descsz + 3) & ~3);
                if (h->n_type == 0x406 || h->n_type == 0x407 || h->n_type == 0x408 || h->n_type == 0x40a)
                    continue;
                // NT_FILE's size is left out: it holds the files' names, and the reference names a
                // file Docker's mount brought in by where its host shares it.
                printf("  note %s %x", name, h->n_type);
                if (h->n_type != NT_FILE) printf(" size %u", h->n_descsz);
                if (h->n_type == NT_PRSTATUS)
                    printf(" signal %d %d %d cursig %d pending %lx blocked %lx the child %d parent %d group %d"
                           " session %d fpvalid %d",
                           *(int *)desc, *(int *)(desc + 4), *(int *)(desc + 8), *(short *)(desc + 12),
                           *(long *)(desc + 16), *(long *)(desc + 24), *(int *)(desc + 32) == child,
                           *(int *)(desc + 36) == getpid(), *(int *)(desc + 40) == getpgrp(),
                           *(int *)(desc + 44) == getsid(0), *(int *)(desc + 384));
                if (h->n_type == NT_PRPSINFO)
                    printf(" state %d %c zombie %d nice %d flag %lx uid %d gid %d the child %d parent %d name %.16s"
                           " arguments %.80s",
                           desc[0], desc[1], desc[2], desc[3], *(long *)(desc + 8), *(int *)(desc + 16),
                           *(int *)(desc + 20), *(int *)(desc + 24) == child, *(int *)(desc + 28) == getpid(),
                           desc + 40, desc + 56);
                if (h->n_type == NT_SIGINFO)
                    printf(" signal %d errno %d code %d", *(int *)desc, *(int *)(desc + 4), *(int *)(desc + 8));
                if (h->n_type == NT_AUXV) {
                    printf(" keys");
                    for (unsigned k = 0; k < h->n_descsz; k += 16) printf(" %ld", *(long *)(desc + k));
                }
                if (h->n_type == NT_FILE) {
                    long count = *(long *)desc;
                    printf(" count %ld page %ld", count, *(long *)(desc + 8));
                    for (long k = 0; k < count; k++) {
                        long *r = (long *)(desc + 16 + k * 24);
                        printf(" [%lx %ld]", r[1] - r[0], r[2]);
                    }
                }
                if (h->n_type == NT_PRFPREG)
                    printf(" fpsr %x fpcr %x", *(unsigned *)(desc + 512), *(unsigned *)(desc + 516));
                if (h->n_type == NT_ARM_TLS) printf(" second %lx", *(long *)(desc + 8));
                if (h->n_type == NT_ARM_TAGGED_ADDR_CTRL) printf(" value %lx", *(long *)desc);
                printf("\n");
            }
        } else if (ph[i].p_type == PT_LOAD) {
            loads++;
            if (ph[i].p_offset < last || ph[i].p_offset % 4096 || ph[i].p_align != 4096 || ph[i].p_paddr ||
                ph[i].p_filesz > ph[i].p_memsz)
                ordered = 0;
            last = ph[i].p_offset + ph[i].p_filesz;
            unsigned long m = (unsigned long)marker;
            if (m >= ph[i].p_vaddr && m < ph[i].p_vaddr + ph[i].p_filesz &&
                ph[i].p_offset + (m - ph[i].p_vaddr) + 17 <= (unsigned long)st.st_size)
                marker_found = !memcmp(core + ph[i].p_offset + (m - ph[i].p_vaddr), marker, 17);
            if (ph[i].p_filesz >= 15 && ph[i].p_offset + 15 <= (unsigned long)st.st_size &&
                !memcmp(core + ph[i].p_offset, "anonymous-bytes", 15))
                anonymous_found = 1;
        } else {
            printf("  program header type %x\n", ph[i].p_type);
        }
    }
    printf("  loads %d ordered %d marker %d anonymous %d whole %d\n", loads > 3, ordered, marker_found,
           anonymous_found, last == (unsigned long)st.st_size);
}

static void dump(const char *what, long limit, int threads, int fault) {
    printf("%s\n", what);
    struct rlimit rl = {limit < 0 ? RLIM_INFINITY : (rlim_t)limit, RLIM_INFINITY};
    setrlimit(RLIMIT_CORE, &rl);
    pid_t child = fork();
    if (child == 0) {
        for (int i = 0; i < threads; i++) {
            pthread_t t;
            pthread_create(&t, 0, sleeper, 0);
        }
        char *anonymous = mmap(0, 65536, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        strcpy(anonymous, "anonymous-bytes");
        if (threads) usleep(100000);
        if (fault) *(volatile int *)16 = 1;
        abort();
    }
    int status;
    waitpid(child, &status, 0);
    printf("  signalled %d signal %d core %d\n", WIFSIGNALED(status), WTERMSIG(status), WCOREDUMP(status) ? 1 : 0);
    describe(child);
    unlink("core");
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (chdir("/tmp")) return 1;
    unlink("core");
    dump("no limit", -1, 0, 0);
    dump("a page", 4096, 0, 0);
    dump("half a megabyte", 512 * 1024, 0, 0);
    dump("under a page", 4095, 0, 0);
    dump("none", 0, 0, 0);
    dump("two more threads", -1, 2, 0);
    dump("a fault", -1, 0, 1);
    umask(0277);
    dump("a umask the file cannot keep", -1, 0, 0);
    umask(022);
    mkdir("core", 0755);
    dump("a directory named core", -1, 0, 0);
    rmdir("core");
    // A stopped process takes a signal that dumps core only once continued.
    struct rlimit rl = {RLIM_INFINITY, RLIM_INFINITY};
    setrlimit(RLIMIT_CORE, &rl);
    pid_t child = fork();
    if (child == 0) {
        for (;;) pause();
    }
    kill(child, SIGSTOP);
    int status;
    waitpid(child, &status, WUNTRACED);
    kill(child, SIGQUIT);
    usleep(200000);
    printf("stopped: still there %d\n", waitpid(child, &status, WNOHANG) == 0);
    kill(child, SIGCONT);
    waitpid(child, &status, 0);
    printf("continued: signal %d core %d\n", WTERMSIG(status), WCOREDUMP(status) ? 1 : 0);
    unlink("core");
    return 0;
}
