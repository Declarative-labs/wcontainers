// Job control on a pseudo-terminal: controlling terminals and sessions, the foreground group,
// background reads and writes stopped by SIGTTIN and SIGTTOU (or refused when ignored, blocked or
// orphaned), keys that send signals to the foreground group, window size changes, and what a
// hangup, a session leader's end and TIOCNOTTY send.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

static int master;
static char name[64];
// The scenario's session leader asks the first process to type into the master through `ask`;
// processes say they are ready through `ready`.
static int ask[2], ready[2];

static void result(const char *label, long r) {
    if (r < 0)
        printf("%s: errno %d\n", label, errno);
    else
        printf("%s: %ld\n", label, r);
}

static volatile sig_atomic_t got[65];
static void note(int signal) { got[signal]++; }

static void handle(int signal) {
    struct sigaction action = {.sa_handler = note, .sa_flags = SA_RESTART};
    sigaction(signal, &action, NULL);
}

static void signal_ready(void) { write(ready[1], "r", 1); }
static void wait_ready(void) {
    char c;
    read(ready[0], &c, 1);
}

// Asks the first process to type `text` into the master, and waits until it has.
static void type(const char *text) {
    write(ask[1], text, strlen(text));
    wait_ready();
}

static void open_pair(void) {
    master = posix_openpt(O_RDWR | O_NOCTTY);
    grantpt(master);
    unlockpt(master);
    ptsname_r(master, name, sizeof name);
}

static int leader(void) {
    setsid();
    int fd = open(name, O_RDWR);
    return fd;
}

static void show_wait(const char *label, int status) {
    if (WIFSTOPPED(status))
        printf("%s: stopped %d\n", label, WSTOPSIG(status));
    else if (WIFSIGNALED(status))
        printf("%s: killed %d\n", label, WTERMSIG(status));
    else if (WIFEXITED(status))
        printf("%s: exited %d\n", label, WEXITSTATUS(status));
    else
        printf("%s: continued\n", label);
}

static pid_t background(void) {
    pid_t child = fork();
    if (child == 0) {
        setpgid(0, 0);
        return 0;
    }
    setpgid(child, child);
    return child;
}

static void sessions(void) {
    int fd = leader();
    result("open slave as leader", fd < 0 ? -1 : 0);
    printf("foreground is self %d\n", tcgetpgrp(fd) == getpid());
    printf("session is self %d\n", tcgetsid(fd) == getpid());
    int tty = open("/dev/tty", O_RDWR);
    result("open /dev/tty", tty < 0 ? -1 : 0);
    struct stat a, b;
    fstat(tty, &a);
    fstat(fd, &b);
    printf("/dev/tty rdev %d:%d, slave %d:%d\n", major(a.st_rdev), minor(a.st_rdev), major(b.st_rdev),
           minor(b.st_rdev));
    char stat[512];
    int proc = open("/proc/self/stat", O_RDONLY);
    long n = read(proc, stat, sizeof stat - 1);
    stat[n] = 0;
    int ttynr = 0, tpgid = 0;
    sscanf(strrchr(stat, ')') + 2, "%*c %*d %*d %*d %d %d", &ttynr, &tpgid);
    printf("stat tty_nr %d:%d tpgid self %d\n", (ttynr >> 8) & 0xfff, (ttynr & 0xff) | ((ttynr >> 12) & 0xfff00),
           tpgid == getpid());
    result("TIOCSCTTY again", ioctl(fd, TIOCSCTTY, 0));
    result("TIOCSCTTY other", ioctl(master, TIOCSCTTY, 0));
    pid_t child = fork();
    if (child == 0) {
        result("child TIOCSCTTY", ioctl(fd, TIOCSCTTY, 0));
        printf("child TIOCGPGRP is leader %d\n", tcgetpgrp(fd) == getppid());
        setsid();
        result("child after setsid /dev/tty", open("/dev/tty", O_RDWR));
        result("child after setsid TIOCGPGRP", tcgetpgrp(fd));
        result("child steal", ioctl(fd, TIOCSCTTY, 1));
        int other = open(name, O_RDWR);
        result("child open slave as leader", other < 0 ? -1 : 0);
        result("child TIOCGSID", tcgetsid(other));
        exit(0);
    }
    waitpid(child, NULL, 0);
    result("tcsetpgrp negative", tcsetpgrp(fd, -1));
    result("tcsetpgrp missing", tcsetpgrp(fd, 99999));
    result("tcsetpgrp init", tcsetpgrp(fd, 1));
    pid_t job = background();
    if (job == 0) {
        pause();
        exit(0);
    }
    result("tcsetpgrp job", tcsetpgrp(fd, job));
    printf("foreground is job %d\n", tcgetpgrp(fd) == job);
    result("tcsetpgrp back from background", tcsetpgrp(fd, getpid()));
    signal(SIGTTOU, SIG_IGN);
    result("tcsetpgrp back ignoring SIGTTOU", tcsetpgrp(fd, getpid()));
    signal(SIGTTOU, SIG_DFL);
    kill(job, SIGKILL);
    waitpid(job, NULL, 0);
}

