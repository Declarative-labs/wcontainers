// Programs for the embedder's hooks and memory primitives. A fixed buffer lets the test host find
// what it watches without being told.
//   lifecycle: a thread, then a forked child that runs /bin/true, then an exit.
//   calls: in a child each, a system call writes into the buffer; each prints what it got.
//   pages: in a child, writes known pages of the buffer, one of them by read().
//   outgrow: maps 64 MiB at a time until refused; prints how many and the error.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/random.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/uio.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define BUFFER ((char *)0x37000000)
#define PAGE 4096
#define PAGES 8

static void *thread_main(void *argument) { return argument; }

static int lifecycle(void) {
    pthread_t thread;
    pthread_create(&thread, NULL, thread_main, NULL);
    pthread_join(thread, NULL);
    pid_t child = fork();
    if (child == 0) {
        execl("/bin/true", "true", (char *)NULL);
        _exit(127);
    }
    int status;
    waitpid(child, &status, 0);
    printf("child %d\n", WEXITSTATUS(status));
    return 0;
}

static void handled(int signal) {
    static const char text[] = "handled\n";
    (void)signal;
    write(1, text, sizeof text - 1);
}

// One call writing into the buffer at page `page`, run in a child.
static void call(int which) {
    char *at = BUFFER + PAGE + 64;
    int fds[2];
    switch (which) {
    case 0: {
        pipe(fds);
        write(fds[1], "hello pipe", 10);
        int n = read(fds[0], at, 64);
        printf("read %d %.*s\n", n, n, at);
        break;
    }
    case 1: {
        pipe(fds);
        write(fds[1], "abcdefghij", 10);
        struct iovec iov[2] = {{at, 4}, {BUFFER + 2 * PAGE + 8, 6}};
        int n = readv(fds[0], iov, 2);
        printf("readv %d %.4s%.6s\n", n, at, BUFFER + 2 * PAGE + 8);
        break;
    }
    case 2: {
        socketpair(AF_UNIX, SOCK_DGRAM, 0, fds);
        send(fds[1], "datagram", 8, 0);
        struct iovec iov = {at, 64};
        struct msghdr message = {.msg_iov = &iov, .msg_iovlen = 1};
        int n = recvmsg(fds[0], &message, 0);
        printf("recvmsg %d %.*s\n", n, n, at);
        break;
    }
    case 3: {
        stack_t stack = {.ss_sp = BUFFER, .ss_size = PAGES * PAGE};
        sigaltstack(&stack, NULL);
        struct sigaction action = {.sa_handler = handled, .sa_flags = SA_ONSTACK};
        sigaction(SIGUSR1, &action, NULL);
        raise(SIGUSR1);
        printf("frame done\n");
        break;
    }
    case 4: {
        int n = pread(open("/etc/hostname", 0), at, 64, 0);
        printf("pread %d\n", n);
        break;
    }
    case 5:
        printf("getcwd %s\n", getcwd(at, 64) ? at : "failed");
        break;
    case 6:
        printf("uname %d %s\n", uname((struct utsname *)at), ((struct utsname *)at)->sysname);
        break;
    case 7:
        printf("fstat %d\n", fstat(0, (struct stat *)at));
        break;
    case 8:
        printf("pipe2 %d %d %d\n", pipe2((int *)at, 0), ((int *)at)[0], ((int *)at)[1]);
        break;
    case 9: {
        pid_t child = fork();
        if (child == 0) _exit(5);
        printf("wait4 %d %d\n", wait4(child, (int *)at, 0, NULL) == child, WEXITSTATUS(*(int *)at));
        break;
    }
    case 10:
        printf("getrandom %d\n", (int)getrandom(at, 16, 0));
        break;
    case 11:
        printf("clock_gettime %d\n", clock_gettime(CLOCK_MONOTONIC, (struct timespec *)at));
        break;
    }
    fflush(stdout);
}

#define CALLS 12

static int calls(void) {
    for (int which = 0; which < CALLS; which++) {
        pid_t child = fork();
        if (child == 0) {
            call(which);
            _exit(0);
        }
        waitpid(child, NULL, 0);
    }
    return 0;
}

// Writes pages 0, 2 and 5 of the buffer itself and page 7 by read().
static int pages(void) {
    pid_t child = fork();
    if (child == 0) {
        BUFFER[0] = 1;
        BUFFER[2 * PAGE + 100] = 2;
        BUFFER[5 * PAGE + PAGE - 1] = 3;
        int fds[2];
        pipe(fds);
        write(fds[1], "page", 4);
        read(fds[0], BUFFER + 7 * PAGE + 10, 4);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    return 0;
}

static int outgrow(void) {
    int count = 0;
    while (mmap(NULL, 64 << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0) != MAP_FAILED)
        count++;
    printf("outgrown %d %s\n", count, errno == ENOMEM ? "ENOMEM" : "other");
    return 0;
}

int main(int argc, char **argv) {
    if (mmap(BUFFER, PAGES * PAGE, PROT_READ | PROT_WRITE, MAP_FIXED | MAP_PRIVATE | MAP_ANONYMOUS, -1, 0) !=
        BUFFER)
        return 1;
    memset(BUFFER, 'x', PAGES * PAGE);
    if (argc < 2) return 2;
    if (!strcmp(argv[1], "lifecycle")) return lifecycle();
    if (!strcmp(argv[1], "calls")) return calls();
    if (!strcmp(argv[1], "pages")) return pages();
    if (!strcmp(argv[1], "outgrow")) return outgrow();
    return 2;
}
