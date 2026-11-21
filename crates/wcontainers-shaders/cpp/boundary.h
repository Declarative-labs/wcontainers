// What both converter boundaries share: copying a reason out, keeping dxbc-spirv's errors as the
// reason, and keeping a converter's failure (an exception or an assertion, which `cpp/assert` turns
// into one) inside the call as an error.

#pragma once

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>

#include "util/util_log.h"

namespace wcs {

// A resource class of Direct3D 12, which the DXIL boundary gives as each resource's descriptor set.
enum class ResourceClass : uint32_t {
  eConstants,
  eView,
  eStorage,
  eSampler,
  // An unordered access view's counter, at its view's binding.
  eCounter,
};

// The words of one entry of a table a boundary returns: a resource's (class, space, register), a
// constant buffer's (space, register, size in bytes).
enum class TableWord : uint32_t {
  eFirst,
  eSecond,
  eThird,
  eCount,
};

// The words of one view the DXIL boundary lists from the resource list: its list (0 the shader resource
// views, 1 the unordered access views), whether its fields could be read, its `ViewShape`, space,
// first register and range size, and its `ViewValue`.
enum class ViewWord : uint32_t {
  eList,
  eReadable,
  eShape,
  eSpace,
  eLowerBound,
  eRangeSize,
  eValue,
  eCount,
};

// What a view's resource kind (`DXIL::ResourceKind`) is: a texture of a kind other than a multisampled
// array, a multisampled array texture, a typed buffer, or another resource.
enum class ViewShape : uint32_t {
  eTexture,
  eMultisampledArray,
  eTypedBuffer,
  eOther,
};

// The range size a view's `eRangeSize` word gives a range without one (DXIL writes -1).
enum class ViewRange : uint32_t {
  eUnbounded = 0xffffffffu,
};

// The kind of value a view's elements hold, from the element type (`DXIL::ComponentType`) its extended
// properties give; none where they give none or one of no such kind.
enum class ViewValue : uint32_t {
  eNone,
  eFloat,
  eSint,
  eUint,
};

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
