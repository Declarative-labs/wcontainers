// An interactive shell whose terminal is a pseudo-terminal, driven as a person would: each command
// given after the prompt (through a pipe, so the shell does no line editing of its own), jobs stopped with the suspend key, moved between foreground and background, stopped
// for reading or writing the terminal in the background, and interrupted. Prints what the
// terminal showed.
#define _GNU_SOURCE
#include <dirent.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

#define PROMPT "/ # "

static int master, commands;
static pid_t shell;
static char screen[65536];
static size_t shown;

static void print_screen(void);

static void pause_briefly(void) {
    struct timespec t = {0, 2000000};
    nanosleep(&t, NULL);
}

// Reads what the terminal shows until it ends with `until`.
static void read_until(const char *until) {
    size_t length = strlen(until);
    size_t start = shown;
    for (;;) {
        if (shown - start >= length && memcmp(screen + shown - length, until, length) == 0)
            return;
        struct pollfd p = {.fd = master, .events = POLLIN};
        // Generous: the shared reference machine has taken over 20 s to show a first prompt.
        if (poll(&p, 1, 120000) <= 0) {
            printf("timed out waiting for %s\n", until);
            print_screen();
            kill(shell, SIGKILL);
            exit(1);
        }
        long n = read(master, screen + shown, sizeof screen - shown);
        if (n <= 0)
            return;
        shown += n;
    }
}

static void wait_foreground(const char *name);

static void type(const char *keys) { write(master, keys, strlen(keys)); }

// A command and the prompt after it.
static void command(const char *line) {
    write(commands, line, strlen(line));
    write(commands, "\n", 1);
    read_until(PROMPT);
}

// A command that runs in the foreground until `name` has the terminal.
static void start(const char *line, const char *name) {
    write(commands, line, strlen(line));
    write(commands, "\n", 1);
    wait_foreground(name);
}

// Waits until the foreground group of the terminal runs `name`.
static void wait_foreground(const char *name) {
    for (int tries = 0; tries < 5000; tries++) {
        pid_t group = tcgetpgrp(master);
        if (group > 0 && group != shell) {
            char path[64], comm[32] = {0};
            snprintf(path, sizeof path, "/proc/%d/comm", group);
            int fd = open(path, O_RDONLY);
            if (fd >= 0) {
                read(fd, comm, sizeof comm - 1);
                close(fd);
                if (strncmp(comm, name, strlen(name)) == 0)
                    return;
            }
        }
        pause_briefly();
    }
    printf("no foreground %s\n", name);
}

// Waits until some process runs `name`: a background job has started its program.
static void wait_running(const char *name) {
    for (int tries = 0; tries < 5000; tries++) {
        DIR *proc = opendir("/proc");
        struct dirent *entry;
        int found = 0;
        while (!found && (entry = readdir(proc))) {
            char path[300], comm[32] = {0};
            snprintf(path, sizeof path, "/proc/%s/comm", entry->d_name);
            int fd = open(path, O_RDONLY);
            if (fd < 0)
                continue;
            read(fd, comm, sizeof comm - 1);
            close(fd);
            found = strncmp(comm, name, strlen(name)) == 0 && comm[strlen(name)] == '\n';
        }
        closedir(proc);
        if (found)
            return;
        pause_briefly();
    }
    printf("no %s running\n", name);
}

static void print_screen(void) {
    for (size_t i = 0; i < shown; i++) {
        unsigned char c = screen[i];
        if (c == '\n')
            printf("\\n\n");
        else if (c >= 32 && c < 127)
            putchar(c);
        else
            printf("\\x%02x", c);
    }
    printf("\n");
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    master = posix_openpt(O_RDWR | O_NOCTTY);
    grantpt(master);
    unlockpt(master);
    char name[64];
    ptsname_r(master, name, sizeof name);
    int pipe_in[2];
    pipe(pipe_in);
    commands = pipe_in[1];
    struct winsize size = {.ws_row = 24, .ws_col = 80};
    ioctl(master, TIOCSWINSZ, &size);
    shell = fork();
    if (shell == 0) {
        setsid();
        int slave = open(name, O_RDWR);
        dup2(pipe_in[0], 0);
        dup2(slave, 1);
        dup2(slave, 2);
        close(slave);
        close(master);
        close(pipe_in[0]);
        close(pipe_in[1]);
        execl("/bin/sh", "sh", "-i", NULL);
        _exit(127);
    }
    read_until(PROMPT);
    command("echo hello; stty -F /dev/tty -a | head -n 1");
    command("sleep 30 &");
    wait_running("sleep");
    command("jobs");
    // Whether the shell sees the job end before or after it waits is a race between CPUs; what
    // it prints about it then goes to the wait's own error output.
    command("kill %1; wait %1 2>/dev/null; echo status $?");
    start("sleep 30", "sleep");
    type("\x1a");
    read_until(PROMPT);
    command("jobs");
    start("fg %1", "sleep");
    type("\x03");
    read_until(PROMPT);
    start("cat /dev/tty", "cat");
    type("typed line\n");
    read_until("typed line\r\n");
    type("\x04");
    read_until(PROMPT);
    // A job is seen stopped before anything else is asked of the shell.
    start("cat /dev/tty & until jobs | grep -q Stopped; do sleep 0.01; done; jobs; fg %1", "cat");
    type("for the job\n");
    read_until("for the job\r\n");
    type("\x04");
    read_until(PROMPT);
    command("stty -F /dev/tty tostop; echo written & until jobs | grep -q Stopped; do sleep 0.01; "
            "done; jobs; stty -F /dev/tty -tostop; fg %1");
    command("trap 'echo got INT' INT");
    type("\x03");
    read_until(PROMPT);
    command("echo bye");
    close(commands);
    int status;
    waitpid(shell, &status, 0);
    // What the shell wrote as it ended.
    long n;
    fcntl(master, F_SETFL, O_RDWR | O_NONBLOCK);
    while ((n = read(master, screen + shown, sizeof screen - shown)) > 0)
        shown += n;
    print_screen();
    printf("shell %s %d\n", WIFEXITED(status) ? "exited" : "killed",
           WIFEXITED(status) ? WEXITSTATUS(status) : WTERMSIG(status));
    return 0;
}
