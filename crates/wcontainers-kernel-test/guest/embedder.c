// Programs for the embedder's hooks and memory primitives, run as `embedder MODE BUFFER PAGES
// [ARGUMENT...]`: BUFFER is where the test host expects a buffer of PAGES pages, mapped first.
//   lifecycle: a thread, then a forked child that runs /bin/true, then an exit.
//   paths: the other ways processes and numbers come and go, one after another.
//   calls: prints how many calls it has, then in a child each, one call writes into the buffer.
//   pages: in a child, writes known pages of the buffer, one of them by read(), and maps new
//     memory over two others.
//   span: in a child, an 8-byte store that ends just inside the buffer's second page.
//   outgrow BYTES: maps BYTES at a time until refused; prints how many and the error.
//   fixed ADDRESS: maps a page at ADDRESS with MAP_FIXED, then with MAP_FIXED_NOREPLACE.
//   brk BYTES: unmaps the buffer, then grows the heap by BYTES with brk.
//   remap ADDRESS COUNT: in a child, moves the buffer's first COUNT pages to ADDRESS, sums them,
//     writes the last moved page, drops the next page of the buffer and prints its first byte.
//   orphan: forks a child that waits for a signal, then ends with status 3.
//   shared ADDRESS: maps a page of a file read-only and shared at ADDRESS, forks a child that
//     ends at once, then prints the file's first byte and the mapping's.
//   attributes: prints how many changes it has, then in a child each: writes the buffer's first
//     page, changes the attributes of its first three pages and back, reads the second page and
//     writes the second and the third.
//   spend BYTES: in a child, maps BYTES, spends some CPU time and prints the process's, in
//     seconds and nanoseconds.
//   kernelwrites LENGTH: in a child, by raw calls, reads LENGTH bytes from a pipe into the buffer's
//     second page and the monotonic time into its third; then, in its fourth, has the kernel
//     store a vfork child's number for it and clear the word of the child as it ends, and the
//     word of a memory sharer it kills; then takes a signal on an alternate stack in its fifth
//     and sixth; then ends.
//   proxy: prints its process id and its user id, each by raw calls and one write.
//   scatter: in a child, receives the first of two datagrams with recvfrom into the buffer's
//     second page, its address into the third and the address's length from the fourth, then
//     the second datagram, and prints both.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <poll.h>
#include <pthread.h>
#include <sched.h>
#include <signal.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <linux/capability.h>
#include <sys/epoll.h>
#include <sys/ioctl.h>
#include <sys/ipc.h>
#include <sys/mman.h>
#include <sys/msg.h>
#include <sys/prctl.h>
#include <sys/random.h>
#include <sys/resource.h>
#include <sys/select.h>
#include <sys/sendfile.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <sys/timerfd.h>
#include <sys/times.h>
#include <sys/uio.h>
#include <sys/un.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static char *buffer;
static long pages, page;

// Where each call writes: inside the buffer's second page.
#define AT (buffer + page + 64)

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

static void *exec_main(void *argument) {
    (void)argument;
    execl("/bin/true", "true", (char *)NULL);
    return NULL;
}

// A child whose group outlives it: a grandchild leaves the group by `leave` once the child is
// reaped and another child has come and gone, which lets the child's number go.
static void group_outlives(int (*leave)(void)) {
    int ready[2];
    pipe(ready);
    pid_t child = fork();
    if (child == 0) {
        setpgid(0, 0);
        if (fork() == 0) {
            char byte;
            read(ready[0], &byte, 1);
            leave();
            _exit(0);
        }
        _exit(0);
    }
    waitpid(child, NULL, 0);
    // A child made and reaped in between: the group keeps the number until the grandchild leaves.
    pid_t between = fork();
    if (between == 0) _exit(0);
    waitpid(between, NULL, 0);
    write(ready[1], "", 1);
    while (wait(NULL) > 0) {
    }
    close(ready[0]);
    close(ready[1]);
}

static int join_own_group(void) { return setpgid(0, 0); }
static int new_session(void) { return setsid(); }

