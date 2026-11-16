// System V shared memory: shmget's keys, sizes and flags, the identifiers it hands out, shmat's
// addresses and flags, what a segment's status shows as processes attach, fork, split, detach, run a
// new program and end, removal while attached, owners and permissions, locking, and every shmctl
// command. Times are printed as unset, or as set within a few seconds of now.

#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define PAGE 4096L

// Linux's own layouts for 64-bit ARM (`struct shmid64_ds`, `struct shminfo64`, `struct shm_info`).
struct perm64 {
    int key;
    unsigned uid, gid, cuid, cgid, mode;
    unsigned short seq, pad;
    unsigned long unused1, unused2;
};
struct shmid64 {
    struct perm64 perm;
    unsigned long segsz;
    long atime, dtime, ctime;
    int cpid, lpid;
    unsigned long nattch, unused4, unused5;
};
struct shminfo64 {
    unsigned long shmmax, shmmin, shmmni, shmseg, shmall, unused[4];
};
struct shminfo {
    int used_ids;
    unsigned long shm_tot, shm_rss, shm_swp, swap_attempts, swap_successes;
};

enum { RMID = 0, SET = 1, STAT = 2, INFO = 3, LOCK = 11, UNLOCK = 12, SSTAT = 13, SINFO = 14, SSTAT_ANY = 15 };
enum { CREAT = 01000, EXCL = 02000, RDONLY = 010000, RND = 020000, REMAP = 040000, EXEC = 0100000 };
enum { HUGETLB = 04000, NORESERVE = 010000 };

static pid_t self;

static long get(long key, unsigned long size, long flags) {
    return syscall(SYS_shmget, key, size, flags);
}

static long ctl(long id, long cmd, void *buffer) {
    return syscall(SYS_shmctl, id, cmd, buffer);
}

static long at(long id, unsigned long address, long flags) {
    return syscall(SYS_shmat, id, address, flags);
}

static long dt(unsigned long address) {
    return syscall(SYS_shmdt, address);
}

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static const char *when(long seconds) {
    if (seconds == 0) {
        return "unset";
    }
    long now = time(NULL);
    return seconds <= now && seconds > now - 30 ? "now" : "other";
}

static const char *who(int pid) {
    if (pid == 0) {
        return "none";
    }
    return pid == self ? "self" : "other";
}

static void status(const char *what, long id) {
    struct shmid64 ds;
    memset(&ds, 0xee, sizeof ds);
    long got = ctl(id, STAT, &ds);
    if (got < 0) {
        result(what, got);
        return;
    }
    printf("%s: key %x uid %u gid %u cuid %u cgid %u mode %o segsz %lu atime %s dtime %s ctime %s cpid %s "
           "lpid %s nattch %lu unused %lx %lx %lx %lx pad %x\n",
           what, ds.perm.key, ds.perm.uid, ds.perm.gid, ds.perm.cuid, ds.perm.cgid, ds.perm.mode, ds.segsz,
           when(ds.atime), when(ds.dtime), when(ds.ctime), who(ds.cpid), who(ds.lpid), ds.nattch,
           ds.perm.unused1, ds.perm.unused2, ds.unused4, ds.unused5, ds.perm.pad);
}

static int last_pid(long id) {
    struct shmid64 ds;
    return ctl(id, STAT, &ds) < 0 ? -1 : ds.lpid;
}

static void attachments(const char *what, long id) {
    struct shmid64 ds;
    long got = ctl(id, STAT, &ds);
    printf("%s: %ld nattch %lu\n", what, got, got < 0 ? 0 : ds.nattch);
}

// The mappings of segments in /proc/self/maps: their pages, permissions, offset, inode as against
// the identifier, and name.
static void maps(const char *what, long id) {
    char line[512];
    FILE *file = fopen("/proc/self/maps", "r");
    printf("%s:\n", what);
    while (fgets(line, sizeof line, file)) {
        char permissions[8], device[16], path[256] = "", deleted[32] = "";
        unsigned long start, end, offset, inode;
        if (sscanf(line, "%lx-%lx %7s %lx %15s %lu %255s %31s", &start, &end, permissions, &offset, device,
                   &inode, path, deleted) >= 7 &&
            strncmp(path, "/SYSV", 5) == 0) {
            printf("  %lu pages %s %lx %s inode %s %s %s\n", (end - start) / PAGE, permissions, offset, device,
                   (long)inode == id ? "id" : "other", path, deleted);
        }
    }
    fclose(file);
}

