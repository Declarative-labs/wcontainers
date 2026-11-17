// The one boundary to dxil-spirv: DXIL bytes in, SPIR-V words out.

#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>

#include "boundary.h"
#include "dxbc/dxbc_container.h"
#include "dxil_spirv_c.h"

namespace {

// Keeps the first error dxil-spirv reports, as the reason a conversion failed.
void log_error(void* userdata, dxil_spv_log_level level, const char* text) {
  auto* first = static_cast<std::string*>(userdata);
  if (level == DXIL_SPV_LOG_LEVEL_ERROR && first->empty())
    *first = text;
}

// Direct3D 12 takes a container whose checksum matches its bytes (the validator's signature), or one
// carrying the bypass value dxc writes when it does not sign.
bool signed_or_bypassed(const uint8_t* data, size_t size) {
  dxbc_spv::dxbc::Container container(data, size);
  if (!container)
    return false;
  if (container.validateHash())
    return true;
  auto hash = container.getHash();
  for (auto byte : hash.data)
    if (byte != 1)
      return false;
  return true;
}

struct Blob {
  dxil_spv_parsed_blob blob = nullptr;
  dxil_spv_converter converter = nullptr;
  ~Blob() {
    if (converter)
      dxil_spv_converter_free(converter);
    if (blob)
      dxil_spv_parsed_blob_free(blob);
  }
};

struct LogScope {
  explicit LogScope(std::string* first) { dxil_spv_set_thread_log_callback(log_error, first); }
  ~LogScope() { dxil_spv_set_thread_log_callback(nullptr, nullptr); }
};

int convert(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  if (!signed_or_bypassed(data, size)) {
    *error = wcs::copy("the container's checksum does not match its bytes");
    return 1;
  }
  std::string first;
  LogScope scope(&first);
  Blob b;
  if (dxil_spv_parse_dxil_blob(data, size, &b.blob) != DXIL_SPV_SUCCESS) {
    *error = wcs::copy(first.empty() ? "not a DXIL container" : first);
    return 1;
  }
  if (dxil_spv_create_converter(b.blob, &b.converter) != DXIL_SPV_SUCCESS ||
      dxil_spv_converter_run(b.converter) != DXIL_SPV_SUCCESS) {
    *error = wcs::copy(first.empty() ? "dxil-spirv could not convert the shader" : first);
    return 2;
  }
  dxil_spv_compiled_spirv spirv = { };
  if (dxil_spv_converter_get_compiled_spirv(b.converter, &spirv) != DXIL_SPV_SUCCESS ||
      !(*out = static_cast<uint32_t*>(std::malloc(spirv.size)))) {
    *error = wcs::copy(first.empty() ? "dxil-spirv gave no SPIR-V" : first);
    return 3;
  }
  std::memcpy(*out, spirv.data, spirv.size);
  *out_words = spirv.size / 4;
  return 0;
}

}

// Returns 0 and the SPIR-V in `out` (free with wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_to_spirv(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error, [&] { return convert(data, size, out, out_words, error); });
}