static int paths(void) {
    // Death by a fatal signal.
    pid_t child = fork();
    if (child == 0) {
        raise(SIGTERM);
        _exit(1);
    }
    int status;
    waitpid(child, &status, 0);
    printf("signalled %d\n", WTERMSIG(status));
    // execve from a second thread, which takes the process's number.
    child = fork();
    if (child == 0) {
        pthread_t thread;
        pthread_create(&thread, NULL, exec_main, NULL);
        pthread_join(thread, NULL);
        _exit(1);
    }
    waitpid(child, &status, 0);
    printf("threaded exec %d\n", WEXITSTATUS(status));
    // vfork.
    child = vfork();
    if (child == 0) {
        execl("/bin/true", "true", (char *)NULL);
        _exit(127);
    }
    waitpid(child, &status, 0);
    printf("vfork %d\n", WEXITSTATUS(status));
    group_outlives(join_own_group);
    group_outlives(new_session);
    printf("groups left\n");
    // A child its parent does not want, reaped as it ends.
    signal(SIGCHLD, SIG_IGN);
    if (fork() == 0) _exit(0);
    printf("unwanted %d\n", waitpid(-1, NULL, 0) == -1 && errno == ECHILD);
    signal(SIGCHLD, SIG_DFL);
    return 0;
}

static void handled(int signal) {
    static const char text[] = "handled\n";
    (void)signal;
    write(1, text, sizeof text - 1);
}

static long seconds_left(struct timeval value) { return value.tv_sec + (value.tv_usec > 0); }
static long seconds_left_spec(struct timespec value) { return value.tv_sec + (value.tv_nsec > 0); }

static void call_read(void) {
    int fds[2];
    pipe(fds);
    write(fds[1], "hello pipe", 10);
    int n = read(fds[0], AT, 64);
    printf("read %d %.*s\n", n, n, AT);
}

static void call_readv(void) {
    int fds[2];
    pipe(fds);
    write(fds[1], "abcdefghij", 10);
    struct iovec iov[2] = {{AT, 4}, {buffer + 2 * page + 8, 6}};
    int n = readv(fds[0], iov, 2);
    printf("readv %d %.4s%.6s\n", n, AT, buffer + 2 * page + 8);
}

static void call_recvmsg(void) {
    int fds[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, fds);
    send(fds[1], "datagram", 8, 0);
    struct iovec iov = {AT, 64};
    struct msghdr message = {.msg_iov = &iov, .msg_iovlen = 1};
    int n = recvmsg(fds[0], &message, 0);
    printf("recvmsg %d %.*s\n", n, n, AT);
}

static void call_frame(void) {
    stack_t stack = {.ss_sp = buffer, .ss_size = pages * page};
    sigaltstack(&stack, NULL);
    struct sigaction action = {.sa_handler = handled, .sa_flags = SA_ONSTACK};
    sigaction(SIGUSR1, &action, NULL);
    raise(SIGUSR1);
    printf("frame done\n");
}

static void call_pread(void) { printf("pread %d\n", (int)pread(open("/etc/hostname", 0), AT, 64, 0)); }
static void call_getcwd(void) { printf("getcwd %s\n", getcwd(AT, 64) ? AT : "failed"); }

static void call_uname(void) {
    printf("uname %d %s\n", uname((struct utsname *)AT), ((struct utsname *)AT)->sysname);
}

static void call_fstat(void) { printf("fstat %d\n", fstat(0, (struct stat *)AT)); }

static void call_pipe2(void) {
    printf("pipe2 %d %d %d\n", pipe2((int *)AT, 0), ((int *)AT)[0], ((int *)AT)[1]);
}

static void call_wait4(void) {
    pid_t child = fork();
    if (child == 0) _exit(5);
    printf("wait4 %d %d\n", wait4(child, (int *)AT, 0, NULL) == child, WEXITSTATUS(*(int *)AT));
}

static void call_getrandom(void) { printf("getrandom %d\n", (int)getrandom(AT, 16, 0)); }

static void call_clock_gettime(void) {
    printf("clock_gettime %d\n", clock_gettime(CLOCK_MONOTONIC, (struct timespec *)AT));
}

static void call_setitimer(void) {
    struct itimerval first = {.it_value = {50, 0}}, second = {.it_value = {100, 0}};
    setitimer(ITIMER_REAL, &first, NULL);
    struct itimerval *old = (struct itimerval *)AT;
    int result = setitimer(ITIMER_REAL, &second, old);
    struct itimerval now;
    getitimer(ITIMER_REAL, &now);
    printf("setitimer %d old %ld now %ld\n", result, seconds_left(old->it_value), seconds_left(now.it_value));
}

static void call_sigaltstack(void) {
    stack_t stack = {.ss_sp = buffer + 4 * page, .ss_size = 2 * page};
    sigaltstack(&stack, NULL);
    stack_t off = {.ss_flags = SS_DISABLE};
    stack_t *old = (stack_t *)AT;
    int result = sigaltstack(&off, old);
    printf("sigaltstack %d old is first %d\n", result, old->ss_sp == stack.ss_sp);
}

static int fds[2];