static void information(const char *what) {
    struct shminfo64 info;
    memset(&info, 0xee, sizeof info);
    long got = ctl(0, INFO, &info);
    printf("%s info: %ld max %lx min %lu mni %lu seg %lu all %lx unused %lx\n", what, got, info.shmmax,
           info.shmmin, info.shmmni, info.shmseg, info.shmall, info.unused[0]);
    struct shminfo used;
    memset(&used, 0xee, sizeof used);
    got = ctl(0, SINFO, &used);
    printf("%s shm_info: %ld used %d tot %lu swp %lu attempts %lu successes %lu\n", what, got, used.used_ids,
           used.shm_tot, used.shm_swp, used.swap_attempts, used.swap_successes);
}

// The identifiers handed out as segments are made and removed.
static void identifiers(void) {
    long ids[8];
    for (int i = 0; i < 8; i++) {
        ids[i] = get(0, 100, 0600);
    }
    printf("ids:");
    for (int i = 0; i < 8; i++) {
        printf(" %ld", ids[i]);
    }
    printf("\n");
    ctl(ids[2], RMID, 0);
    ctl(ids[5], RMID, 0);
    long again = get(0, 100, 0600);
    printf("after removing two: %ld\n", again);
    struct shmid64 ds;
    result("stat by index", ctl(again & 0x7fff, SSTAT, &ds));
    result("stat any by index", ctl(again & 0x7fff, SSTAT_ANY, &ds));
    result("stat by removed index", ctl(ids[2] & 0x7fff, SSTAT, &ds));
    result("ipc_stat with old sequence", ctl(again + 32768, STAT, &ds));
    information("with nine");
    for (int i = 0; i < 8; i++) {
        ctl(ids[i], RMID, 0);
    }
    ctl(again, RMID, 0);
    // Enough to wrap past the cycle's first 64 slots.
    long first = -1, last = -1;
    for (int i = 0; i < 70; i++) {
        long id = get(0, 1, 0600);
        if (first < 0) {
            first = id;
        }
        last = id;
        ctl(id, RMID, 0);
    }
    printf("cycle: first %ld last %ld\n", first, last);
}

static void keys(void) {
    result("missing key", get(0x1234, 4096, 0600));
    long b = get(0x1234, 4096, CREAT | 0640);
    result("create key", b);
    result("exclusive again", get(0x1234, 4096, CREAT | EXCL | 0640));
    result("larger", get(0x1234, 8192, 0));
    result("smaller", get(0x1234, 100, 0) == b ? 0 : -1);
    result("size 0 existing", get(0x1234, 0, 0) == b ? 0 : -1);
    result("no flags", get(0x1234, 4096, 0) == b ? 0 : -1);
    result("write only asked", get(0x1234, 4096, 0222) == b ? 0 : -1);
    status("key status", b);
    // Removing a segment that is still attached hides its key.
    long address = at(b, 0, 0);
    result("attach key", address < 0 ? address : 0);
    result("remove attached", ctl(b, RMID, 0));
    status("removed attached", b);
    result("key after removal", get(0x1234, 4096, 0640));
    long c = get(0x1234, 4096, CREAT | 0640);
    result("key made again", c == b ? -1 : 0);
    long again = at(b, 0, RDONLY);
    result("attach removed", again < 0 ? again : 0);
    attachments("removed twice attached", b);
    dt(again);
    dt(address);
    result("gone at last detach", ctl(b, STAT, &(struct shmid64){0}));
    ctl(c, RMID, 0);
}

