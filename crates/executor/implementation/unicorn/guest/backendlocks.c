// Appends from one thread while another truncates the same backend file by descriptor and by path.
// Linux holds the inode across each append and each truncation, so an append lands at the end the
// file has after any truncation: the file holds whole records and never a hole of zero bytes.
#define _GNU_SOURCE
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static const char record[] = "abcdefg\n";

static void *appender(void *arg) {
    int fd = open("appended", O_WRONLY | O_APPEND);
    for (int i = 0; i < 200; i++) write(fd, record, 8);
    close(fd);
    return 0;
}

static void *truncator(void *arg) {
    int fd = open("appended", O_WRONLY);
    for (int i = 0; i < 60; i++) {
        if (i % 2) {
            ftruncate(fd, 0);
        } else {
            truncate("appended", 0);
        }
    }
    close(fd);
    return 0;
}

int main(void) {
    close(open("appended", O_CREAT | O_TRUNC | O_WRONLY, 0644));
    pthread_t a, b;
    pthread_create(&a, 0, appender, 0);
    pthread_create(&b, 0, truncator, 0);
    pthread_join(a, 0);
    pthread_join(b, 0);
    char buf[2000];
    int fd = open("appended", O_RDONLY);
    int n = read(fd, buf, sizeof buf), zeros = 0, broken = 0;
    close(fd);
    for (int i = 0; i < n; i++) zeros += buf[i] == 0;
    for (int i = 0; i + 8 <= n; i += 8) broken += memcmp(buf + i, record, 8) != 0;
    printf("whole records %s, zero bytes %d, broken records %d\n", n % 8 ? "no" : "yes", zeros,
           broken);
    return 0;
}