static void background_jobs(void) {
    int fd = leader();
    char buf[16];
    int status;
    pid_t job = background();
    if (job == 0) {
        result("background read", read(fd, buf, sizeof buf));
        exit(0);
    }
    waitpid(job, &status, WUNTRACED);
    show_wait("background read", status);
    kill(job, SIGKILL);
    waitpid(job, NULL, 0);
    job = background();
    if (job == 0) {
        result("background write", write(fd, "bg\n", 3));
        exit(0);
    }
    waitpid(job, &status, WUNTRACED);
    show_wait("background write", status);
    struct termios t;
    tcgetattr(fd, &t);
    t.c_lflag |= TOSTOP;
    tcsetattr(fd, TCSANOW, &t);
    job = background();
    if (job == 0) {
        result("background write with tostop", write(fd, "x\n", 2));
        exit(0);
    }
    waitpid(job, &status, WUNTRACED);
    show_wait("background write with tostop", status);
    kill(job, SIGCONT);
    waitpid(job, &status, WUNTRACED | WCONTINUED);
    show_wait("after SIGCONT", status);
    waitpid(job, &status, WUNTRACED);
    show_wait("background write with tostop", status);
    kill(job, SIGKILL);
    waitpid(job, NULL, 0);
    job = background();
    if (job == 0) {
        signal(SIGTTOU, SIG_IGN);
        result("background write ignoring SIGTTOU", write(fd, "ig\n", 3));
        result("background tcsetattr ignoring SIGTTOU", tcsetattr(fd, TCSANOW, &t));
        exit(0);
    }
    waitpid(job, &status, 0);
    job = background();
    if (job == 0) {
        result("background tcsetattr", tcsetattr(fd, TCSANOW, &t));
        exit(0);
    }
    waitpid(job, &status, WUNTRACED);
    show_wait("background tcsetattr", status);
    kill(job, SIGKILL);
    waitpid(job, NULL, 0);
    job = background();
    if (job == 0) {
        signal(SIGTTIN, SIG_IGN);
        result("background read ignoring SIGTTIN", read(fd, buf, sizeof buf));
        signal(SIGTTIN, SIG_DFL);
        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGTTIN);
        sigprocmask(SIG_BLOCK, &set, NULL);
        result("background read blocking SIGTTIN", read(fd, buf, sizeof buf));
        result("background TIOCGPGRP", tcgetpgrp(fd) == getppid());
        result("background TCFLSH", tcflush(fd, TCIFLUSH));
        exit(0);
    }
    waitpid(job, &status, WUNTRACED);
    show_wait("background job", status);
    if (WIFSTOPPED(status)) {
        kill(job, SIGKILL);
        waitpid(job, NULL, 0);
    }
    // A group whose members' parents are all outside the session is orphaned.
    int go[2], done[2];
    pipe(go);
    pipe(done);
    job = background();
    if (job == 0) {
        pid_t inner = fork();
        if (inner == 0) {
            char c;
            read(go[0], &c, 1);
            result("orphaned read", read(fd, buf, sizeof buf));
            result("orphaned write with tostop", write(fd, "or\n", 3));
            fflush(stdout);
            write(done[1], "d", 1);
            exit(0);
        }
        exit(0);
    }
    waitpid(job, &status, 0);
    write(go[1], "g", 1);
    char c;
    read(done[0], &c, 1);
}

static void keys(void) {
    int fd = leader();
    int status;
    int pipe_to_job[2];
    pipe(pipe_to_job);
    pid_t job = background();
    if (job == 0) {
        handle(SIGINT);
        handle(SIGWINCH);
        signal_ready();
        char c;
        read(pipe_to_job[0], &c, 1);
        printf("job SIGINT %d SIGWINCH %d\n", got[SIGINT], got[SIGWINCH]);
        char buf[16];
        fcntl(fd, F_SETFL, O_RDWR | O_NONBLOCK);
        result("job read after flush", read(fd, buf, sizeof buf));
        fflush(stdout);
        signal_ready();
        // Stopped by the suspend key, then continued, then ended by the interrupt key.
        signal(SIGINT, SIG_DFL);
        read(pipe_to_job[0], &c, 1);
        signal_ready();
        for (;;)
            pause();
    }
    wait_ready();
    result("tcsetpgrp job", tcsetpgrp(fd, job));
    type("abc\x03");
    struct winsize ws = {.ws_row = 10, .ws_col = 20};
    result("TIOCSWINSZ", ioctl(fd, TIOCSWINSZ, &ws));
    result("TIOCSWINSZ same", ioctl(fd, TIOCSWINSZ, &ws));
    write(pipe_to_job[1], "g", 1);
    wait_ready();
    write(pipe_to_job[1], "g", 1);
    wait_ready();
    type("\x1a");
    waitpid(job, &status, WUNTRACED);
    show_wait("suspend key", status);
    kill(job, SIGCONT);
    waitpid(job, &status, WCONTINUED);
    show_wait("continued", status);
    type("\x03");
    waitpid(job, &status, 0);
    show_wait("interrupt key", status);
    // TIOCSIG from the master sends to the foreground group too.
    signal(SIGTTOU, SIG_IGN);
    tcsetpgrp(fd, getpid());
    handle(SIGINT);
    type("S");
    printf("leader SIGINT %d\n", got[SIGINT]);
}

