// The one boundary to dxbc-spirv: DXBC bytes in, its legalised IR out in its serialised token form.

#include <cstdlib>
#include <cstring>
#include <string>

#include "dxbc/dxbc_api.h"
#include "ir/ir_serialize.h"

using namespace dxbc_spv;

namespace {

// Keeps the first error dxbc-spirv reports, as the reason a conversion failed.
class ErrorLog : public util::Logger {
public:
  void message(util::LogLevel severity, const char* text) override {
    if (severity == util::LogLevel::eError && first.empty())
      first = text;
  }
  util::LogLevel getMinimumSeverity() override { return util::LogLevel::eError; }
  std::string first;
};

char* copy(const std::string& s) {
  char* out = static_cast<char*>(std::malloc(s.size() + 1));
  if (out)
    std::memcpy(out, s.c_str(), s.size() + 1);
  return out;
}

}

// Returns 0 and the serialised IR in `out` (free with wcs_free), or nonzero with the reason in `error`
// (free with wcs_free) when one was reported.
extern "C" int wcs_dxbc_to_ir(const uint8_t* data, size_t size, uint8_t** out, size_t* out_size, char** error) {
  ErrorLog log;
  *out = nullptr;
  *out_size = 0;
  *error = nullptr;

  dxbc::Converter::Options convert = { };

  ir::CompileOptions compile = { };
  compile.resourceOptions.allowSubDwordScratchAndLds = false;
  compile.resourceOptions.structuredCbv = false;
  compile.resourceOptions.structuredSrvUav = false;
  compile.min16Options.enableFloat16 = false;
  compile.min16Options.enableInt16 = false;
  compile.scalarizeOptions.subDwordVectors = false;
  compile.arithmeticOptions.lowerDot = false;
  compile.arithmeticOptions.lowerUdiv = true;
  compile.bufferOptions.useTypedForSparseFeedback = true;
  compile.bufferIndexing.allowDynamicIndexing = true;
  compile.cseOptions.relocateDescriptorLoad = true;
  compile.cseOptions.resolveOverlappingLoads = true;

  auto builder = dxbc::compileShaderToLegalizedIr(data, size, convert, compile);
  if (!builder) {
    *error = copy(log.first);
    return 1;
  }

  ir::Serializer serializer(*builder);
  size_t n = serializer.computeSerializedSize();
  auto* buffer = static_cast<uint8_t*>(std::malloc(n));
  if (!buffer || !serializer.serialize(buffer, n)) {
    std::free(buffer);
    *error = copy("the IR did not serialise");
    return 2;
  }
  *out = buffer;
  *out_size = n;
  return 0;
}

extern "C" void wcs_free(void* p) {
  std::free(p);
}
