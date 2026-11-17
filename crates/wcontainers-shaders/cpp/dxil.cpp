// The one boundary to dxil-spirv: DXIL bytes in, SPIR-V words out.

#include <cstdlib>
#include <cstring>
#include <string>

#include "dxil_spirv_c.h"

namespace {

// Keeps the first error dxil-spirv reports, as the reason a conversion failed.
void log_error(void* userdata, dxil_spv_log_level level, const char* text) {
  auto* first = static_cast<std::string*>(userdata);
  if (level == DXIL_SPV_LOG_LEVEL_ERROR && first->empty())
    *first = text;
}

char* copy(const std::string& s) {
  char* out = static_cast<char*>(std::malloc(s.size() + 1));
  if (out)
    std::memcpy(out, s.c_str(), s.size() + 1);
  return out;
}

}

// Returns 0 and the SPIR-V in `out` (free with wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_to_spirv(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  std::string first;
  dxil_spv_set_thread_log_callback(log_error, &first);
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;

  dxil_spv_parsed_blob blob = nullptr;
  if (dxil_spv_parse_dxil_blob(data, size, &blob) != DXIL_SPV_SUCCESS) {
    dxil_spv_set_thread_log_callback(nullptr, nullptr);
    *error = copy(first.empty() ? "not a DXIL container" : first);
    return 1;
  }
  dxil_spv_converter converter = nullptr;
  int status = 0;
  if (dxil_spv_create_converter(blob, &converter) != DXIL_SPV_SUCCESS ||
      dxil_spv_converter_run(converter) != DXIL_SPV_SUCCESS) {
    status = 2;
  } else {
    dxil_spv_compiled_spirv spirv = { };
    if (dxil_spv_converter_get_compiled_spirv(converter, &spirv) != DXIL_SPV_SUCCESS ||
        !(*out = static_cast<uint32_t*>(std::malloc(spirv.size)))) {
      status = 3;
    } else {
      std::memcpy(*out, spirv.data, spirv.size);
      *out_words = spirv.size / 4;
    }
  }
  if (status)
    *error = copy(first.empty() ? "dxil-spirv could not convert the shader" : first);
  if (converter)
    dxil_spv_converter_free(converter);
  dxil_spv_parsed_blob_free(blob);
  dxil_spv_set_thread_log_callback(nullptr, nullptr);
  return status;
}
