// How execve fails on ELF programs and the loaders they name: before the point of no return the
// caller gets the error; after it the process dies of SIGSEGV. Also the room for arguments, and a
// start with no arguments. Run with no argument for the errors, "fatal" for a loader that cannot be
// placed, "empty" for a start with no arguments.

#define _GNU_SOURCE
#include <elf.h>
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <unistd.h>

static void put(const char *path, const void *bytes, size_t length, mode_t mode) {
    unlink(path);
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, mode);
    if (fd < 0 || write(fd, bytes, length) != (ssize_t)length) {
        perror(path);
        exit(1);
    }
    close(fd);
    chmod(path, mode);
}

// A program: its file header, up to four program headers, then the rest.
struct file {
    Elf64_Ehdr header;
    Elf64_Phdr segments[4];
    unsigned char rest[0x400];
};

static void program(struct file *file, uint16_t type, int count) {
    memset(file, 0, sizeof *file);
    memcpy(file->header.e_ident, ELFMAG, SELFMAG);
    file->header.e_ident[EI_CLASS] = ELFCLASS64;
    file->header.e_ident[EI_DATA] = ELFDATA2LSB;
    file->header.e_ident[EI_VERSION] = EV_CURRENT;
    file->header.e_type = type;
    file->header.e_machine = EM_AARCH64;
    file->header.e_version = EV_CURRENT;
    file->header.e_entry = 0x400000 + offsetof(struct file, rest) + 0x300;
    file->header.e_phoff = offsetof(struct file, segments);
    file->header.e_ehsize = sizeof(Elf64_Ehdr);
    file->header.e_phentsize = sizeof(Elf64_Phdr);
    file->header.e_phnum = count;
}

// The loadable segment every program here has.
static void load(Elf64_Phdr *segment, uint64_t address) {
    segment->p_type = PT_LOAD;
    segment->p_flags = PF_R | PF_X;
    segment->p_offset = 0;
    segment->p_vaddr = address;
    segment->p_paddr = address;
    segment->p_filesz = sizeof(struct file);
    segment->p_memsz = sizeof(struct file);
    segment->p_align = 0x1000;
}

// The loader's name, at the start of `rest`.
static void interp(Elf64_Phdr *segment, uint64_t size) {
    segment->p_type = PT_INTERP;
    segment->p_offset = offsetof(struct file, rest);
    segment->p_filesz = size;
}

static void run(const char *label) {
    char *arguments[] = {"m", 0};
    char *environment[] = {0};
    errno = 0;
    execve("/tmp/m", arguments, environment);
    printf("%-40s errno %d (%s)\n", label, errno, strerror(errno));
}

// A program naming /tmp/i as its loader, and a loader made by `make`.
static void with_loader(const char *label, const void *loader, size_t length, mode_t mode) {
    struct file main;
    program(&main, ET_EXEC, 2);
    load(&main.segments[0], 0x400000);
    interp(&main.segments[1], 7);
    memcpy(main.rest, "/tmp/i", 7);
    put("/tmp/m", &main, sizeof main, 0755);
    if (loader) {
        put("/tmp/i", loader, length, mode);
    } else {
        unlink("/tmp/i");
    }
    run(label);
}