static void prepare_recvmmsg(void) {
    socketpair(AF_UNIX, SOCK_DGRAM, 0, fds);
    send(fds[1], "dgram", 5, 0);
    struct iovec *iov = (struct iovec *)(buffer + 4 * page);
    *iov = (struct iovec){buffer + 3 * page, 64};
    *(struct mmsghdr *)AT = (struct mmsghdr){.msg_hdr = {.msg_iov = iov, .msg_iovlen = 1}};
}

static void call_recvmmsg(void) {
    struct mmsghdr *message = (struct mmsghdr *)AT;
    int n = recvmmsg(fds[0], message, 1, MSG_DONTWAIT, NULL);
    printf("recvmmsg %d %d %.5s\n", n, n > 0 ? (int)message->msg_len : errno, buffer + 3 * page);
}

// A received message's header on the buffer's fourth page, apart from its data: the call writes
// the header's lengths and flags back after taking the data. The calls are made directly, since
// the C library's wrappers copy or clear the header themselves.
static void prepare_message(int type, const char *text, size_t room) {
    socketpair(AF_UNIX, type, 0, fds);
    send(fds[1], text, strlen(text), 0);
    struct iovec *iov = (struct iovec *)(buffer + 4 * page);
    *iov = (struct iovec){AT, room};
    *(struct msghdr *)(buffer + 3 * page) = (struct msghdr){.msg_iov = iov, .msg_iovlen = 1};
}

static void prepare_recvmsg_datagram(void) { prepare_message(SOCK_DGRAM, "datagram", 64); }
static void prepare_recvmsg_stream(void) { prepare_message(SOCK_STREAM, "streamed", 5); }

static void call_recvmsg_header(void) {
    struct msghdr *message = (struct msghdr *)(buffer + 3 * page);
    int n = syscall(SYS_recvmsg, fds[0], message, 0);
    char rest[16];
    int more = recv(fds[0], rest, sizeof rest, MSG_DONTWAIT);
    printf("recvmsg header %d %.*s flags %d rest %d %.*s\n", n, n, AT, message->msg_flags, more,
           more > 0 ? more : 0, rest);
}

// An mmsghdr whose header ends the buffer's fourth page and whose length starts the fifth.
static void prepare_recvmmsg_split(void) {
    socketpair(AF_UNIX, SOCK_DGRAM, 0, fds);
    send(fds[1], "split", 5, 0);
    struct iovec *iov = (struct iovec *)(buffer + 5 * page);
    *iov = (struct iovec){AT, 64};
    struct mmsghdr *message = (struct mmsghdr *)(buffer + 4 * page - offsetof(struct mmsghdr, msg_len));
    *message = (struct mmsghdr){.msg_hdr = {.msg_iov = iov, .msg_iovlen = 1}};
}

static void call_recvmmsg_split(void) {
    struct mmsghdr *message = (struct mmsghdr *)(buffer + 4 * page - offsetof(struct mmsghdr, msg_len));
    int n = syscall(SYS_recvmmsg, fds[0], message, 1, MSG_DONTWAIT, NULL);
    printf("recvmmsg split %d %u %.5s flags %d\n", n, message->msg_len, AT, message->msg_hdr.msg_flags);
}

// A size sched_setattr refuses, which it answers by writing the size it knows.
static void prepare_sched_setattr(void) { *(unsigned *)AT = 1; }

static void call_sched_setattr(void) {
    int result = syscall(SYS_sched_setattr, 0, AT, 0);
    printf("sched_setattr %d %d %u\n", result, result ? errno : 0, *(unsigned *)AT);
}

// A capability version capset does not know, which it answers by writing the one it prefers.
static void prepare_capset(void) {
    *(struct __user_cap_header_struct *)AT = (struct __user_cap_header_struct){1, 0};
}

static void call_capset(void) {
    struct __user_cap_data_struct data[2] = {0};
    int result = syscall(SYS_capset, AT, data);
    printf("capset %d %d %x\n", result, result ? errno : 0, ((struct __user_cap_header_struct *)AT)->version);
}

static void call_msgrcv(void) {
    int queue = msgget(IPC_PRIVATE, 0600);
    struct {
        long type;
        char text[8];
    } sent = {1, "msg"};
    msgsnd(queue, &sent, 4, 0);
    int n = msgrcv(queue, AT, 8, 0, IPC_NOWAIT);
    printf("msgrcv %d %s\n", n, n > 0 ? AT + sizeof(long) : strerror(errno));
    msgctl(queue, IPC_RMID, NULL);
}

static void prepare_offset(void) { *(loff_t *)AT = 0; }