static void sizes(void) {
    result("size 0", get(0, 0, 0600));
    result("size max", get(0, -1UL, 0600));
    result("size over max", get(0, -1UL - (1UL << 24) + 1, 0600));
    result("size max allowed", get(0, -1UL - (1UL << 24), 0600));
    long large = get(0, 1UL << 40, 0600);
    result("size 2^40", large < 0 ? large : 0);
    status("size 2^40", large);
    ctl(large, RMID, 0);
    long id = get(0, 1, 0600);
    status("size 1", id);
    ctl(id, RMID, 0);
    long huge = get(0, 2 << 20, HUGETLB | 0600);
    result("huge pages", huge < 0 ? huge : 0);
    if (huge >= 0) {
        ctl(huge, RMID, 0);
    }
}

static void attaching(void) {
    long id = get(0, 10000, CREAT | 0600);
    status("fresh", id);
    unsigned char *p = (unsigned char *)at(id, 0, 0);
    result("attach", (long)p < 0 ? (long)p : 0);
    status("attached", id);
    maps("maps attached", id);
    memcpy(p, "segment", 8);
    p[10000] = 1;
    p[3 * PAGE - 1] = 2;
    unsigned char *q = (unsigned char *)at(id, 0, RDONLY);
    printf("read only sees: %s %d %d\n", (char *)q, q[10000], q[3 * PAGE - 1]);
    result("write protect lift", mprotect(q, PAGE, PROT_READ | PROT_WRITE));
    result("execute added", mprotect(q, PAGE, PROT_READ | PROT_EXEC));
    attachments("twice", id);
    maps("maps twice", id);
    result("misaligned", at(id, (unsigned long)q + 100, 0));
    result("misaligned remap", at(id, (unsigned long)q + 100, REMAP));
    long rounded = at(id, (unsigned long)q + 100, RND | REMAP);
    result("rounded remap lands", rounded == (long)q ? 0 : -1);
    printf("remapped sees: %s\n", (char *)q);
    result("rounded below a page with remap", at(id, 100, RND | REMAP));
    result("remap with no address", at(id, 0, REMAP));
    result("over a mapping", at(id, (unsigned long)p, 0));
    result("past the top", at(id, -PAGE, 0));
    long executable = at(id, 0, RDONLY | EXEC);
    result("executable", executable < 0 ? executable : 0);
    maps("maps with executable", id);
    attachments("three", id);
    // Pieces: protecting part of an attachment splits it, unmapping part of it splits it again.
    result("protect middle", mprotect(p + PAGE, PAGE, PROT_READ));
    attachments("split in three", id);
    result("unmap last page", munmap(p + 2 * PAGE, PAGE));
    attachments("one unmapped", id);
    result("detach inside", dt((unsigned long)p + PAGE));
    result("detach misaligned", dt((unsigned long)p + 1));
    result("detach nothing", dt(PAGE * 16));
    result("detach pieces", dt((unsigned long)p));
    attachments("pieces gone", id);
    status("after detach", id);
    result("detach again", dt((unsigned long)p));
    dt((unsigned long)q);
    dt(executable);
    status("all detached", id);
    result("attach wrong sequence", at(id + 32768, 0, 0));
    result("attach negative", at(-1, 0, 0));
    ctl(id, RMID, 0);
}

static void families(const char *program) {
    long id = get(0, PAGE, 0600);
    char *p = (char *)at(id, 0, 0);
    strcpy(p, "parent");
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        self = getpid();
        attachments("child sees", id);
        printf("child last pid: %s\n", last_pid(id) == getppid() ? "parent" : "other");
        strcpy(p, "child");
        fflush(stdout);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    printf("parent reads: %s\n", p);
    attachments("after child", id);
    printf("last pid after child: %s\n", last_pid(id) == child ? "child" : "other");
    // A child that starts a program loses its attachments.
    fflush(stdout);
    child = fork();
    if (child == 0) {
        char number[32];
        snprintf(number, sizeof number, "%ld", id);
        execl(program, program, "exec", number, (char *)0);
        _exit(1);
    }
    waitpid(child, NULL, 0);
    attachments("after exec child", id);
    // vfork borrows memory and attaches nothing.
    child = vfork();
    if (child == 0) {
        struct shmid64 ds;
        ctl(id, STAT, &ds);
        _exit((int)ds.nattch);
    }
    int wstatus;
    waitpid(child, &wstatus, 0);
    printf("vfork child saw nattch %d\n", WEXITSTATUS(wstatus));
    dt((unsigned long)p);
    ctl(id, RMID, 0);
}

