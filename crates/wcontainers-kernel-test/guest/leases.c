// File leases: taking, changing and giving them up, the checks on who may hold which, and
// openers in another process waiting while the holder is told and lets go or comes down.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#define R(call)                                                                                    \
    do {                                                                                           \
        long r_ = (call);                                                                          \
        printf("%-50s %ld %s\n", #call, r_, r_ < 0 ? strerror(errno) : "");                         \
    } while (0)

static volatile sig_atomic_t told, told_fd;
static void tell(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)context;
    told++;
    told_fd = info->si_fd;
}

static void wait_told(void) {
    while (!told)
        usleep(1000);
    printf("told %d fd %d\n", told, told_fd);
    told = 0;
}

// A child opens the file with `flags` and says how it went through a pipe; returns its pid and
// leaves the pipe's reading end in `*from`.
static pid_t opener(int flags, int *from) {
    int pipes[2];
    pipe(pipes);
    pid_t child = fork();
    if (child == 0) {
        // The parent's descriptors would hold its leases' files open here.
        for (int fd = 3; fd < 64; fd++)
            if (fd != pipes[1])
                close(fd);
        int fd = open("leased", flags);
        char word = fd >= 0 ? 'o' : (char)('a' + errno);
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

    // The holder's last close lets a waiting opener on; SIGIO when no signal is chosen.
    int plain = open("leased", O_RDONLY);
    R(fcntl(plain, F_SETLEASE, F_RDLCK));
    child = opener(O_RDWR, &from);
    wait_told();
    close(plain);
    heard(from, child);
    unlink("leased");
    unlink("theirs");
    return 0;
}
