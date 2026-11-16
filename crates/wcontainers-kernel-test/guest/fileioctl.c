// The generic file requests of ioctl (Linux's do_vfs_ioctl and file_ioctl), the random devices' own
// and tmpfs's inode flags, on every kind of file a process can open, printing each call and what it
// gave.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/fiemap.h>
#include <linux/fs.h>
#include <linux/random.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

#ifndef SYS_memfd_secret
#define SYS_memfd_secret 447
#endif

// Linux's own numbers for requests the C library's headers may lack.
#define RESVSP _IOW('X', 40, struct space)
#define UNRESVSP _IOW('X', 41, struct space)
#define RESVSP64 _IOW('X', 42, struct space)
#define UNRESVSP64 _IOW('X', 43, struct space)
#define ZERO_RANGE _IOW('X', 57, struct space)
#define GETFSUUID _IOR(0x15, 0, struct uuid2)
#define GETFSSYSFSPATH _IOR(0x15, 1, struct sysfs_path)
#define WATCH_QUEUE_SET_SIZE _IO('W', 0x60)
#define WATCH_QUEUE_SET_FILTER _IO('W', 0x61)

struct space {
    int16_t type;
    int16_t whence;
    int64_t start;
    int64_t len;
    int32_t sysid;
    uint32_t pid;
    int32_t pad[4];
};

struct uuid2 {
    uint8_t len;
    uint8_t uuid[16];
};

struct sysfs_path {
    uint8_t len;
    uint8_t name[128];
};

static void *bad = (void *)16;

static long call(int fd, unsigned long request, void *argument) {
    errno = 0;
    return syscall(SYS_ioctl, fd, request, argument);
}

static void show(const char *what, const char *name, long r) {
    printf("%-10s %-22s = %ld (%s)", what, name, r, r < 0 ? strerror(errno) : "-");
}

// Every request that needs no setup, on one open file.
static void requests(const char *what, int fd) {
    long value;
    long r;
    union {
        long words[64];
        struct space space;
        struct fsxattr fsx;
        struct file_clone_range clone;
        struct file_dedupe_range dedupe;
        struct uuid2 uuid;
        struct sysfs_path path;
    } u;

#define ONE(name, request, print)                                                                  \
    do {                                                                                           \
        memset(&u, 0, sizeof(u));                                                                  \
        value = 0x5a5a5a5a5a5a5a5a;                                                                \
        r = call(fd, request, &value);                                                             \
        show(what, name, r);                                                                       \
        if (r == 0 && print) printf(" value %lx", value);                                         \
        printf("\n");                                                                              \
        show(what, name " bad", call(fd, request, bad));                                           \
        printf("\n");                                                                              \
    } while (0)

    ONE("FIONREAD", FIONREAD, 1);
    ONE("FIOQSIZE", FIOQSIZE, 1);
    ONE("FIGETBSZ", FIGETBSZ, 1);
    ONE("FIBMAP", FIBMAP, 1);
    ONE("FIFREEZE", FIFREEZE, 0);
    ONE("FITHAW", FITHAW, 0);
    ONE("GETFLAGS", FS_IOC_GETFLAGS, 1);
    ONE("unknown", 0x1234, 0);
    ONE("unknown f", 0x80086680, 0);
    ONE("TCGETS", TCGETS, 0);
    ONE("RNDGETENTCNT", RNDGETENTCNT, 0);
    ONE("RNDADDTOENTCNT", RNDADDTOENTCNT, 0);
    ONE("RNDADDENTROPY", RNDADDENTROPY, 0);
    ONE("RNDZAPENTCNT", RNDZAPENTCNT, 0);
    ONE("RNDCLEARPOOL", RNDCLEARPOOL, 0);
    ONE("RNDRESEEDCRNG", RNDRESEEDCRNG, 0);
    ONE("RNDGETPOOL", _IOR('R', 0x02, int[2]), 0);
    ONE("R unknown", _IO('R', 0x09), 0);
    ONE("WATCH_SIZE", WATCH_QUEUE_SET_SIZE, 0);
    ONE("WATCH_FILTER", WATCH_QUEUE_SET_FILTER, 0);
    ONE("FIOASYNC 0", FIOASYNC, 0);
    ONE("FIEMAP", FS_IOC_FIEMAP, 0);
    ONE("FICLONE", FICLONE, 0);

    memset(&u, 0, sizeof(u));
    r = call(fd, FS_IOC_FSGETXATTR, &u.fsx);
    show(what, "FSGETXATTR", r);
    printf(" xflags %x extsize %u nextents %u projid %u cow %u\n", u.fsx.fsx_xflags,
           u.fsx.fsx_extsize, u.fsx.fsx_nextents, u.fsx.fsx_projid, u.fsx.fsx_cowextsize);
    show(what, "FSGETXATTR bad", call(fd, FS_IOC_FSGETXATTR, bad));
    printf("\n");

    memset(&u, 0, sizeof(u));
    r = call(fd, GETFSUUID, &u.uuid);
    show(what, "GETFSUUID", r);
    printf(" len %u\n", u.uuid.len);
    memset(&u, 0, sizeof(u));
    r = call(fd, GETFSSYSFSPATH, &u.path);
    show(what, "GETFSSYSFSPATH", r);
    printf(" %s\n", (char *)u.path.name);

    memset(&u, 0, sizeof(u));
    u.clone.src_fd = fd;
    r = call(fd, FICLONERANGE, &u.clone);
    show(what, "FICLONERANGE", r);
    printf("\n");
    show(what, "FICLONERANGE bad", call(fd, FICLONERANGE, bad));
    printf("\n");

    memset(&u, 0, sizeof(u));
    u.dedupe.dest_count = 1;
    u.dedupe.src_length = 1;
    u.dedupe.info[0].dest_fd = fd;
    r = call(fd, FIDEDUPERANGE, &u.dedupe);
    show(what, "FIDEDUPERANGE", r);
    printf(" status %d\n", u.dedupe.info[0].status);
    u.dedupe.dest_count = 200;
    show(what, "FIDEDUPERANGE 200", call(fd, FIDEDUPERANGE, &u.dedupe));
    printf("\n");
    show(what, "FIDEDUPERANGE bad", call(fd, FIDEDUPERANGE, bad));
    printf("\n");

    memset(&u, 0, sizeof(u));
    u.space.whence = SEEK_SET;
    u.space.start = 0;
    u.space.len = 4096;
    show(what, "RESVSP", call(fd, RESVSP, &u.space));
    printf("\n");
    show(what, "RESVSP bad", call(fd, RESVSP, bad));
    printf("\n");
    u.space.whence = 7;
    show(what, "RESVSP whence 7", call(fd, RESVSP64, &u.space));
    printf("\n");
}

