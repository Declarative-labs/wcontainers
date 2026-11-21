// Calls blocked when their process is stopped and continued go on as if nothing happened: a pipe
// read, a relative sleep, a futex wait and a wait for a child.
#define _GNU_SOURCE
#include <linux/futex.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static int in[2], out[2];
static uint32_t word;

static void *sleeper(void *arg) {
    (void)arg;
    struct timespec ts = {0, 50000000};
    long r = nanosleep(&ts, NULL);
    dprintf(out[1], "sleep %ld\n", r);
    return NULL;
}
static void *futexer(void *arg) {
    (void)arg;
    long r = syscall(SYS_futex, &word, FUTEX_WAIT_PRIVATE, 0, NULL, NULL, 0);
    dprintf(out[1], "futex %ld\n", r);
    return NULL;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    pipe(in);
    pipe(out);
    pid_t child = fork();
    if (child == 0) {
        pthread_t a, b;
        pthread_create(&a, NULL, sleeper, NULL);
        pthread_create(&b, NULL, futexer, NULL);
        char c;
        long r = read(in[0], &c, 1);
        word = 1;
        syscall(SYS_futex, &word, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0);
        pthread_join(a, NULL);
        pthread_join(b, NULL);
        dprintf(out[1], "read %ld\n", r);
        _exit(0);
    }
    close(out[1]);
    usleep(10000);
    for (int i = 0; i < 5; i++) {
        kill(child, SIGSTOP);
        int status;
        waitpid(child, &status, WUNTRACED);
        usleep(20000);
        kill(child, SIGCONT);
        usleep(5000);
    }
    write(in[1], "x", 1);
    char buf[256];
    ssize_t n, total = 0;
    while ((n = read(out[0], buf + total, sizeof buf - 1 - total)) > 0) total += n;
    buf[total] = 0;
    int status;
    waitpid(child, &status, 0);
    // Lines can come in any order; print them sorted by first letter.
    const char *order = "frs";
    for (const char *o = order; *o; o++)
        for (char *line = buf; *line; ) {
            char *end = line;
            while (*end && *end != '\n') end++;
            if (*line == *o) printf("%.*s\n", (int)(end - line), line);
            line = *end ? end + 1 : end;
        }
    printf("exit %d\n", WEXITSTATUS(status));
    return 0;
}