static void call_sendfile(void) {
    int file = open("/etc/passwd", O_RDONLY), fds[2];
    pipe(fds);
    off_t *offset = (off_t *)AT;
    int n = sendfile(fds[1], file, offset, 4);
    char got[8] = {0};
    read(fds[0], got, 4);
    printf("sendfile %d %ld %s then %ld\n", n, (long)*offset, got, (long)lseek(file, 0, SEEK_CUR));
}

static void call_timerfd_settime(void) {
    int timer = timerfd_create(CLOCK_MONOTONIC, 0);
    struct itimerspec first = {.it_value = {50, 0}}, second = {.it_value = {100, 0}};
    timerfd_settime(timer, 0, &first, NULL);
    struct itimerspec *old = (struct itimerspec *)AT;
    int result = timerfd_settime(timer, 0, &second, old);
    printf("timerfd_settime %d old %ld\n", result, seconds_left_spec(old->it_value));
}

static void call_timer_settime(void) {
    timer_t timer;
    struct sigevent event = {.sigev_notify = SIGEV_NONE};
    timer_create(CLOCK_MONOTONIC, &event, &timer);
    struct itimerspec first = {.it_value = {50, 0}}, second = {.it_value = {100, 0}};
    timer_settime(timer, 0, &first, NULL);
    struct itimerspec *old = (struct itimerspec *)AT;
    int result = timer_settime(timer, 0, &second, old);
    printf("timer_settime %d old %ld\n", result, seconds_left_spec(old->it_value));
}

static void prepare_pselect6(void) {
    pipe(fds);
    write(fds[1], "x", 1);
    FD_ZERO((fd_set *)AT);
    FD_SET(fds[0], (fd_set *)AT);
}

static void call_pselect6(void) {
    struct timespec none = {0, 0};
    int n = pselect(fds[0] + 1, (fd_set *)AT, NULL, NULL, &none, NULL);
    printf("pselect6 %d %d\n", n, FD_ISSET(fds[0], (fd_set *)AT));
}

// The option's value at AT, its length after it.
#define LENGTH ((socklen_t *)(AT + sizeof(long)))

static void prepare_getsockopt(void) { *LENGTH = sizeof(int); }

static void call_getsockopt(void) {
    int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    int result = getsockopt(socket_fd, SOL_SOCKET, SO_TYPE, AT, LENGTH);
    printf("getsockopt %d %d %d\n", result, *(int *)AT, (int)*LENGTH);
}

static void call_fionread(void) {
    int fds[2];
    pipe(fds);
    write(fds[1], "12345", 5);
    printf("ioctl %d %d\n", ioctl(fds[0], FIONREAD, (int *)AT), *(int *)AT);
}

static void call_getgroups(void) {
    int n = getgroups(16, (gid_t *)AT);
    printf("getgroups %d %d\n", n, n > 0 ? (int)((gid_t *)AT)[0] : -1);
}

static void call_prctl(void) {
    prctl(PR_SET_NAME, "watched");
    printf("prctl %d %s\n", prctl(PR_GET_NAME, AT), AT);
}

static void call_getresuid(void) {
    uid_t *ids = (uid_t *)AT;
    printf("getresuid %d %d %d %d\n", getresuid(ids, ids + 1, ids + 2), ids[0], ids[1], ids[2]);
}

static void call_sched_getaffinity(void) {
    long n = syscall(SYS_sched_getaffinity, 0, 128, AT);
    printf("sched_getaffinity %d\n", n > 0);
}

static void call_rt_sigtimedwait(void) {
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR2);
    sigprocmask(SIG_BLOCK, &set, NULL);
    raise(SIGUSR2);
    struct timespec none = {0, 0};
    int signal = sigtimedwait(&set, (siginfo_t *)AT, &none);
    printf("sigtimedwait %d %d\n", signal, ((siginfo_t *)AT)->si_signo);
}

static void call_splice(void) {
    int file = open("/etc/passwd", O_RDONLY), fds[2];
    pipe(fds);
    loff_t *offset = (loff_t *)AT;
    long n = splice(file, offset, fds[1], NULL, 4, 0);
    char got[8] = {0};
    read(fds[0], got, 4);
    printf("splice %ld %ld %s\n", n, (long)*offset, got);
}

static void call_copy_file_range(void) {
    int file = open("/etc/passwd", O_RDONLY);
    int out = open("/tmp/copied", O_RDWR | O_CREAT | O_TRUNC, 0600);
    loff_t *offset = (loff_t *)AT;
    long n = copy_file_range(file, offset, out, NULL, 4, 0);
    printf("copy_file_range %ld %ld %ld\n", n, (long)*offset, (long)lseek(file, 0, SEEK_CUR));
}

static void call_times(void) { printf("times %d\n", times((struct tms *)AT) != (clock_t)-1); }

