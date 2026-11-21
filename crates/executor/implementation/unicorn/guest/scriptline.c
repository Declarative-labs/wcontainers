// How execve splits a script's first line when it cannot start it: lines with no interpreter, an
// empty one, or one cut short. Prints each line and what execve gave.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static void attempt(const char *text, size_t length) {
    int fd = open("/tmp/s", O_WRONLY | O_CREAT | O_TRUNC, 0755);
    write(fd, text, length);
    close(fd);
    char *arguments[] = {"/tmp/s", NULL};
    char *environment[] = {NULL};
    errno = 0;
    execve("/tmp/s", arguments, environment);
    printf("%zu bytes:", length);
    for (size_t i = 0; i < length && i < 40; i++) {
        unsigned char c = text[i];
        printf(c >= 32 && c < 127 ? "%c" : "\\x%02x", c);
    }
    printf(" = %s\n", strerror(errno));
}

#define TRY(text) attempt(text, sizeof(text) - 1)

int main(void) {
    TRY("#!");
    TRY("#!   ");
    TRY("#!\t \t");
    TRY("#!\n");
    TRY("#! \t\n/bin/sh\n");
    TRY("#!\0/bin/sh\n");
    TRY("#! \0/bin/sh\n");
    TRY("#!/nonexistent\0/bin/sh\n");
    TRY("#!/nonexistent argument\0more\n");
    TRY("#!/bin\n");
    TRY("#!/tmp\0\n");
    // A name that fills the first 256 bytes without ending is cut short.
    char long_name[300];
    memset(long_name, 'a', sizeof(long_name));
    memcpy(long_name, "#!/", 3);
    attempt(long_name, sizeof(long_name));
    long_name[200] = '\0';
    attempt(long_name, sizeof(long_name));
    long_name[200] = 'a';
    long_name[255] = ' ';
    attempt(long_name, sizeof(long_name));
    long_name[255] = 'a';
    long_name[256] = ' ';
    attempt(long_name, sizeof(long_name));
    // Only blanks in the first 256 bytes.
    memset(long_name + 2, ' ', sizeof(long_name) - 2);
    attempt(long_name, sizeof(long_name));
    return 0;
}