static void errors(void) {
    struct file main, loader;
    with_loader("no loader", 0, 0, 0);
    with_loader("loader is a script", "#!/bin/sh\n", 10, 0755);
    with_loader("loader of 5 bytes", "\177ELF\2", 5, 0755);
    with_loader("loader not readable", "\177ELF\2", 5, 0111);
    program(&loader, ET_DYN, 1);
    load(&loader.segments[0], 0);
    with_loader("loader is mode 0644", &loader, sizeof loader, 0644);
    with_loader("loader is only its header", &loader, sizeof(Elf64_Ehdr), 0755);
    unsigned char zeros[64] = {0};
    with_loader("loader of zeros", zeros, sizeof zeros, 0755);
    loader.header.e_machine = EM_X86_64;
    with_loader("loader for x86-64", &loader, sizeof loader, 0755);
    program(&loader, ET_DYN, 0);
    with_loader("loader with no program headers", &loader, sizeof loader, 0755);
    program(&loader, ET_DYN, 1);
    loader.header.e_phentsize = 32;
    with_loader("loader with short program headers", &loader, sizeof loader, 0755);
    program(&loader, ET_DYN, 1);
    loader.header.e_ident[EI_CLASS] = ELFCLASS32;
    loader.header.e_ident[EI_DATA] = ELFDATA2MSB;
    loader.header.e_version = 0;
    load(&loader.segments[0], 0);
    loader.segments[0].p_type = PT_NOTE;
    loader.header.e_type = 99;
    // The kind, class, byte order and version of a loader are checked only after the point of no
    // return, so with a bad property note this one fails before it.
    loader.segments[0].p_type = PT_GNU_PROPERTY;
    loader.segments[0].p_offset = 0x10000;
    loader.segments[0].p_filesz = 32;
    with_loader("loader of another kind, bad note", &loader, sizeof loader, 0755);

    // The program's own loader name.
    uint64_t sizes[] = {0, 1, 4097, 7};
    uint64_t offsets[] = {0x200, 0x200, 0x200, 0x100000, 1ULL << 63, 0x7fffffffffffffffULL};
    for (int kind = 0; kind < 9; kind++) {
        program(&main, ET_DYN, 2);
        load(&main.segments[0], 0);
        interp(&main.segments[1], 7);
        memcpy(main.rest, "/tmp/i", 7);
        if (kind < 3) {
            main.segments[1].p_filesz = sizes[kind];
        } else if (kind < 6) {
            main.segments[1].p_offset = offsets[kind];
        } else if (kind == 6) {
            main.rest[6] = 'x';
        } else if (kind == 7) {
            main.segments[1].p_filesz = 2;
            main.rest[0] = 0;
            main.rest[1] = 0;
        } else {
            main.rest[3] = 0;
        }
        put("/tmp/m", &main, sizeof main, 0755);
        unlink("/tmp/i");
        char label[64];
        snprintf(label, sizeof label, "loader name %d", kind);
        run(label);
    }

    // The program itself.
    for (int kind = 0; kind < 14; kind++) {
        program(&main, ET_EXEC, 1);
        load(&main.segments[0], 0x400000);
        switch (kind) {
        case 0: main.header.e_ident[0] = 0; break;
        case 1: main.header.e_type = ET_REL; break;
        case 2: main.header.e_type = ET_CORE; break;
        case 3: main.header.e_machine = EM_ARM; break;
        case 4: main.header.e_phentsize = 0; break;
        case 5: main.header.e_phnum = 0; break;
        case 6: main.header.e_phnum = 1171; break;
        case 7: main.header.e_phoff = 0x100000; break;
        // Not checked: the class, byte order and version. With a missing loader named, ENOENT.
        case 8:
            main.header.e_ident[EI_CLASS] = ELFCLASS32;
            main.header.e_ident[EI_DATA] = ELFDATA2MSB;
            main.header.e_ident[EI_VERSION] = 0;
            main.header.e_version = 0;
            main.header.e_phnum = 2;
            interp(&main.segments[1], 7);
            memcpy(main.rest, "/tmp/i", 7);
            unlink("/tmp/i");
            break;
        // Property notes: too large, short, the wrong kind, and properties out of order, cut
        // short or of the wrong size.
        default: {
            main.header.e_phnum = 2;
            Elf64_Phdr *note = &main.segments[1];
            note->p_type = PT_GNU_PROPERTY;
            note->p_offset = offsetof(struct file, rest);
            note->p_filesz = 48;
            uint32_t *words = (uint32_t *)main.rest;
            words[0] = 4;
            words[1] = 32;
            words[2] = NT_GNU_PROPERTY_TYPE_0;
            memcpy(&words[3], "GNU", 4);
            words[4] = 0xc0000000;
            words[5] = 4;
            words[6] = 3;
            words[8] = 0xc0000002;
            words[9] = 4;
            if (kind == 9) note->p_filesz = 1025;
            if (kind == 10) note->p_filesz = 15;
            if (kind == 11) words[2] = 1;
            if (kind == 12) words[8] = 0xc0000000;
            if (kind == 13) words[5] = 8;
            break;
        }
        }
        put("/tmp/m", &main, sizeof main, 0755);
        char label[64];
        snprintf(label, sizeof label, "program %d", kind);
        run(label);
    }

    // The room for arguments: a quarter of the stack limit, at least 32 pages, less a pointer each.
    put("/tmp/m", "#!/nowhere\n", 11, 0755);
    struct rlimit limit = {256 << 10, RLIM_INFINITY};
    setrlimit(RLIMIT_STACK, &limit);
    static char *many[20000];
    char *environment[] = {0};
    for (int kind = 0; kind < 6; kind++) {
        size_t count = (size_t[]){16384, 16383, 127, 127, 1, 1}[kind];
        size_t length = (size_t[]){0, 0, 1023, 1024, 131072 - 16, 131072 - 15}[kind];
        static char string[200000];
        memset(string, 'b', length);
        string[length] = 0;
        for (size_t at = 0; at < count; at++) {
            many[at] = string;
        }
        many[count] = 0;
        errno = 0;
        execve("/tmp/m", many, environment);
        printf("%zu arguments of %zu bytes: errno %d (%s)\n", count, length, errno, strerror(errno));
    }
}

int main(int argc, char **argv) {
    if (argc == 1 && argv[0][0] == 0) {
        // Started again with no arguments.
        char line[64] = {0};
        int fd = open("/proc/self/cmdline", O_RDONLY);
        ssize_t got = read(fd, line, sizeof line);
        printf("restarted argc=%d argv0='%s' execfn=%s cmdline %zd bytes, first %d\n", argc, argv[0],
               (char *)getauxval(AT_EXECFN), got, line[0]);
        return 0;
    }
    setvbuf(stdout, 0, _IONBF, 0);
    if (argc > 1 && strcmp(argv[1], "empty") == 0) {
        char *none[] = {0};
        execve("/proc/self/exe", none, none);
        printf("execve: %s\n", strerror(errno));
        return 1;
    }
    if (argc > 1 && strcmp(argv[1], "fatal") == 0) {
        // A loader that is a fixed program at address 0 cannot be placed, after the point of no
        // return.
        struct file loader;
        program(&loader, ET_EXEC, 1);
        load(&loader.segments[0], 0);
        with_loader("fatal", &loader, sizeof loader, 0755);
        return 1;
    }
    errors();
    return 0;
}