static void call_getitimer(void) {
    struct itimerval first = {.it_value = {50, 0}};
    setitimer(ITIMER_REAL, &first, NULL);
    struct itimerval *now = (struct itimerval *)AT;
    printf("getitimer %d %ld\n", getitimer(ITIMER_REAL, now), seconds_left(now->it_value));
}

static void call_statfs(void) { printf("statfs %d\n", statfs("/", (struct statfs *)AT)); }

static void call_readlinkat(void) {
    long n = readlinkat(AT_FDCWD, "/proc/self/cwd", AT, 64);
    printf("readlinkat %ld %.*s\n", n, (int)n, AT);
}

static void call_capget(void) {
    struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
    struct __user_cap_data_struct *data = (struct __user_cap_data_struct *)AT;
    int result = syscall(SYS_capget, &header, data);
    printf("capget %d %x\n", result, data[0].effective);
}

static void call_epoll_pwait(void) {
    int fds[2], poller = epoll_create1(0);
    pipe(fds);
    write(fds[1], "x", 1);
    struct epoll_event event = {.events = EPOLLIN, .data.u32 = 7};
    epoll_ctl(poller, EPOLL_CTL_ADD, fds[0], &event);
    struct epoll_event *got = (struct epoll_event *)AT;
    int n = epoll_pwait(poller, got, 4, 0, NULL);
    printf("epoll_pwait %d %u\n", n, got->data.u32);
}

static void call_waitid(void) {
    pid_t child = fork();
    if (child == 0) _exit(3);
    siginfo_t *info = (siginfo_t *)AT;
    int result = waitid(P_PID, child, info, WEXITED);
    printf("waitid %d %d\n", result, info->si_status);
}

static void call_getrlimit(void) {
    struct rlimit *limit = (struct rlimit *)AT;
    printf("getrlimit %d %ld\n", getrlimit(RLIMIT_NOFILE, limit), (long)limit->rlim_cur);
}

static void prepare_fcntl_getlk(void) {
    *(struct flock *)AT = (struct flock){.l_type = F_WRLCK, .l_whence = SEEK_SET};
}

static void call_fcntl_getlk(void) {
    int file = open("/tmp/locked", O_RDWR | O_CREAT, 0600);
    int result = fcntl(file, F_GETLK, (struct flock *)AT);
    printf("fcntl %d %d\n", result, ((struct flock *)AT)->l_type == F_UNLCK);
}

// The kernel's own stores: the parent's and the child's copies of the child's number, and the
// word cleared when a child sharing the memory ends.
static long clone_child(unsigned long flags, int *parent_word, int *child_word) {
    long child = syscall(SYS_clone, flags | SIGCHLD, 0, parent_word, 0, child_word);
    if (child == 0) syscall(SYS_exit, 0);
    waitpid(child, NULL, 0);
    return child;
}

static void call_parent_settid(void) {
    int *word = (int *)AT;
    long child = clone_child(CLONE_VM | CLONE_VFORK | CLONE_PARENT_SETTID, word, NULL);
    printf("parent settid %d\n", *word == child);
}

static void call_child_settid(void) {
    int *word = (int *)AT;
    long child = clone_child(CLONE_VM | CLONE_VFORK | CLONE_CHILD_SETTID, NULL, word);
    printf("child settid %d\n", *word == child);
}

static void call_child_cleartid(void) {
    int *word = (int *)AT;
    clone_child(CLONE_VM | CLONE_VFORK | CLONE_CHILD_CLEARTID, NULL, word);
    printf("child cleartid %d\n", *word == 0);
}

// A process of its own sharing the memory, waiting in ppoll on a stack of its own, killed by
// another process: Linux clears its word as it ends (`mm_release`).
static char sharer_stack[PTHREAD_STACK_MIN] __attribute__((aligned(16)));

static long clone_waiting_sharer(int *child_word) {
    register long x8 __asm__("x8") = SYS_clone;
    register unsigned long x0 __asm__("x0") = CLONE_VM | CLONE_CHILD_CLEARTID | SIGCHLD;
    register void *x1 __asm__("x1") = sharer_stack + sizeof sharer_stack;
    register long x2 __asm__("x2") = 0;
    register long x3 __asm__("x3") = 0;
    register int *x4 __asm__("x4") = child_word;
    __asm__ volatile("svc #0\n"
                     "cbnz x0, 2f\n"
                     "1: mov x0, xzr\n mov x1, xzr\n mov x2, xzr\n mov x3, xzr\n"
                     "mov x8, %[ppoll]\n svc #0\n b 1b\n"
                     "2:\n"
                     : "+r"(x0), "+r"(x1), "+r"(x2), "+r"(x3), "+r"(x4), "+r"(x8)
                     : [ppoll] "i"(SYS_ppoll)
                     : "memory");
    return (long)x0;
}

