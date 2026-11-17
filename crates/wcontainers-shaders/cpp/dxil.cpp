// The one boundary to dxil-spirv: DXIL bytes in, SPIR-V words out, and the constant buffers' sizes.

#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "boundary.h"
#include "cast.hpp"
#include "dxbc/dxbc_container.h"
#include "dxil_parser.hpp"
#include "dxil_spirv_c.h"
#include "llvm_bitcode_parser.hpp"
#include "metadata.hpp"
#include "module.hpp"
#include "value.hpp"

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

int convert(const uint8_t* data, size_t size, uint32_t samples, uint32_t** out, size_t* out_words,
            char** error) {
  // dxbc-spirv's container reader reports into this, not to stderr.
  wcs::ErrorLog container;
  if (!signed_or_bypassed(data, size)) {
    *error = wcs::copy(container.first.empty() ? "the container's checksum does not match its bytes"
                                               : container.first);
    return 1;
  }
  std::string first;
  LogScope scope(&first);
  Blob b;
  if (dxil_spv_parse_dxil_blob(data, size, &b.blob) != DXIL_SPV_SUCCESS) {
    *error = wcs::copy(first.empty() ? "not a DXIL container" : first);
    return 1;
  }
  if (dxil_spv_create_converter(b.blob, &b.converter) != DXIL_SPV_SUCCESS) {
    *error = wcs::copy(first.empty() ? "dxil-spirv could not convert the shader" : first);
    return 2;
  }
  // The targets' sample count as a constant where the variant fixes it; otherwise dxil-spirv leaves a
  // specialisation constant, which the caller refuses.
  dxil_spv_option_rasterizer_sample_count count = { { DXIL_SPV_OPTION_RASTERIZER_SAMPLE_COUNT }, samples,
                                                    DXIL_SPV_FALSE };
  if ((samples != 0 && dxil_spv_converter_add_option(b.converter, &count.base) != DXIL_SPV_SUCCESS) ||
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

// An integer operand of a metadata node, if it is one.
bool integer(const llvm::MDNode* node, unsigned index, uint32_t* value) {
  if (index >= node->getNumOperands())
    return false;
  auto* c = llvm::dyn_cast<llvm::ConstantAsMetadata>(node->getOperand(index));
  auto* v = c ? llvm::dyn_cast<llvm::ConstantInt>(c->getValue()) : nullptr;
  if (!v)
    return false;
  *value = uint32_t(v->getUniqueInteger().getSExtValue());
  return true;
}

// (space, register, size in bytes) of every constant buffer the module's resource list declares.
int constant_sizes(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  dxil_spv::DXILContainerParser container;
  dxil_spv::LLVMBCParser bitcode;
  if (!container.parse_container(data, size, false) ||
      !(container.is_dxbc_binary() ? bitcode.parseDXBCBinary(data, size)
                                   : bitcode.parse(container.get_blob().data(), container.get_blob().size()))) {
    *error = wcs::copy("the DXIL did not parse");
    return 1;
  }
  std::vector<uint32_t> sizes;
  auto* resources = bitcode.get_module().getNamedMetadata("dx.resources");
  auto* lists = resources && resources->getNumOperands() ? resources->getOperand(0) : nullptr;
  auto* cbvs = lists && lists->getNumOperands() > 2 ? llvm::dyn_cast<llvm::MDNode>(lists->getOperand(2)) : nullptr;
  for (unsigned i = 0; cbvs && i < cbvs->getNumOperands(); i++) {
    auto* cbv = llvm::dyn_cast<llvm::MDNode>(cbvs->getOperand(i));
    uint32_t space, reg, bytes;
    if (cbv && integer(cbv, 3, &space) && integer(cbv, 4, &reg) && integer(cbv, 6, &bytes))
      sizes.insert(sizes.end(), { space, reg, bytes });
  }
  if (!(*out = static_cast<uint32_t*>(std::malloc(sizes.size() * 4 + 4)))) {
    *error = wcs::copy("out of memory");
    return 3;
  }
  std::memcpy(*out, sizes.data(), sizes.size() * 4);
  *out_words = sizes.size();
  return 0;
}

}

// Returns 0 and the constant buffers' (space, register, size in bytes) triples in `out` (free with
// wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_constant_sizes(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words,
                                       char** error) {
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error, [&] { return constant_sizes(data, size, out, out_words, error); });
}

// Returns 0 and the SPIR-V in `out` (free with wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_to_spirv(const uint8_t* data, size_t size, uint32_t samples, uint32_t** out,
                                 size_t* out_words, char** error) {
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error, [&] { return convert(data, size, samples, out, out_words, error); });
}
