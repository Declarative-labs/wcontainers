// What both converter boundaries share: the slots every entry fills, copying a reason out, and
// keeping dxbc-spirv's errors as the reason. Both are built without C++ exceptions: a converter's
// failure (an assertion, `cpp/assert`, or a throw) ends its module instance, which the host reports
// as a refused conversion.

#pragma once

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <string>

#include "util/util_log.h"

// Declares a boundary function, exported from the converters' module under its own name.
#define WCS_EXPORT(name) extern "C" __attribute__((export_name(#name)))

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

// The words every boundary function fills, given as its last argument: two buffers it allocated with
// malloc (free each with wcs_free) and their element counts, and the reason a call failed (a
// NUL-terminated string, also from malloc) and its length. An entry giving one buffer leaves the
// second empty.
enum class Slot : uint32_t {
  eFirst,
  eFirstCount,
  eSecond,
  eSecondCount,
  eReason,
  eReasonLength,
  eCount,
};

// Writes an entry's buffers and reason into `slots`.
inline void fill(uintptr_t* slots, const void* first, size_t first_count, const void* second,
                 size_t second_count, const char* reason) {
  slots[uint32_t(Slot::eFirst)] = reinterpret_cast<uintptr_t>(first);
  slots[uint32_t(Slot::eFirstCount)] = first_count;
  slots[uint32_t(Slot::eSecond)] = reinterpret_cast<uintptr_t>(second);
  slots[uint32_t(Slot::eSecondCount)] = second_count;
  slots[uint32_t(Slot::eReason)] = reinterpret_cast<uintptr_t>(reason);
  slots[uint32_t(Slot::eReasonLength)] = reason ? std::strlen(reason) : 0;
}

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

}