static void kill_sharer(int signal) {
    int *word = (int *)AT;
    long child = clone_waiting_sharer(word);
    kill(child, signal);
    int status = 0;
    waitpid(child, &status, 0);
    printf("killed sharer %d cleartid %d\n", WIFSIGNALED(status) && WTERMSIG(status) == signal,
           *(volatile int *)word == 0);
}

static void call_sigkill_sharer(void) { kill_sharer(SIGKILL); }
static void call_sigterm_sharer(void) { kill_sharer(SIGTERM); }

// Each call, and what the parent puts in the buffer before the child that makes it starts.
static const struct {
    void (*prepare)(void);
    void (*call)(void);
} CALLS[] = {
    {NULL, call_read},
    {NULL, call_readv},
    {NULL, call_recvmsg},
    {NULL, call_frame},
    {NULL, call_pread},
    {NULL, call_getcwd},
    {NULL, call_uname},
    {NULL, call_fstat},
    {NULL, call_pipe2},
    {NULL, call_wait4},
    {NULL, call_getrandom},
    {NULL, call_clock_gettime},
    {NULL, call_setitimer},
    {NULL, call_sigaltstack},
    {prepare_recvmmsg, call_recvmmsg},
    {NULL, call_msgrcv},
    {prepare_offset, call_sendfile},
    {NULL, call_timerfd_settime},
    {NULL, call_timer_settime},
    {prepare_pselect6, call_pselect6},
    {prepare_getsockopt, call_getsockopt},
    {NULL, call_fionread},
    {NULL, call_getgroups},
    {NULL, call_prctl},
    {NULL, call_getresuid},
    {NULL, call_sched_getaffinity},
    {NULL, call_rt_sigtimedwait},
    {prepare_offset, call_splice},
    {prepare_offset, call_copy_file_range},
    {NULL, call_times},
    {NULL, call_getitimer},
    {NULL, call_statfs},
    {NULL, call_readlinkat},
    {NULL, call_capget},
    {NULL, call_epoll_pwait},
    {NULL, call_waitid},
    {NULL, call_getrlimit},
    {prepare_fcntl_getlk, call_fcntl_getlk},
    {NULL, call_parent_settid},
    {NULL, call_child_settid},
    {NULL, call_child_cleartid},
    {prepare_recvmsg_datagram, call_recvmsg_header},
    {prepare_recvmsg_stream, call_recvmsg_header},
    {prepare_recvmmsg_split, call_recvmmsg_split},
    {prepare_sched_setattr, call_sched_setattr},
    {prepare_capset, call_capset},
    {NULL, call_sigkill_sharer},
    {NULL, call_sigterm_sharer},
};

static int calls(void) {
    int count = sizeof CALLS / sizeof CALLS[0];
    printf("calls %d\n", count);
    fflush(stdout);
    for (int which = 0; which < count; which++) {
        memset(buffer, 'x', pages * page);
        if (CALLS[which].prepare) CALLS[which].prepare();
        pid_t child = fork();
        if (child == 0) {
            CALLS[which].call();
            fflush(stdout);
            _exit(0);
        }
        waitpid(child, NULL, 0);
    }
    return 0;
}

