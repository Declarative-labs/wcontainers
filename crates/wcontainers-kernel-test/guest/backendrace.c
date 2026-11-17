// Calls that wait for the backend while another thread changes their arguments: Linux takes a
// call's descriptor (fdget) and copies its path (getname) once, so each call acts once, on what it
// named when it started. Also a thread that spins while another waits for the backend.
#define _GNU_SOURCE
#include <dirent.h>
#include <fcntl.h>
#include <pthread.h>
#include <sched.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int fd_memory, fd_backend;
static volatile int stop;

static void *flip_descriptor(void *arg) {
    while (!stop) {
        dup2(fd_memory, 10);
        dup2(fd_backend, 10);
    }
    return 0;
}

static void pwrite_while_flipped(void) {
    fd_memory = open("/dev/shm/race", O_RDWR | O_CREAT | O_TRUNC, 0600);
    fd_backend = open("/data/race", O_RDWR | O_CREAT | O_TRUNC, 0600);
    dup2(fd_backend, 10);
    pthread_t thread;
    pthread_create(&thread, 0, flip_descriptor, 0);
    int ok = 0;
    for (int i = 0; i < 400; i++) ok += pwrite(10, "a", 1, i) == 1;
    stop = 1;
    pthread_join(thread, 0);
    char memory[400] = {0}, backend[400] = {0};
    pread(fd_memory, memory, sizeof memory, 0);
    pread(fd_backend, backend, sizeof backend, 0);
    int both = 0, neither = 0;
    for (int i = 0; i < 400; i++) {
        both += memory[i] == 'a' && backend[i] == 'a';
        neither += memory[i] != 'a' && backend[i] != 'a';
    }
    printf("pwrite: ok %d, offsets written to both files %d, to neither %d\n", ok, both, neither);
    close(fd_memory);
    close(fd_backend);
    close(10);
    unlink("/dev/shm/race");
    unlink("/data/race");
}

static char name[] = "/data/dA0000";

static void *flip_name(void *arg) {
    while (!stop) {
        name[7] = name[7] == 'A' ? 'B' : 'A';
        sched_yield();
    }
    return 0;
}

static void mkdir_while_flipped(void) {
    stop = 0;
    pthread_t thread;
    pthread_create(&thread, 0, flip_name, 0);
    int made = 0;
    for (int i = 0; i < 20; i++) {
        name[10] = '0' + i / 10;
        name[11] = '0' + i % 10;
        made += mkdir(name, 0700) == 0;
    }
    stop = 1;
    pthread_join(thread, 0);
    int found = 0;
    DIR *dir = opendir("/data");
    struct dirent *entry;
    while ((entry = readdir(dir))) {
        if (strncmp(entry->d_name, "dA00", 4) == 0 || strncmp(entry->d_name, "dB00", 4) == 0) {
            found++;
            char path[64];
            snprintf(path, sizeof path, "/data/%s", entry->d_name);
            rmdir(path);
        }
    }
    closedir(dir);
    printf("mkdir: successful calls %d, directories made %s\n", made,
           made == found ? "the same" : "differ");
}

static volatile int ready;

static void *read_file(void *arg) {
    char buf[16] = {0};
    int in = open("/data/spin", O_RDONLY);
    read(in, buf, sizeof buf - 1);
    close(in);
    printf("spin: read %s\n", buf);
    ready = 1;
    return 0;
}

static void spin_while_waiting(void) {
    int out = open("/data/spin", O_WRONLY | O_CREAT | O_TRUNC, 0600);
    write(out, "hello", 5);
    close(out);
    pthread_t thread;
    pthread_create(&thread, 0, read_file, 0);
    while (!ready) {
    }
    pthread_join(thread, 0);
    unlink("/data/spin");
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    pwrite_while_flipped();
    mkdir_while_flipped();
    spin_while_waiting();
    return 0;
}
