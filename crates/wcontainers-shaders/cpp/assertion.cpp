// A converter's failed assertion: its message on standard error, then the end of the module
// instance, which the host reports as a refused conversion with the message.

#include <cstdio>
#include <cstdlib>

[[noreturn]] void wcs_assert_failed(const char* expression, const char* file, int line) {
  std::fprintf(stderr, "assertion `%s` failed at %s:%d\n", expression, file, line);
  std::abort();
}
