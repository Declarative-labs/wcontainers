// C++ exceptions: thrown from every depth, caught by type, rethrown, unwinding destructors on the way. The
// stand-in kernel has no mmap, so the program brings its own allocator over a static area.
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <string>

extern "C" {
alignas(16) static unsigned char area[1 << 20];
static std::size_t used;

// Each block has its size in the 16 bytes before it.
void *malloc(std::size_t n) {
    std::size_t need = 16 + ((n + 15) & ~std::size_t(15));
    if (n > sizeof area || used + need > sizeof area) return nullptr;
    unsigned char *block = area + used + 16;
    std::memcpy(block - 16, &n, sizeof n);
    used += need;
    return block;
}
void free(void *) {}
void *calloc(std::size_t count, std::size_t n) {
    if (n != 0 && count > sizeof area / n) return nullptr;
    void *p = malloc(count * n);
    if (p) std::memset(p, 0, count * n);
    return p;
}
std::size_t malloc_usable_size(void *p) {
    std::size_t n;
    std::memcpy(&n, static_cast<unsigned char *>(p) - 16, sizeof n);
    return n;
}
void *realloc(void *p, std::size_t n) {
    void *moved = malloc(n);
    if (p && moved) {
        std::size_t old = malloc_usable_size(p);
        std::memcpy(moved, p, old < n ? old : n);
    }
    return moved;
}
void *aligned_alloc(std::size_t, std::size_t n) { return malloc(n); }
void *memalign(std::size_t, std::size_t n) { return malloc(n); }
int posix_memalign(void **out, std::size_t, std::size_t n) {
    *out = malloc(n);
    return *out ? 0 : 12;
}
}

static int unwound = 0;

struct Guard {
    int n;
    ~Guard() { unwound += n; }
};

__attribute__((noinline)) static int dive(int n, int kind) {
    Guard guard{n};
    if (n == 0) {
        if (kind == 0) throw std::runtime_error("bottom");
        if (kind == 1) throw std::out_of_range(std::to_string(kind));
        if (kind == 2) throw 42;
        return 1;
    }
    if (n == 5) {
        try {
            return dive(n - 1, kind) + 1;
        } catch (const std::logic_error &e) {
            std::printf("caught %s at 5\n", e.what());
            throw;
        }
    }
    return dive(n - 1, kind) + 1;
}

int main() {
    int caught = 0;
    for (int i = 0; i < 32; i++) {
        try {
            std::printf("returned %d\n", dive(i % 9, i % 4));
        } catch (const std::runtime_error &e) {
            caught += 1;
            std::printf("%s %d\n", e.what(), i);
        } catch (const std::exception &e) {
            caught += 10;
            std::printf("exception %s %d\n", e.what(), i);
        } catch (int x) {
            caught += x;
            std::printf("int %d %d\n", x, i);
        }
    }
    try {
        try {
            throw 41;
        } catch (int x) {
            throw x + 1;
        }
    } catch (int y) {
        std::printf("%d\n", y);
    }
    std::printf("caught %d, unwound %d\n", caught, unwound);
    return 0;
}
