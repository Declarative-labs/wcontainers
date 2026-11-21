// execveat: its refusals and checks, then a chain of programs started by path, by a directory
// descriptor, by the program's own descriptor and by a script's descriptor. Each stage prints its
// arguments, its name and its program; the last argument says which stage it is. The chain ends
// with a copy of the program in a memory file whose own descriptor is open for writing.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef AT_EXECVE_CHECK
#define AT_EXECVE_CHECK 0x10000
#endif

extern char **environ;

static long execveat_(int dirfd, const char *path, char *const argv[], int flags) {
    errno = 0;
    long r = syscall(SYS_execveat, dirfd, path, argv, environ, flags);
    return r < 0 ? -errno : r;
}

static void show(int argc, char **argv) {
    char comm[32] = "";
    int fd = open("/proc/self/comm", O_RDONLY);
    long n = read(fd, comm, sizeof comm - 1);
    close(fd);
    if (n > 0) comm[n - 1] = 0;
    printf("argc %d:", argc);
    for (int i = 0; i < argc; i++) printf(" [%s]", argv[i]);
    char exe[256] = "";
    readlink("/proc/self/exe", exe, sizeof exe - 1);
    printf(" comm [%s] exe [%s]\n", comm, exe);
}

int main(int argc, char **argv) {
    setvbuf(stdout, 0, _IONBF, 0);
    const char *stage = argc > 1 ? argv[argc - 1] : "";
    show(argc, argv);
    if (!strcmp(stage, "")) {
        char *one[] = {"x", "one", 0};
        int directory = open("/wc", O_PATH | O_DIRECTORY);
        int passwd = open("/etc/passwd", O_RDONLY);
        printf("bad flags %ld\n", execveat_(AT_FDCWD, "/wc/execveat", one, 0x2));
        printf("empty %ld\n", execveat_(AT_FDCWD, "", one, 0));
        printf("empty cwd %ld\n", execveat_(AT_FDCWD, "", one, AT_EMPTY_PATH));
        printf("bad fd %ld\n", execveat_(99, "execveat", one, 0));
        printf("not a directory %ld\n", execveat_(passwd, "execveat", one, 0));
        printf("not executable %ld\n", execveat_(passwd, "", one, AT_EMPTY_PATH));
        printf("missing %ld\n", execveat_(directory, "nope", one, 0));
        symlink("/wc/execveat", "/tmp/link");
        printf("link no follow %ld\n", execveat_(AT_FDCWD, "/tmp/link", one, AT_SYMLINK_NOFOLLOW));
        printf("check %ld\n", execveat_(AT_FDCWD, "/wc/execveat", one, AT_EXECVE_CHECK));
        printf("check link %ld\n", execveat_(AT_FDCWD, "/tmp/link", one, AT_EXECVE_CHECK));
        printf("check passwd %ld\n", execveat_(AT_FDCWD, "/etc/passwd", one, AT_EXECVE_CHECK));
        printf("check directory %ld\n", execveat_(directory, "", one, AT_EMPTY_PATH | AT_EXECVE_CHECK));
        printf("check bad argv %ld\n", execveat_(AT_FDCWD, "/wc/execveat", (char **)8, AT_EXECVE_CHECK));
        printf("path fault %ld\n", execveat_(AT_FDCWD, (char *)8, one, 0));
        close(passwd);
        close(directory);
        // An absolute path ignores the descriptor.
        printf("by path %ld\n", execveat_(99, "/wc/execveat", one, 0));
    } else if (!strcmp(stage, "one")) {
        char *two[] = {"x", "two", 0};
        int directory = open("/wc", O_PATH | O_DIRECTORY);
        printf("by directory %ld\n", execveat_(directory, "execveat", two, 0));
    } else if (!strcmp(stage, "two")) {
        char *three[] = {"x", "three", 0};
        int self = open("/wc/execveat", O_RDONLY);
        printf("by descriptor %ld\n", execveat_(self, "", three, AT_EMPTY_PATH));
    } else if (!strcmp(stage, "three")) {
        char *four[] = {"x", "four", 0};
        int script = open("/tmp/script.sh", O_WRONLY | O_CREAT | O_TRUNC, 0755);
        write(script, "#!/wc/execveat\n", 15);
        close(script);
        printf("check script %ld\n", execveat_(AT_FDCWD, "/tmp/script.sh", four, AT_EXECVE_CHECK));
        script = open("/tmp/script.sh", O_RDONLY | O_CLOEXEC);
        printf("script closed at exec %ld\n", execveat_(script, "", four, AT_EMPTY_PATH));
        int tmp = open("/tmp", O_PATH | O_DIRECTORY | O_CLOEXEC);
        printf("script in directory closed at exec %ld\n", execveat_(tmp, "script.sh", four, 0));
        close(script);
        script = open("/tmp/script.sh", O_RDONLY);
        printf("script %ld\n", execveat_(script, "", four, AT_EMPTY_PATH));
    } else if (!strcmp(stage, "four")) {
        char *five[] = {"x", "five", 0};
        int program = open("/wc/execveat", O_RDONLY);
        int memory = memfd_create("program", 0);
        char chunk[65536];
        long n;
        while ((n = read(program, chunk, sizeof chunk)) > 0)
            write(memory, chunk, n);
        close(program);
        printf("memory file %ld\n", execveat_(memory, "", five, AT_EMPTY_PATH));
    }
    return 0;
}
