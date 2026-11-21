// The one boundary to dxbc-spirv: DXBC bytes in, its legalised IR out in its serialised token form.

#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "boundary.h"
#include "dxbc/dxbc_api.h"
#include "dxbc/dxbc_container.h"
#include "dxbc/dxbc_parser.h"
#include "ir/ir_serialize.h"

using namespace dxbc_spv;

namespace {

// (space, first register, size in vectors) of every dynamically indexed constant buffer, which
// dxbc-spirv declares with its largest size.
std::vector<uint32_t> dynamic_constant_buffers(const dxbc::Container& container) {
  std::vector<uint32_t> out;
  dxbc::Parser parser(container.getCodeChunk());
  bool sm51 = parser.getShaderInfo().getVersion() >= std::make_pair(5u, 1u);
  while (parser) {
    auto op = parser.parseInstruction();
    if (!op)
      break;
    if (op.getOpToken().getOpCode() != dxbc::OpCode::eDclConstantBuffer ||
        !op.getOpToken().getCbvDynamicIndexingFlag())
      continue;
    const auto& operand = op.getDst(0u);
    if (sm51)
      out.insert(out.end(), { op.getImm(1u).getImmediate<uint32_t>(0u), operand.getIndex(1u),
                              op.getImm(0u).getImmediate<uint32_t>(0u) });
    else
      out.insert(out.end(), { 0u, operand.getIndex(0u), operand.getIndex(1u) });
  }
  return out;
}

int convert(const uint8_t* data, size_t size, uint8_t** out, size_t* out_size, uint32_t** sizes,
            size_t* sizes_words, char** error, wcs::ErrorLog& log) {
  // Direct3D refuses a container whose checksum does not match its bytes.
  dxbc::Container container(data, size);
  if (!container || !container.validateHash()) {
    *error = wcs::copy("the container's checksum does not match its bytes");
    return 1;
  }
  auto dynamic = dynamic_constant_buffers(container);

  dxbc::Converter::Options convert = { };
  // Its check bounds every vertex-indexed load by the input vertex count, a hull shader's reads of
  // its output control points too; a stage's inputs are arrays of the declared count here, and an
  // index past them is undefined in Direct3D.
  convert.boundCheckShaderIo = false;

  ir::CompileOptions compile = { };
  compile.resourceOptions.allowSubDwordScratchAndLds = false;
  compile.resourceOptions.structuredCbv = false;
  compile.resourceOptions.structuredSrvUav = false;
  compile.min16Options.enableFloat16 = false;
  compile.min16Options.enableInt16 = false;
  compile.scalarizeOptions.subDwordVectors = false;
  compile.arithmeticOptions.lowerDot = false;
  compile.arithmeticOptions.lowerUdiv = true;
  // f32tof16's clamp is made in the lowering (src/lower.rs).
  compile.arithmeticOptions.lowerF32toF16 = false;
  compile.bufferOptions.useTypedForSparseFeedback = true;
  compile.bufferIndexing.allowDynamicIndexing = true;
  compile.cseOptions.relocateDescriptorLoad = true;
  compile.cseOptions.resolveOverlappingLoads = true;

  auto builder = dxbc::compileShaderToLegalizedIr(data, size, convert, compile);
  if (!builder) {
    *error = wcs::copy(log.first.empty() ? "dxbc-spirv gave no reason" : log.first);
    return 1;
  }

  ir::Serializer serializer(*builder);
  size_t n = serializer.computeSerializedSize();
  auto* buffer = static_cast<uint8_t*>(std::malloc(n));
  if (!buffer || !serializer.serialize(buffer, n)) {
    std::free(buffer);
    *error = wcs::copy("the IR did not serialise");
    return 2;
  }
  *sizes = static_cast<uint32_t*>(std::malloc(dynamic.size() * 4 + 4));
  if (!*sizes) {
    std::free(buffer);
    *error = wcs::copy("out of memory");
    return 2;
  }
  std::memcpy(*sizes, dynamic.data(), dynamic.size() * 4);
  *sizes_words = dynamic.size();
  *out = buffer;
  *out_size = n;
  return 0;
}

}

// Returns 0 with the serialised IR in `out` and the dynamically indexed constant buffers' (space,
// register, size) triples in `sizes` (free both with wcs_free), or nonzero with the reason in `error`
// (free with wcs_free).
extern "C" int wcs_dxbc_to_ir(const uint8_t* data, size_t size, uint8_t** out, size_t* out_size,
                              uint32_t** sizes, size_t* sizes_words, char** error) {
  *out = nullptr;
  *out_size = 0;
  *sizes = nullptr;
  *sizes_words = 0;
  *error = nullptr;
  wcs::ErrorLog log;
  return wcs::contained("dxbc-spirv", error, [&] {
    return convert(data, size, out, out_size, sizes, sizes_words, error, log);
  });
}

extern "C" void wcs_free(void* p) {
  std::free(p);
}
