// For each loadable segment, prints its permissions, how much larger it is in memory than in the file,
// and the bytes from the end of its file part to the end of that page: how many, how many are not zero,
// and their sum. No C library.

#include <elf.h>

#include "bare.h"

// Data that ends part way into a page, followed by zeroed data, so the writable segment has both.
static volatile char data[100] = {1};
static volatile char zeroed[10000];

void start(long *stack) {
    long *at = stack + 1 + stack[0] + 1;
    while (*at) {
        at++;
    }
    const Elf64_Phdr *headers = 0;
    long count = 0;
    for (at++; at[0] != AT_NULL; at += 2) {
        if (at[0] == AT_PHDR) {
            headers = (const Elf64_Phdr *)at[1];
        } else if (at[0] == AT_PHNUM) {
            count = at[1];
        }
    }
    unsigned long base = 0;
    for (long index = 0; index < count; index++) {
        if (headers[index].p_type == PT_PHDR) {
            base = (unsigned long)headers - headers[index].p_vaddr;
        }
    }
    for (long index = 0; index < count; index++) {
        const Elf64_Phdr *segment = &headers[index];
        if (segment->p_type != PT_LOAD) {
            continue;
        }
        unsigned long file_end = base + segment->p_vaddr + segment->p_filesz;
        unsigned long page_end = (file_end + 4095) & ~4095ul;
        long nonzero = 0, sum = 0;
        for (unsigned long byte = file_end; byte < page_end; byte++) {
            unsigned char value = *(volatile unsigned char *)byte;
            nonzero += value != 0;
            sum += value;
        }
        print("flags ");
        print_number(segment->p_flags);
        print("extra ");
        print_number(segment->p_memsz - segment->p_filesz);
        print("tail ");
        print_number(page_end - file_end);
        print("nonzero ");
        print_number(nonzero);
        print("sum ");
        print_number(sum);
    }
    exit_group(data[0] + zeroed[0] - 1);
}
