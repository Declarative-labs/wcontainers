// The terminal `docker run -t` and `-it` give.
//   info: what descriptors 0 to 2 are, the size, the settings and the job control.
//   resize <columns> <rows>: waits for SIGWINCH until the terminal has that size.
//   lines: canonical reads with echo and erase, end of file at a line's start, then raw reads.
//   signals: the interrupt and suspend keys reaching the foreground group (it and a child).
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

static void say(const char *format, ...) __attribute__((format(printf, 1, 2)));

static void say(const char *format, ...) {
    char line[512];
    va_list arguments;
    va_start(arguments, format);
    int length = vsnprintf(line, sizeof line, format, arguments);
    va_end(arguments);
    if (write(1, line, length) != length) _exit(3);
}

static void size(void) {
    struct winsize size;
    if (ioctl(0, TIOCGWINSZ, &size)) {
        say("size failed %s\n", strerror(errno));
        return;
    }
    say("size %d %d\n", size.ws_col, size.ws_row);
}

static int info(void) {
    for (int fd = 0; fd < 3; fd++) say("isatty %d %d\n", fd, isatty(fd));
    char name[64];
    say("ttyname %d %s\n", ttyname_r(0, name, sizeof name), name);
    size();
    struct termios settings;
    tcgetattr(0, &settings);
    say("iflag %o oflag %o cflag %o lflag %o\n", settings.c_iflag, settings.c_oflag, settings.c_cflag,
        settings.c_lflag);
    say("pgrp %d sid %d tcgetpgrp %d tcgetsid %d\n", getpgrp(), getsid(0), tcgetpgrp(0), tcgetsid(0));
    struct stat status;
    fstat(0, &status);
    say("rdev %u %u mode %o gid %d\n", major(status.st_rdev), minor(status.st_rdev), status.st_mode,
        status.st_gid);
    say("same file %d %d\n", fcntl(0, F_GETFL), fcntl(1, F_GETFL));
    char line[256];
    FILE *stat_file = fopen("/proc/self/stat", "r");
    if (stat_file && fgets(line, sizeof line, stat_file)) {
        // Fields 5 to 8: process group, session, terminal, its foreground group.
        char *field = strrchr(line, ')') + 2;
        int pgrp, session, tty, tpgid;
        sscanf(field, "%*c %*d %d %d %d %d", &pgrp, &session, &tty, &tpgid);
        say("stat %d %d %d %d\n", pgrp, session, tty, tpgid);
    }
    FILE *fdinfo = fopen("/proc/self/fdinfo/0", "r");
    while (fdinfo && fgets(line, sizeof line, fdinfo))
        if (!strncmp(line, "flags", 5)) say("%s", line);
    return 0;
}

static volatile sig_atomic_t winched, interrupted, suspended;

static void on_signal(int signal) {
    if (signal == SIGWINCH) winched++;
    if (signal == SIGINT) interrupted++;
    if (signal == SIGTSTP) suspended++;
}

static void catch(int signal) {
    struct sigaction action = {0};
    action.sa_handler = on_signal;
    sigaction(signal, &action, 0);
}

// The client resizes the terminal as it starts, so only the size asked for ends the wait.
static int resize(int columns, int rows) {
    sigset_t blocked, empty;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGWINCH);
    sigprocmask(SIG_BLOCK, &blocked, 0);
    catch(SIGWINCH);
    say("ready\n");
    sigemptyset(&empty);
    struct winsize size = {0};
    while (size.ws_col != columns || size.ws_row != rows) {
        while (!winched) sigsuspend(&empty);
        winched = 0;
        ioctl(0, TIOCGWINSZ, &size);
    }
    say("winch, size %d %d\n", size.ws_col, size.ws_row);
    return 0;
}

static void show_read(void) {
    char buffer[128];
    ssize_t count = read(0, buffer, sizeof buffer);
    // One write, so the line is whole when the outside sees it.
    char line[512];
    int length = snprintf(line, sizeof line, "read %zd:", count);
    for (ssize_t i = 0; i < count; i++)
        length += snprintf(line + length, sizeof line - length, " %02x", (unsigned char)buffer[i]);
    say("%s\n", line);
}

static int lines(void) {
    say("ready\n");
    show_read();
    show_read();
    show_read();
    struct termios settings, raw;
    tcgetattr(0, &settings);
    raw = settings;
    raw.c_lflag &= ~(ICANON | ECHO);
    raw.c_cc[VMIN] = 3;
    raw.c_cc[VTIME] = 0;
    tcsetattr(0, TCSANOW, &raw);
    say("raw\n");
    show_read();
    tcsetattr(0, TCSANOW, &settings);
    say("done\n");
    return 0;
}

static int signals(void) {
    sigset_t blocked, empty;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGINT);
    sigaddset(&blocked, SIGTSTP);
    sigaddset(&blocked, SIGUSR1);
    sigprocmask(SIG_BLOCK, &blocked, 0);
    catch(SIGINT);
    catch(SIGTSTP);
    catch(SIGUSR1);
    sigemptyset(&empty);
    pid_t child = fork();
    if (child == 0) {
        // Ends once it has seen both keys' signals, its status what it saw.
        kill(getppid(), SIGUSR1);
        while (!(interrupted && suspended)) sigsuspend(&empty);
        _exit(interrupted * 16 + suspended);
    }
    // The child is ready once it has said so.
    sigset_t usr1;
    sigemptyset(&usr1);
    sigaddset(&usr1, SIGUSR1);
    int got;
    sigwait(&usr1, &got);
    say("ready\n");
    while (!interrupted) sigsuspend(&empty);
    say("interrupted\n");
    while (!suspended) sigsuspend(&empty);
    say("suspended\n");
    int status;
    waitpid(child, &status, 0);
    say("child %d\n", WIFEXITED(status) ? WEXITSTATUS(status) : -WTERMSIG(status));
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    if (!strcmp(argv[1], "info")) return info();
    if (argc >= 4 && !strcmp(argv[1], "resize")) return resize(atoi(argv[2]), atoi(argv[3]));
    if (!strcmp(argv[1], "lines")) return lines();
    if (!strcmp(argv[1], "signals")) return signals();
    return 2;
}
