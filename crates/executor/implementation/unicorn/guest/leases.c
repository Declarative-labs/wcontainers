// File leases: taking, changing and giving them up, the checks on who may hold which and on
// which file systems, and openers and truncations in another process waiting while the holder is
// told and lets go or comes down.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <sys/mman.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#define R(call)                                                                                    \
    do {                                                                                           \
        long r_ = (call);                                                                          \
        printf("%-50s %ld %s\n", #call, r_, r_ < 0 ? strerror(errno) : "");                         \
    } while (0)

static volatile sig_atomic_t told, told_fd, told_code, told_io;
static void tell(int signal, siginfo_t *info, void *context) {
    (void)context;
    told++;
    told_fd = info->si_fd;
    told_code = info->si_code;
    told_io = signal == SIGIO;
}

static void wait_told(void) {
    while (!told)
        usleep(1000);
    printf("told %d fd %d code %d sigio %d\n", told, told_fd, told_code, told_io);
    told = 0;
}

// A child opens the file with `flags`, or truncates it by name for flags -1, and says how it
// went through a pipe; returns its pid and leaves the pipe's reading end in `*from`.
static pid_t opener(int flags, int *from) {
    int pipes[2];
    pipe(pipes);
    pid_t child = fork();
    if (child == 0) {
        // The parent's descriptors would hold its leases' files open here.
        for (int fd = 3; fd < 64; fd++)
            if (fd != pipes[1])
                close(fd);
        int done = flags < 0 ? truncate("leased", 3) : open("leased", flags);
        char word = done >= 0 ? 'o' : (char)('a' + errno);
        write(pipes[1], &word, 1);
        _exit(0);
    }
    close(pipes[1]);
    *from = pipes[0];
    return child;
}

static void heard(int from, pid_t child) {
    char word = 0;
    read(from, &word, 1);
    printf("opener: %c\n", word);
    close(from);
    waitpid(child, NULL, 0);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct sigaction action = {.sa_sigaction = tell, .sa_flags = SA_SIGINFO | SA_RESTART};
    sigaction(SIGRTMIN + 2, &action, NULL);
    sigaction(SIGIO, &action, NULL);
    close(open("leased", O_RDWR | O_CREAT | O_TRUNC, 0644));

    int reader = open("leased", O_RDONLY);
    R(fcntl(reader, F_GETLEASE));
    R(fcntl(reader, F_GETOWN) == getpid());
    R(fcntl(reader, F_SETLEASE, F_RDLCK));
    R(fcntl(reader, F_GETOWN) == getpid());
    R(fcntl(reader, F_GETLEASE));
    R(fcntl(reader, F_SETLEASE, 7));
    R(fcntl(reader, F_SETLEASE, F_UNLCK));
    R(fcntl(reader, F_SETLEASE, F_UNLCK));
    R(fcntl(reader, F_SETLEASE, F_WRLCK));
    R(fcntl(reader, F_GETLEASE));
    // Down to read first: an open here would wait out the break time on its own lease.
    R(fcntl(reader, F_SETLEASE, F_RDLCK));
    int other = open("leased", O_RDONLY);
    R(fcntl(other, F_SETLEASE, F_WRLCK));
    R(fcntl(other, F_SETLEASE, F_RDLCK));
    R(fcntl(reader, F_SETLEASE, F_UNLCK));
    R(fcntl(reader, F_SETLEASE, F_WRLCK));
    R(fcntl(reader, F_SETLEASE, F_RDLCK));
    R(fcntl(reader, F_GETLEASE));
    R(fcntl(other, F_GETLEASE));
    R(fcntl(reader, F_SETLEASE, F_UNLCK));
    R(fcntl(other, F_SETLEASE, F_UNLCK));
    close(other);
    int writer = open("leased", O_RDWR);
    R(fcntl(writer, F_SETLEASE, F_RDLCK));
    R(fcntl(reader, F_SETLEASE, F_RDLCK));
    R(fcntl(reader, F_SETLEASE, F_WRLCK));
    R(fcntl(writer, F_SETLEASE, F_WRLCK));
    close(reader);
    R(fcntl(writer, F_SETLEASE, F_WRLCK));
    R(fcntl(writer, F_GETLEASE));
    R(fcntl(writer, F_SETLEASE, F_UNLCK));
    close(writer);

    // Only regular files, and only their owner without CAP_LEASE.
    int directory = open(".", O_RDONLY);
    R(fcntl(directory, F_SETLEASE, F_RDLCK));
    int pipes[2];
    pipe(pipes);
    R(fcntl(pipes[0], F_SETLEASE, F_RDLCK));
    R(fcntl(pipes[0], F_SETLEASE, 7));
    close(open("theirs", O_RDWR | O_CREAT, 0644));
    chown("theirs", 1000, 1000);
    int theirs = open("theirs", O_RDONLY);
    R(fcntl(theirs, F_SETLEASE, F_RDLCK));
    R(fcntl(theirs, F_SETLEASE, F_UNLCK));
    R(fcntl(theirs, F_GETLEASE));
    // Only on file systems that take leases.
    const char *elsewhere[] = {"/proc/self/status", "/sys/kernel/mm/transparent_hugepage/enabled",
                               "/sys/fs/cgroup/cgroup.procs", "/dev/shm/leased"};
    for (unsigned i = 0; i < sizeof elsewhere / sizeof *elsewhere; i++) {
        int fd = open(elsewhere[i], O_RDONLY | O_CREAT, 0644);
        printf("%s: open %d\n", elsewhere[i], fd >= 0);
        R(fcntl(fd, F_SETLEASE, F_RDLCK));
        R(fcntl(fd, F_SETLEASE, F_UNLCK));
        close(fd);
    }
    unlink("/dev/shm/leased");
    // A memory file's own descriptor is not counted as a writer, but is its holder's.
    int memory = memfd_create("leased", 0);
    R(fcntl(memory, F_SETLEASE, F_RDLCK));
    R(fcntl(memory, F_SETLEASE, F_UNLCK));
    R(fcntl(memory, F_SETLEASE, F_WRLCK));
    close(memory);
    int huge = memfd_create("leased", MFD_HUGETLB);
    printf("huge memory file %d\n", huge >= 0);
    R(fcntl(huge, F_SETLEASE, F_RDLCK));
    R(fcntl(huge, F_SETLEASE, F_UNLCK));
    close(huge);

    // A write lease, broken by a reader: the holder is told and must come down to read.
    int held = open("leased", O_RDONLY);
    R(fcntl(held, F_SETSIG, SIGRTMIN + 2));
    R(fcntl(held, F_SETLEASE, F_WRLCK));
    int from;
    pid_t child = opener(O_RDONLY | O_NONBLOCK, &from);
    heard(from, child);
    wait_told();
    R(fcntl(held, F_GETLEASE));
    child = opener(O_RDONLY, &from);
    usleep(100000);
    R(fcntl(held, F_GETLEASE));
    R(fcntl(held, F_SETLEASE, F_RDLCK));
    heard(from, child);
    R(fcntl(held, F_GETLEASE));
    printf("told again %d\n", told);

    // A read lease, broken by a writer: coming down to read is refused while it waits, and
    // letting go lets it on.
    child = opener(O_WRONLY, &from);
    wait_told();
    R(fcntl(held, F_GETLEASE));
    R(fcntl(held, F_SETLEASE, F_RDLCK));
    R(fcntl(held, F_SETLEASE, F_WRLCK));
    R(fcntl(held, F_SETLEASE, F_UNLCK));
    heard(from, child);
    // Letting go takes the file's owner and signal.
    R(fcntl(held, F_GETOWN));
    R(fcntl(held, F_GETSIG));
    struct f_owner_ex owner_ex;
    R(fcntl(held, F_GETOWN_EX, &owner_ex));
    printf("owner type %d pid %d\n", owner_ex.type, owner_ex.pid);

    // The holder's last close lets a waiting opener on; SIGIO when no signal is chosen.
    int plain = open("leased", O_RDONLY);
    R(fcntl(plain, F_SETLEASE, F_RDLCK));
    child = opener(O_RDWR, &from);
    wait_told();
    close(plain);
    heard(from, child);

    // A truncation by name breaks a write lease, as an open for writing does, and waits for it.
    R(fcntl(held, F_SETLEASE, F_WRLCK));
    child = opener(-1, &from);
    wait_told();
    R(fcntl(held, F_GETLEASE));
    struct stat status;
    stat("leased", &status);
    printf("size before letting go %ld\n", (long)status.st_size);
    R(fcntl(held, F_SETLEASE, F_UNLCK));
    heard(from, child);
    stat("leased", &status);
    printf("size after %ld\n", (long)status.st_size);
    unlink("leased");
    unlink("theirs");
    return 0;
}