// Writes pages 0, 2 and 5 of the buffer itself and page 7 by read(), and maps new memory over
// page 3 (writable) and page 4 (read-only).
static int written_pages(void) {
    pid_t child = fork();
    if (child == 0) {
        buffer[0] = 1;
        buffer[2 * page + 100] = 2;
        buffer[5 * page + page - 1] = 3;
        int fds[2];
        pipe(fds);
        write(fds[1], "page", 4);
        read(fds[0], buffer + 7 * page + 10, 4);
        mmap(buffer + 3 * page, page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
        mmap(buffer + 4 * page, page, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    return 0;
}

static int span(void) {
    pid_t child = fork();
    if (child == 0) {
        *(volatile long *)(buffer + 2 * page - 2 * sizeof(long)) = 1;
        _exit(0);
    }
    waitpid(child, NULL, 0);
    return 0;
}

static int outgrow(long piece) {
    int count = 0;
    while (mmap(NULL, piece, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0) != MAP_FAILED)
        count++;
    printf("outgrown %d %s\n", count, errno == ENOMEM ? "ENOMEM" : "other");
    return 0;
}

static int fixed(char *at) {
    int flags = PROT_READ | PROT_WRITE, kind = MAP_PRIVATE | MAP_ANONYMOUS;
    void *first = mmap(at, page, flags, kind | MAP_FIXED, -1, 0);
    printf("fixed %s\n", first == at ? "mapped" : strerror(errno));
    void *second = mmap(at + page, page, flags, kind | MAP_FIXED_NOREPLACE, -1, 0);
    printf("noreplace %s\n", second == at + page ? "mapped" : strerror(errno));
    return 0;
}

static int grow_heap(long bytes) {
    munmap(buffer, pages * page);
    long old = syscall(SYS_brk, 0);
    long now = syscall(SYS_brk, old + bytes);
    printf("brk %s\n", now == old + bytes ? "grown" : now == old ? "refused" : "other");
    return 0;
}

static int remap(char *to, long count) {
    pid_t child = fork();
    if (child == 0) {
        char *moved = mremap(buffer, count * page, count * page, MREMAP_MAYMOVE | MREMAP_FIXED, to);
        if (moved != to) {
            printf("mremap %s\n", strerror(errno));
            fflush(stdout);
            _exit(1);
        }
        unsigned long sum = 0;
        for (long i = 0; i < count * page; i++) sum = sum * 31 + (unsigned char)moved[i];
        moved[(count - 1) * page] = 'y';
        madvise(buffer + count * page, page, MADV_DONTNEED);
        printf("moved %lu dropped %d\n", sum, buffer[count * page]);
        fflush(stdout);
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    return WEXITSTATUS(status);
}

static int orphan(void) {
    if (fork() == 0) {
        pause();
        _exit(0);
    }
    return 3;
}

static int shared_read_only(char *at) {
    int fd = memfd_create("shared", 0);
    if (fd < 0 || write(fd, "x", 1) != 1 || ftruncate(fd, page)) return 1;
    if (mmap(at, page, PROT_READ, MAP_SHARED | MAP_FIXED, fd, 0) != at) return 1;
    pid_t child = fork();
    if (child == 0) _exit(0);
    waitpid(child, NULL, 0);
    char byte = 0;
    pread(fd, &byte, 1, 0);
    printf("file %c mapped %c\n", byte, at[0]);
    return 0;
}

static int change_protection(char *at, long length) {
    return mprotect(at, length, PROT_READ) || mprotect(at, length, PROT_READ | PROT_WRITE);
}
static int same_protection(char *at, long length) { return mprotect(at, length, PROT_READ | PROT_WRITE); }
static int key_protection(char *at, long length) {
    return syscall(SYS_pkey_mprotect, at, length, PROT_READ | PROT_WRITE, -1);
}
static int lock(char *at, long length) { return mlock(at, length) || munlock(at, length); }
static int lock_on_fault(char *at, long length) {
    return syscall(SYS_mlock2, at, length, MLOCK_ONFAULT) || munlock(at, length);
}
static int not_forked(char *at, long length) {
    return madvise(at, length, MADV_DONTFORK) || madvise(at, length, MADV_DOFORK);
}
static int wiped_on_fork(char *at, long length) {
    return madvise(at, length, MADV_WIPEONFORK) || madvise(at, length, MADV_KEEPONFORK);
}

// Calls that change a range's attributes and leave its pages and their bytes as they were.
static const struct {
    const char *name;
    int (*change)(char *, long);
} CHANGES[] = {
    {"mprotect", change_protection},  {"mprotect-same", same_protection},
    {"pkey_mprotect", key_protection}, {"mlock", lock},
    {"mlock2", lock_on_fault},         {"madvise-dontfork", not_forked},
    {"madvise-wipeonfork", wiped_on_fork},
};

static int attributes(void) {
    int count = sizeof CHANGES / sizeof CHANGES[0];
    printf("attributes %d\n", count);
    fflush(stdout);
    for (int which = 0; which < count; which++) {
        pid_t child = fork();
        if (child == 0) {
            buffer[0] = 'p';
            int result = CHANGES[which].change(buffer, 3 * page);
            volatile char *pages = buffer;
            char seen = pages[page];
            pages[page + 1] = 'p';
            pages[2 * page + 1] = 'p';
            printf("%s %d read %c\n", CHANGES[which].name, result, seen);
            fflush(stdout);
            _exit(0);
        }
        waitpid(child, NULL, 0);
    }
    return 0;
}

static int spend(long bytes) {
    pid_t child = fork();
    if (child == 0) {
        if (mmap(NULL, bytes, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0) == MAP_FAILED)
            _exit(1);
        volatile unsigned long sum = 0;
        for (long i = 0; i < bytes; i++) sum += i;
        struct timespec used;
        clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &used);
        printf("used %lld %ld\n", (long long)used.tv_sec, used.tv_nsec);
        fflush(stdout);
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    return WEXITSTATUS(status);
}

static int scatter(void) {
    pid_t child = fork();
    if (child == 0) {
        int ends[2];
        if (socketpair(AF_UNIX, SOCK_DGRAM, 0, ends)) _exit(1);
        send(ends[1], "one", 3, 0);
        send(ends[1], "two", 3, 0);
        char *data = buffer + page;
        struct sockaddr *address = (struct sockaddr *)(buffer + 2 * page);
        socklen_t *length = (socklen_t *)(buffer + 3 * page);
        *length = sizeof(struct sockaddr_un);
        long got = recvfrom(ends[0], data, 16, 0, address, length);
        char next[16] = {0};
        recv(ends[0], next, sizeof next - 1, MSG_DONTWAIT);
        printf("recvfrom %ld %.*s len %d next %s\n", got, (int)(got > 0 ? got : 0), data, (int)*length, next);
        fflush(stdout);
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    return WEXITSTATUS(status);
}

static void on_frame_signal(int signal) { (void)signal; }

static int kernel_writes(long length) {
    int ends[2];
    if (pipe(ends)) return 1;
    char *bytes = malloc(length);
    memset(bytes, 'k', length);
    write(ends[1], bytes, length);
    pid_t child = fork();
    if (child == 0) {
        syscall(SYS_read, ends[0], AT, length);
        syscall(SYS_clock_gettime, CLOCK_MONOTONIC, buffer + 2 * page + sizeof(long));
        int *words = (int *)(buffer + 3 * page);
        clone_child(CLONE_VM | CLONE_VFORK | CLONE_PARENT_SETTID | CLONE_CHILD_CLEARTID, &words[0],
                    &words[1]);
        long sharer = clone_waiting_sharer(&words[2]);
        kill(sharer, SIGKILL);
        waitpid(sharer, NULL, 0);
        stack_t stack = {.ss_sp = buffer + 4 * page, .ss_size = 2 * page};
        syscall(SYS_sigaltstack, &stack, NULL);
        struct sigaction action = {.sa_handler = on_frame_signal, .sa_flags = SA_ONSTACK};
        sigaction(SIGUSR1, &action, NULL);
        syscall(SYS_tgkill, syscall(SYS_getpid), syscall(SYS_gettid), SIGUSR1);
        syscall(SYS_exit_group, 0);
    }
    waitpid(child, NULL, 0);
    return 0;
}

static int proxy(void) {
    char line[64];
    int length = snprintf(line, sizeof line, "pid %ld\n", syscall(SYS_getpid));
    syscall(SYS_write, STDOUT_FILENO, line, length);
    length = snprintf(line, sizeof line, "uid %ld\n", syscall(SYS_getuid));
    syscall(SYS_write, STDOUT_FILENO, line, length);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 4) return 2;
    buffer = (char *)strtoul(argv[2], NULL, 0);
    pages = strtol(argv[3], NULL, 0);
    page = sysconf(_SC_PAGESIZE);
    int flags = PROT_READ | PROT_WRITE, kind = MAP_FIXED | MAP_PRIVATE | MAP_ANONYMOUS;
    if (mmap(buffer, pages * page, flags, kind, -1, 0) != buffer) return 1;
    memset(buffer, 'x', pages * page);
    const char *mode = argv[1];
    if (!strcmp(mode, "lifecycle")) return lifecycle();
    if (!strcmp(mode, "paths")) return paths();
    if (!strcmp(mode, "calls")) return calls();
    if (!strcmp(mode, "pages")) return written_pages();
    if (!strcmp(mode, "span")) return span();
    if (!strcmp(mode, "outgrow") && argc > 4) return outgrow(strtol(argv[4], NULL, 0));
    if (!strcmp(mode, "fixed") && argc > 4) return fixed((char *)strtoul(argv[4], NULL, 0));
    if (!strcmp(mode, "brk") && argc > 4) return grow_heap(strtol(argv[4], NULL, 0));
    if (!strcmp(mode, "remap") && argc > 5)
        return remap((char *)strtoul(argv[4], NULL, 0), strtol(argv[5], NULL, 0));
    if (!strcmp(mode, "orphan")) return orphan();
    if (!strcmp(mode, "shared") && argc > 4) return shared_read_only((char *)strtoul(argv[4], NULL, 0));
    if (!strcmp(mode, "attributes")) return attributes();
    if (!strcmp(mode, "spend") && argc > 4) return spend(strtol(argv[4], NULL, 0));
    if (!strcmp(mode, "scatter")) return scatter();
    if (!strcmp(mode, "kernelwrites") && argc > 4) return kernel_writes(strtol(argv[4], NULL, 0));
    if (!strcmp(mode, "proxy")) return proxy();
    return 2;
}
