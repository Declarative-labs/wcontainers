// A splice from a terminal into a pipe waits holding the pipe's lock, so a reader, whether it waits
// at the pipe already or comes to it afterwards, waits for the next line. Each command runs with a pseudo-terminal as its
// controlling terminal; "abc\n" is typed and what the terminal shows within a while is printed.

#define _GNU_SOURCE
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static void run(const char *command) {
    int master = posix_openpt(O_RDWR | O_NOCTTY);
    grantpt(master);
    unlockpt(master);
    char name[64];
    ptsname_r(master, name, sizeof name);
    pid_t c = fork();
    if (c == 0) {
        setsid();
        int slave = open(name, O_RDWR);
        int null = open("/dev/null", O_RDONLY);
        dup2(null, 0);
        dup2(slave, 1);
        dup2(slave, 2);
        close(master);
        execl("/bin/sh", "sh", "-c", command, (char *)0);
        _exit(127);
    }
    usleep(300000);
    write(master, "abc\n", 4);
    char buf[512];
    size_t got = 0;
    for (int i = 0; i < 25; i++) {
        struct pollfd p = {master, POLLIN, 0};
        if (poll(&p, 1, 100) > 0) {
            long n = read(master, buf + got, sizeof buf - 1 - got);
            if (n > 0)
                got += n;
        }
    }
    printf("%s: ", command);
    for (size_t i = 0; i < got; i++) {
        unsigned char ch = buf[i];
        if (ch >= 32 && ch < 127)
            putchar(ch);
        else
            printf("\\x%02x", ch);
    }
    printf("\n");
    kill(-c, SIGKILL);
    kill(c, SIGKILL);
    waitpid(c, NULL, 0);
    close(master);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    run("cat /dev/tty | cat");
    run("cat /dev/tty | (sleep 1; cat)");
    run("cat /dev/tty | (sleep 1; head -c 2)");
    run("head -n 1 /dev/tty | cat");
    return 0;
}
