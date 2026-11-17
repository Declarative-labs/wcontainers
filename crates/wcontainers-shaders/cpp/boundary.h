// What both converter boundaries share: copying a reason out, keeping dxbc-spirv's errors as the
// reason, and keeping a converter's failure (an exception or an assertion, which `cpp/assert` turns
// into one) inside the call as an error.

#pragma once

#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>

#include "util/util_log.h"

namespace wcs {

// Keeps the first error dxbc-spirv reports on this thread while it lives, as the reason a call failed.
class ErrorLog : public dxbc_spv::util::Logger {
public:
  void message(dxbc_spv::util::LogLevel severity, const char* text) override {
    if (severity == dxbc_spv::util::LogLevel::eError && first.empty())
      first = text;
  }
  dxbc_spv::util::LogLevel getMinimumSeverity() override { return dxbc_spv::util::LogLevel::eError; }
  std::string first;
};

inline char* copy(const std::string& s) {
  char* out = static_cast<char*>(std::malloc(s.size() + 1));
  if (out)
    std::memcpy(out, s.c_str(), s.size() + 1);
  return out;
}

// Runs `f`, returning its status. Where C++ exceptions are on (native builds), an exception leaving the
// converter becomes status 3 with its text in `error`; without them (WebAssembly) it ends the module
// instance, which the host reports as a failed conversion.
template <typename F>
int contained(const char* who, char** error, F&& f) {
#if defined(__cpp_exceptions) || defined(_CPPUNWIND)
  auto fail = [&](const std::string& what) {
    std::free(*error);
    *error = copy(std::string(who) + " failed: " + what);
    return 3;
  };
  try {
    return f();
  } catch (const std::exception& e) {
    return fail(e.what());
  } catch (...) {
    return fail("an exception of unknown type");
  }
#else
  (void)who;
  (void)error;
  return f();
#endif
}

}