static void owners(void) {
    long id = get(0x777, PAGE, CREAT | 0600);
    struct shmid64 ds;
    ctl(id, STAT, &ds);
    ds.perm.uid = 1000;
    ds.perm.gid = 1001;
    ds.perm.mode = 07644;
    ds.perm.key = 5;
    ds.segsz = 1;
    result("set", ctl(id, SET, &ds));
    status("after set", id);
    ds.perm.uid = -1;
    result("set uid -1", ctl(id, SET, &ds));
    ds.perm.uid = 1000;
    ds.perm.gid = -1;
    result("set gid -1", ctl(id, SET, &ds));
    result("set from nowhere", ctl(id, SET, (void *)16));
    result("set bad id from nowhere", ctl(id + 1, SET, (void *)16));
    result("stat to nowhere", ctl(id, STAT, (void *)16));
    result("info to nowhere", ctl(id, INFO, (void *)16));
    result("bad command", ctl(id, 99, &ds));
    result("negative command", ctl(id, -1, &ds));
    result("negative id", ctl(-1, STAT, &ds));
    result("ipc_64 command", ctl(id, STAT | 0x100, &ds));
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        self = getpid();
        // Another user, not the owner and not the creator, in the segment's group or not.
        setresgid(1002, 1002, 1002);
        setgroups(0, NULL);
        setresuid(2000, 2000, 2000);
        status("other stat", id);
        result("other attach", at(id, 0, 0));
        long ro = at(id, 0, RDONLY);
        result("other read only attach", ro < 0 ? ro : 0);
        result("other get", get(0x777, PAGE, 0) == id ? 0 : -1);
        result("other get for writing", get(0x777, PAGE, 0200));
        result("other set", ctl(id, SET, &ds));
        result("other lock", ctl(id, LOCK, 0));
        result("other remove", ctl(id, RMID, 0));
        long mine = get(0, PAGE, 0400);
        status("own read only", mine);
        result("own attach for writing", at(mine, 0, 0));
        result("own lock", ctl(mine, LOCK, 0));
        status("own locked", mine);
        result("own unlock", ctl(mine, UNLOCK, 0));
        struct rlimit none = {0, 0};
        setrlimit(RLIMIT_MEMLOCK, &none);
        result("own lock with no limit", ctl(mine, LOCK, 0));
        result("own unlock unlocked", ctl(mine, UNLOCK, 0));
        ctl(mine, RMID, 0);
        fflush(stdout);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    // As the creator but no longer the owner.
    child = fork();
    if (child == 0) {
        self = getpid();
        setresuid(0, 0, 0);
        result("creator lock", ctl(id, LOCK, 0));
        status("locked", id);
        result("lock twice", ctl(id, LOCK, 0));
        result("unlock", ctl(id, UNLOCK, 0));
        result("unlock twice", ctl(id, UNLOCK, 0));
        struct rlimit small = {PAGE, PAGE};
        setrlimit(RLIMIT_MEMLOCK, &small);
        long big = get(0, 2 * PAGE, 0600);
        result("lock over the limit", ctl(big, LOCK, 0));
        ctl(big, RMID, 0);
        result("creator remove", ctl(id, RMID, 0));
        fflush(stdout);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    result("removed by creator", ctl(id, STAT, &ds));
}

int main(int argc, char **argv) {
    self = getpid();
    if (argc == 3) {
        long id = atol(argv[2]);
        attachments("exec child sees", id);
        printf("exec child last pid: %s\n", last_pid(id) == getpid() ? "self" : "other");
        return 0;
    }
    information("empty");
    sizes();
    keys();
    identifiers();
    attaching();
    families(argv[0]);
    owners();
    information("end");
    return 0;
}
