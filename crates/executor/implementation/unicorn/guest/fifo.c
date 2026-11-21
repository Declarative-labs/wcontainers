// Named pipes: opens that wait for the other end or do not (nonblocking, read and write), ENXIO for a
// nonblocking writer with no reader, a waiting opener counted as an end, the buffer dropped when the
// last end closes, hangups in poll, a signal ending a waiting open, and what stat, lseek and
// /proc/self/fd say of one.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#define FIFO "/tmp/fifo"

static void result(const char *what, long value) {
    printf("%s: %ld %s\n", what, value, value < 0 ? strerror(errno) : "");
}

static int poll_events(int fd) {
    struct pollfd entry = {.fd = fd, .events = POLLIN | POLLOUT};
    poll(&entry, 1, 0);
    return entry.revents;
}

static volatile int handled;

static void on_signal(int signal) {
    (void)signal;
    handled++;
}

static void nonblocking(void) {
    int reader = open(FIFO, O_RDONLY | O_NONBLOCK);
    result("nonblocking reader, no writer", reader);
    char buffer[16];
    result("read with no writer", read(reader, buffer, sizeof buffer));
    printf("poll before any writer: %#x\n", poll_events(reader));
    int writer = open(FIFO, O_WRONLY | O_NONBLOCK);
    result("nonblocking writer with a reader", writer);
    printf("poll with a writer: %#x\n", poll_events(reader));
    result("write", write(writer, "abc", 3));
    printf("poll with bytes: %#x\n", poll_events(reader));
    close(writer);
    printf("poll after the writer closed: %#x\n", poll_events(reader));
    long got = read(reader, buffer, sizeof buffer);
    printf("read: %ld %.*s\n", got, (int)(got > 0 ? got : 0), buffer);
    result("read at the end", read(reader, buffer, sizeof buffer));
    int second = open(FIFO, O_RDONLY | O_NONBLOCK);
    printf("poll of a new reader after a writer came and went: %#x\n", poll_events(second));
    close(second);
    close(reader);
    result("nonblocking writer, no reader", open(FIFO, O_WRONLY | O_NONBLOCK));
}

static void dropped_when_closed(void) {
    int both = open(FIFO, O_RDWR);
    result("read and write open", both);
    result("write", write(both, "kept?", 5));
    printf("pipe size: %d\n", fcntl(both, F_GETPIPE_SZ));
    result("set pipe size", fcntl(both, F_SETPIPE_SZ, 65536 * 2));
    close(both);
    both = open(FIFO, O_RDWR | O_NONBLOCK);
    char buffer[16];
    result("read after every end closed", read(both, buffer, sizeof buffer));
    printf("pipe size after every end closed: %d\n", fcntl(both, F_GETPIPE_SZ));
    struct stat status;
    fstat(both, &status);
    printf("fifo: %d size %ld\n", S_ISFIFO(status.st_mode), (long)status.st_size);
    result("lseek", lseek(both, 0, SEEK_SET));
    char link[64] = "";
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", both);
    long length = readlink(path, link, sizeof link - 1);
    printf("link: %.*s\n", (int)(length > 0 ? length : 0), link);
    close(both);
}

static void waits(void) {
    int ready[2];
    pipe(ready);
    pid_t child = fork();
    if (child == 0) {
        int writer = open(FIFO, O_WRONLY);
        write(writer, "from the child", 14);
        close(writer);
        _exit(0);
    }
    int reader = open(FIFO, O_RDONLY);
    result("blocking reader", reader);
    char buffer[32];
    long got = 0, total = 0;
    while ((got = read(reader, buffer + total, sizeof buffer - total)) > 0) {
        total += got;
    }
    printf("read: %.*s, then %ld\n", (int)total, buffer, got);
    waitpid(child, NULL, 0);
    close(reader);

    // A reader waiting in open counts as a reader: a nonblocking writer gets in once it waits.
    child = fork();
    if (child == 0) {
        int waiting = open(FIFO, O_RDONLY);
        char byte = 0;
        read(waiting, &byte, 1);
        printf("child read: %c\n", byte);
        _exit(0);
    }
    int writer;
    int tries = 0;
    while ((writer = open(FIFO, O_WRONLY | O_NONBLOCK)) < 0 && errno == ENXIO) {
        tries++;
        usleep(1000);
    }
    result("nonblocking writer once a reader waits", writer);
    write(writer, "w", 1);
    waitpid(child, NULL, 0);
    close(writer);
    close(ready[0]);
    close(ready[1]);
}

static void interrupted(void) {
    struct sigaction action = {0};
    action.sa_handler = on_signal;
    sigaction(SIGUSR1, &action, NULL);
    int done[2];
    pipe(done);
    pid_t parent = getpid();
    pid_t child = fork();
    if (child == 0) {
        close(done[1]);
        fcntl(done[0], F_SETFL, O_NONBLOCK);
        char byte;
        while (read(done[0], &byte, 1) < 0) {
            kill(parent, SIGUSR1);
            usleep(2000);
        }
        _exit(0);
    }
    close(done[0]);
    int fd = open(FIFO, O_RDONLY);
    result("blocking open with a signal", fd);
    printf("handled: %d\n", handled > 0);
    close(done[1]);
    waitpid(child, NULL, 0);
    // The interrupted open gave up its end: a nonblocking writer finds no reader.
    result("nonblocking writer after it", open(FIFO, O_WRONLY | O_NONBLOCK));

    action.sa_flags = SA_RESTART;
    sigaction(SIGUSR1, &action, NULL);
    handled = 0;
    child = fork();
    if (child == 0) {
        for (int i = 0; i < 5; i++) {
            kill(parent, SIGUSR1);
            usleep(2000);
        }
        int writer = open(FIFO, O_WRONLY);
        write(writer, "r", 1);
        _exit(0);
    }
    fd = open(FIFO, O_RDONLY);
    result("blocking open restarted", fd);
    char byte = 0;
    read(fd, &byte, 1);
    printf("read: %c, handled: %d\n", byte, handled > 0);
    waitpid(child, NULL, 0);
    close(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    unlink(FIFO);
    result("mkfifo", mkfifo(FIFO, 0600));
    struct stat status;
    stat(FIFO, &status);
    printf("mode: %o\n", status.st_mode);
    result("open for its path", open(FIFO, O_PATH));
    result("direct, writer with no reader", open(FIFO, O_WRONLY | O_NONBLOCK | O_DIRECT));
    result("direct, nonblocking reader", open(FIFO, O_RDONLY | O_NONBLOCK | O_DIRECT));
    result("direct, read and write", open(FIFO, O_RDWR | O_DIRECT));
    nonblocking();
    dropped_when_closed();
    waits();
    interrupted();
    result("mkfifo again", mkfifo(FIFO, 0600));
    unlink(FIFO);
    return 0;
}