static void flags(const char *what, int fd, unsigned value) {
    unsigned long word = value;
    long r = call(fd, FS_IOC_SETFLAGS, &word);
    char name[32];
    snprintf(name, sizeof(name), "SETFLAGS %x", value);
    show(what, name, r);
    word = 0;
    long g = call(fd, FS_IOC_GETFLAGS, &word);
    printf(" then %ld %lx\n", g, word);
}

static void xflags(const char *what, int fd, unsigned value, unsigned extsize) {
    struct fsxattr fsx;
    memset(&fsx, 0, sizeof(fsx));
    fsx.fsx_xflags = value;
    fsx.fsx_extsize = extsize;
    long r = call(fd, FS_IOC_FSSETXATTR, &fsx);
    char name[40];
    snprintf(name, sizeof(name), "FSSETX %x %u", value, extsize);
    show(what, name, r);
    memset(&fsx, 0, sizeof(fsx));
    long g = call(fd, FS_IOC_FSGETXATTR, &fsx);
    unsigned long word = 0;
    call(fd, FS_IOC_GETFLAGS, &word);
    printf(" then %ld %x flags %lx\n", g, fsx.fsx_xflags, word);
}

static void attributes(const char *what, int fd) {
    struct statx stx;
    memset(&stx, 0, sizeof(stx));
    long r = syscall(SYS_statx, fd, "", AT_EMPTY_PATH, STATX_BASIC_STATS, &stx);
    printf("%-10s statx %ld attributes %llx mask %llx\n", what, r,
           (unsigned long long)stx.stx_attributes, (unsigned long long)stx.stx_attributes_mask);
}

