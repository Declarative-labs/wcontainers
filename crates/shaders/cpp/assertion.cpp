// A converter's failed assertion: an exception the boundary turns into a refused conversion where C++
// exceptions are on; otherwise the message and the end of the module instance.

#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>

[[noreturn]] void wcs_assert_failed(const char* expression, const char* file, int line) {
#if defined(__cpp_exceptions) || defined(_CPPUNWIND)
  throw std::runtime_error(std::string("assertion `") + expression + "` at " + file + ":" +
                           std::to_string(line));
#else
  std::fprintf(stderr, "assertion `%s` failed at %s:%d\n", expression, file, line);
  std::abort();
#endif
}