static void hangup(void) {
    int fd = leader();
    handle(SIGHUP);
    handle(SIGCONT);
    pid_t job = background();
    if (job == 0) {
        handle(SIGHUP);
        handle(SIGCONT);
        signal_ready();
        while (!got[SIGHUP])
            pause();
        printf("job after leader ended: SIGHUP %d SIGCONT %d\n", got[SIGHUP], got[SIGCONT]);
        result("job /dev/tty", open("/dev/tty", O_RDWR));
        fflush(stdout);
        signal_ready();
        exit(0);
    }
    wait_ready();
    tcsetpgrp(fd, job);
    type("H");
    printf("leader after hangup: SIGHUP %d SIGCONT %d\n", got[SIGHUP], got[SIGCONT]);
    char buf[8];
    result("read after hangup", read(fd, buf, sizeof buf));
    result("write after hangup", write(fd, "x", 1));
    result("/dev/tty after hangup", open("/dev/tty", O_RDWR));
    result("TIOCGPGRP after hangup", tcgetpgrp(fd));
}

static void leader_exit(void) {
    int fd = leader();
    pid_t job = background();
    if (job == 0) {
        handle(SIGHUP);
        handle(SIGCONT);
        signal_ready();
        while (!got[SIGHUP])
            pause();
        printf("job after leader ended: SIGHUP %d SIGCONT %d\n", got[SIGHUP], got[SIGCONT]);
        result("job /dev/tty", open("/dev/tty", O_RDWR));
        result("job TIOCGPGRP", tcgetpgrp(fd));
        fflush(stdout);
        signal_ready();
        exit(0);
    }
    wait_ready();
    tcsetpgrp(fd, job);
}

static void notty(void) {
    int fd = leader();
    int pipe_to_job[2];
    pipe(pipe_to_job);
    pid_t job = background();
    if (job == 0) {
        handle(SIGHUP);
        handle(SIGCONT);
        signal_ready();
        char c;
        read(pipe_to_job[0], &c, 1);
        printf("job after TIOCNOTTY: SIGHUP %d SIGCONT %d\n", got[SIGHUP], got[SIGCONT]);
        result("job /dev/tty", open("/dev/tty", O_RDWR));
        fflush(stdout);
        signal_ready();
        exit(0);
    }
    wait_ready();
    tcsetpgrp(fd, job);
    handle(SIGHUP);
    result("TIOCNOTTY", ioctl(fd, TIOCNOTTY));
    printf("leader SIGHUP %d\n", got[SIGHUP]);
    result("TIOCNOTTY again", ioctl(fd, TIOCNOTTY));
    result("TIOCGPGRP", tcgetpgrp(fd));
    result("/dev/tty", open("/dev/tty", O_RDWR));
    write(pipe_to_job[1], "g", 1);
    wait_ready();
    result("TIOCSCTTY after TIOCNOTTY", ioctl(fd, TIOCSCTTY, 0));
    kill(job, SIGKILL);
}

// Runs a scenario in a new session leader, typing into the master what it asks for ('H' closes
// the master, 'S' sends SIGINT with TIOCSIG), and shows what the master read.
static void scenario(const char *label, void (*body)(void)) {
    printf("== %s\n", label);
    fflush(stdout);
    open_pair();
    pipe(ask);
    pipe(ready);
    pid_t child = fork();
    if (child == 0) {
        close(ask[0]);
        close(master);
        body();
        fflush(stdout);
        exit(0);
    }
    close(ask[1]);
    // What is asked for at once is typed in one write.
    // Typed keys are echoed after they have been acted on, so the echo says they were.
    char text[64], buf[256];
    long asked, seen = 0;
    while ((asked = read(ask[0], text, sizeof text)) > 0) {
        if (text[0] == 'H')
            close(master);
        else if (text[0] == 'S')
            ioctl(master, TIOCSIG, SIGINT);
        else {
            write(master, text, asked);
            long n = read(master, buf + seen, sizeof buf - seen);
            if (n > 0)
                seen += n;
        }
        write(ready[1], "t", 1);
    }
    // Every process of the scenario has ended before the next one starts.
    int status;
    while (waitpid(-1, &status, 0) > 0) {
    }
    fcntl(master, F_SETFL, O_RDWR | O_NONBLOCK);
    long n = read(master, buf + seen, sizeof buf - seen);
    if (n > 0)
        seen += n;
    n = seen ? seen : n;
    printf("master read: %ld \"", n);
    for (long i = 0; i < n; i++)
        printf(buf[i] >= 32 && buf[i] < 127 ? "%c" : "\\x%02x", (unsigned char)buf[i]);
    printf("\"\n");
    fflush(stdout);
    close(master);
    close(ask[0]);
    close(ready[0]);
    close(ready[1]);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    scenario("sessions", sessions);
    scenario("background jobs", background_jobs);
    scenario("keys", keys);
    scenario("hangup", hangup);
    scenario("leader exit", leader_exit);
    scenario("notty", notty);
    return 0;
}