int main(void) {
    char buffer[64];
    mkdir("/dev/shm/t", 0755);
    int file = open("/dev/shm/t/f", O_CREAT | O_RDWR, 0644);
    write(file, "0123456789", 10);
    lseek(file, 3, SEEK_SET);
    requests("file", file);
    lseek(file, 30, SEEK_SET);
    long n = 0;
    show("file", "FIONREAD past end", call(file, FIONREAD, &n));
    printf(" %ld\n", n);
    lseek(file, 0, SEEK_SET);
    // Inode flags tmpfs keeps, and those it refuses.
    flags("file", file, 0);
    flags("file", file, FS_NODUMP_FL);
    attributes("file", file);
    flags("file", file, FS_NOATIME_FL | FS_NODUMP_FL);
    flags("file", file, FS_IMMUTABLE_FL);
    flags("file", file, FS_APPEND_FL);
    flags("file", file, FS_SECRM_FL);
    flags("file", file, FS_SYNC_FL);
    flags("file", file, FS_CASEFOLD_FL);
    flags("file", file, FS_DIRSYNC_FL);
    flags("file", file, 0x80000000u);
    flags("file", file, FS_NOATIME_FL);
    xflags("file", file, 0, 0);
    xflags("file", file, FS_XFLAG_NODUMP, 0);
    xflags("file", file, FS_XFLAG_NOATIME | FS_XFLAG_NODUMP, 0);
    xflags("file", file, FS_XFLAG_IMMUTABLE, 0);
    xflags("file", file, FS_XFLAG_SYNC, 0);
    xflags("file", file, FS_XFLAG_DAX, 0);
    xflags("file", file, FS_XFLAG_PROJINHERIT, 0);
    xflags("file", file, FS_XFLAG_EXTSIZE, 0);
    xflags("file", file, FS_XFLAG_EXTSIZE, 4096);
    xflags("file", file, 0, 4096);
    xflags("file", file, FS_XFLAG_NOATIME, 0);
    attributes("file", file);
    // A file with no access time updates keeps its access time across reads.
    struct stat before, after;
    fstat(file, &before);
    struct timespec pause = {0, 20000000};
    nanosleep(&pause, 0);
    pread(file, buffer, 4, 0);
    fstat(file, &after);
    printf("noatime read keeps atime: %d\n",
           before.st_atim.tv_sec == after.st_atim.tv_sec &&
               before.st_atim.tv_nsec == after.st_atim.tv_nsec);
    flags("file", file, 0);
    int readonly = open("/dev/shm/t/f", O_RDONLY);
    flags("ro file", readonly, FS_NODUMP_FL);
    int path = open("/dev/shm/t/f", O_PATH);
    show("path", "FIONREAD", call(path, FIONREAD, &n));
    printf("\n");
    show("closed", "FIONREAD", call(99, FIONREAD, &n));
    printf("\n");
    unsigned long on = 1;
    show("file", "FIOASYNC 1", call(file, FIOASYNC, &on));
    printf(" flags %x\n", fcntl(file, F_GETFL));
    // Another user may not change a file's flags.
    int other = open("/dev/shm/t/g", O_CREAT | O_RDWR, 0666);
    fchown(other, 1000, 1000);
    seteuid(1000);
    flags("other", other, FS_NODUMP_FL);
    flags("other", file, FS_NODUMP_FL);
    seteuid(0);
    // Preallocation through ioctl, as fallocate with KEEP_SIZE.
    struct space space;
    memset(&space, 0, sizeof(space));
    space.whence = SEEK_END;
    space.start = 0;
    space.len = 8192;
    show("file", "RESVSP end", call(file, RESVSP, &space));
    struct stat st;
    fstat(file, &st);
    printf(" size %lld blocks %lld\n", (long long)st.st_size, (long long)st.st_blocks);
    space.whence = SEEK_SET;
    space.start = 4096;
    space.len = 4096;
    show("file", "UNRESVSP", call(file, UNRESVSP64, &space));
    fstat(file, &st);
    printf(" size %lld blocks %lld\n", (long long)st.st_size, (long long)st.st_blocks);
    show("file", "ZERO_RANGE", call(file, ZERO_RANGE, &space));
    printf("\n");
    space.len = 0;
    show("file", "RESVSP len 0", call(file, RESVSP, &space));
    printf("\n");
    show("ro file", "RESVSP", call(readonly, RESVSP, &space));
    printf("\n");
    // Cloning and deduplicating between two tmpfs files, and with a file elsewhere.
    show("file", "FICLONE g", call(file, FICLONE, (void *)(long)other));
    printf("\n");
    show("file", "FICLONE 99", call(file, FICLONE, (void *)99L));
    printf("\n");

    int directory = open("/dev/shm/t", O_RDONLY | O_DIRECTORY);
    requests("dir", directory);
    flags("dir", directory, FS_NODUMP_FL);
    flags("dir", directory, FS_CASEFOLD_FL);
    flags("dir", directory, 0);
    int memfd = syscall(SYS_memfd_create, "m", 0);
    write(memfd, "abc", 3);
    lseek(memfd, 0, SEEK_SET);
    requests("memfd", memfd);
    flags("memfd", memfd, FS_NODUMP_FL);
    int secret = syscall(SYS_memfd_secret, 0);
    if (secret >= 0) {
        ftruncate(secret, 4096);
        requests("secret", secret);
    } else {
        printf("memfd_secret: %s\n", strerror(errno));
    }
    requests("stdin", 0);
    requests("stdout", 1);
    int null = open("/dev/null", O_RDWR);
    requests("null", null);
    int zero = open("/dev/zero", O_RDONLY);
    requests("zero", zero);
    int urandom = open("/dev/urandom", O_RDONLY);
    requests("urandom", urandom);
    int random = open("/dev/random", O_RDWR);
    requests("random", random);
    int count = 0;
    printf("random RNDGETENTCNT %ld", call(random, RNDGETENTCNT, &count));
    printf(" %d\n", count);
    int comm = open("/proc/self/comm", O_RDWR);
    read(comm, buffer, 2);
    requests("comm", comm);
    int cpuinfo = open("/proc/cpuinfo", O_RDONLY);
    requests("cpuinfo", cpuinfo);
    int sysctl = open("/proc/sys/kernel/ostype", O_RDONLY);
    requests("sysctl", sysctl);
    flags("sysctl", sysctl, 0);
    int procdir = open("/proc/self", O_RDONLY | O_DIRECTORY);
    requests("procdir", procdir);
    int pts = open("/dev/pts", O_RDONLY | O_DIRECTORY);
    requests("pts", pts);
    flags("pts", pts, 0);
    return 0;
}
