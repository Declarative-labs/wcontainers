// The one boundary to dxil-spirv: DXIL bytes in, SPIR-V words out, the constant buffers' sizes and
// the registers and views the resource list declares.

#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <iterator>
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

// Every resource the shader declares, as (class, space, register) triples: dxil-spirv is told to bind
// each at descriptor set = its class (`wcs::ResourceClass`) and binding = its index here, so resources
// of different classes at one register stay apart; a counter takes its view's index.
struct Resources {
  std::vector<uint32_t> table;

  // The words of one triple.
  static constexpr size_t TRIPLE = size_t(wcs::TableWord::eCount);

  uint32_t index(wcs::ResourceClass cls, const dxil_spv_d3d_binding& d) {
    for (size_t i = 0; i + TRIPLE <= table.size(); i += TRIPLE)
      if (table[i] == uint32_t(cls) && table[i + 1] == d.register_space && table[i + 2] == d.register_index)
        return uint32_t(i / TRIPLE);
    table.insert(table.end(), { uint32_t(cls), d.register_space, d.register_index });
    return uint32_t(table.size() / TRIPLE - 1);
  }

  static void bind(dxil_spv_vulkan_binding* vk, wcs::ResourceClass set, uint32_t binding) {
    *vk = { };
    vk->set = uint32_t(set);
    vk->binding = binding;
    vk->descriptor_type = DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_IDENTITY;
  }

  // Raw and structured buffers as storage buffers (dxil-spirv may otherwise pick a texel buffer),
  // read in words: with the conservative vectorization option this keeps dxil-spirv from adding
  // vector views of the buffer at the same binding, which WGSL cannot declare.
  static void untyped(const dxil_spv_d3d_binding& d, dxil_spv_vulkan_binding* vk) {
    if (d.kind == DXIL_SPV_RESOURCE_KIND_RAW_BUFFER || d.kind == DXIL_SPV_RESOURCE_KIND_STRUCTURED_BUFFER)
      vk->descriptor_type = DXIL_SPV_VULKAN_DESCRIPTOR_TYPE_SSBO_SCALAR_ALIGNED;
  }

  static dxil_spv_bool srv(void* self, const dxil_spv_d3d_binding* d, dxil_spv_srv_vulkan_binding* vk) {
    *vk = { };
    bind(&vk->buffer_binding, wcs::ResourceClass::eView,
         static_cast<Resources*>(self)->index(wcs::ResourceClass::eView, *d));
    untyped(*d, &vk->buffer_binding);
    return DXIL_SPV_TRUE;
  }

  static dxil_spv_bool sampler(void* self, const dxil_spv_d3d_binding* d, dxil_spv_vulkan_binding* vk) {
    bind(vk, wcs::ResourceClass::eSampler,
         static_cast<Resources*>(self)->index(wcs::ResourceClass::eSampler, *d));
    return DXIL_SPV_TRUE;
  }

  static dxil_spv_bool uav(void* self, const dxil_spv_uav_d3d_binding* d, dxil_spv_uav_vulkan_binding* vk) {
    *vk = { };
    uint32_t i = static_cast<Resources*>(self)->index(wcs::ResourceClass::eStorage, d->d3d_binding);
    bind(&vk->buffer_binding, wcs::ResourceClass::eStorage, i);
    untyped(d->d3d_binding, &vk->buffer_binding);
    bind(&vk->counter_binding, wcs::ResourceClass::eCounter, i);
    return DXIL_SPV_TRUE;
  }

  static dxil_spv_bool cbv(void* self, const dxil_spv_d3d_binding* d, dxil_spv_cbv_vulkan_binding* vk) {
    *vk = { };
    bind(&vk->vulkan.uniform_binding, wcs::ResourceClass::eConstants,
         static_cast<Resources*>(self)->index(wcs::ResourceClass::eConstants, *d));
    return DXIL_SPV_TRUE;
  }
};

struct LogScope {
  explicit LogScope(std::string* first) { dxil_spv_set_thread_log_callback(log_error, first); }
  ~LogScope() { dxil_spv_set_thread_log_callback(nullptr, nullptr); }
};

// The first location of the patch constants.
constexpr unsigned PATCH_LOCATION_OFFSET = 0;

int convert(const uint8_t* data, size_t size, uint32_t samples, uint32_t** out, size_t* out_words,
            uint32_t** table, size_t* table_words, char** error) {
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
  Resources resources;
  // Patch constants keep their own registers as locations, apart from the control points' by their
  // `Patch` decoration, as Direct3D numbers them.
  dxil_spv_converter_set_patch_location_offset(b.converter, PATCH_LOCATION_OFFSET);
  dxil_spv_converter_set_srv_remapper(b.converter, Resources::srv, &resources);
  dxil_spv_converter_set_sampler_remapper(b.converter, Resources::sampler, &resources);
  dxil_spv_converter_set_uav_remapper(b.converter, Resources::uav, &resources);
  dxil_spv_converter_set_cbv_remapper(b.converter, Resources::cbv, &resources);
  // The targets' sample count as a constant where the variant fixes it; otherwise dxil-spirv leaves a
  // specialisation constant, which the caller refuses.
  dxil_spv_option_rasterizer_sample_count count = { { DXIL_SPV_OPTION_RASTERIZER_SAMPLE_COUNT }, samples,
                                                    DXIL_SPV_FALSE };
  // A storage buffer is bound at its view's first byte (the fronts place it), so a view's elements
  // need no offset into it whatever their stride.
  dxil_spv_option_ssbo_alignment alignment = { { DXIL_SPV_OPTION_SSBO_ALIGNMENT }, 4 };
  dxil_spv_option_conservative_ssbo_vectorization words = {
    { DXIL_SPV_OPTION_CONSERVATIVE_SSBO_VECTORIZATION }, DXIL_SPV_TRUE };
  if (dxil_spv_converter_add_option(b.converter, &alignment.base) != DXIL_SPV_SUCCESS ||
      dxil_spv_converter_add_option(b.converter, &words.base) != DXIL_SPV_SUCCESS ||
      (samples != 0 && dxil_spv_converter_add_option(b.converter, &count.base) != DXIL_SPV_SUCCESS) ||
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
  if (!(*table = static_cast<uint32_t*>(std::malloc(resources.table.size() * 4 + 4)))) {
    *error = wcs::copy("out of memory");
    return 3;
  }
  std::memcpy(*table, resources.table.data(), resources.table.size() * 4);
  *table_words = resources.table.size();
  return 0;
}

// An integer operand of a metadata node, sign extended, if it is one.
bool integer64(const llvm::MDNode* node, unsigned index, int64_t* value) {
  if (index >= node->getNumOperands())
    return false;
  auto* c = llvm::dyn_cast<llvm::ConstantAsMetadata>(node->getOperand(index));
  auto* v = c ? llvm::dyn_cast<llvm::ConstantInt>(c->getValue()) : nullptr;
  if (!v)
    return false;
  *value = v->getUniqueInteger().getSExtValue();
  return true;
}

// An integer operand of a metadata node, if it is one, in its low 32 bits.
bool integer(const llvm::MDNode* node, unsigned index, uint32_t* value) {
  int64_t v;
  if (!integer64(node, index, &v))
    return false;
  *value = uint32_t(v);
  return true;
}

// An integer operand of a metadata node, if it is one and a 32-bit unsigned value.
bool word(const llvm::MDNode* node, unsigned index, uint32_t* value) {
  int64_t v;
  if (!integer64(node, index, &v) || v < 0 || v > int64_t(UINT32_MAX))
    return false;
  *value = uint32_t(v);
  return true;
}

// The lists of DXIL's resource list (`dx.resources`), in its order, with the class each list's
// resources take.
constexpr wcs::ResourceClass LIST_CLASSES[] = { wcs::ResourceClass::eView, wcs::ResourceClass::eStorage,
                                                wcs::ResourceClass::eConstants, wcs::ResourceClass::eSampler };
constexpr unsigned CONSTANTS_LIST = 2;

// The operands of a resource's node in DXIL's resource list that are read here.
enum ResourceOperand : unsigned {
  eSpace = 3,
  eLowerBound = 4,
  eRangeSize = 5,
  // A constant buffer's size in bytes.
  eConstantBytes = 6,
  // A view's resource kind (`DXIL::ResourceKind`).
  eKind = 6,
};

// The range size DXIL gives an unbounded range.
constexpr uint32_t UNBOUNDED = uint32_t(wcs::ViewRange::eUnbounded);

// A module the resource list readers parsed, with the container it came from.
struct Parsed {
  std::vector<uint8_t> container;
  bool dxbc_binary = false;
  std::unique_ptr<dxil_spv::LLVMBCParser> bitcode;
};
// The modules parsed last on this thread, the latest first, one for each stage a graphics pipeline
// has (vertex, hull, domain, geometry, pixel): each stage's views, declared registers and constant
// buffer sizes are read from one parse of its module, in whatever order the stages are read.
constexpr size_t REMEMBERED = 5;
#if defined(__wasm__) && !defined(__wasm_atomics__)
// WebAssembly without threads has one thread, and its C++ runtime has no thread exit hook
// (`__cxa_thread_atexit`) to destroy a `thread_local`.
std::vector<Parsed> remembered;
#else
thread_local std::vector<Parsed> remembered;
#endif

// The module of a container's `DXIL` chunk, or of its DXBC program where `dxbc` allows one; null with
// the reason in `error` where there is none or it does not parse. Valid until the next call.
dxil_spv::LLVMBCParser* parse(const uint8_t* data, size_t size, bool dxbc, char** error) {
  auto found = std::find_if(remembered.begin(), remembered.end(), [&](const Parsed& p) {
    return p.container.size() == size && std::memcmp(p.container.data(), data, size) == 0;
  });
  if (found == remembered.end()) {
    // dxil-spirv's and dxbc-spirv's reasons go into `error`, not to stderr.
    std::string first;
    LogScope scope(&first);
    wcs::ErrorLog dxbc_log;
    auto refused = [&](const char* reason) -> dxil_spv::LLVMBCParser* {
      *error = wcs::copy(!first.empty() ? first : !dxbc_log.first.empty() ? dxbc_log.first : reason);
      return nullptr;
    };
    dxil_spv::DXILContainerParser container;
    if (!container.parse_container(data, size, false))
      return refused("the DXIL did not parse");
    Parsed parsed;
    parsed.dxbc_binary = container.is_dxbc_binary();
    if (parsed.dxbc_binary && !dxbc)
      return refused("not a DXIL container");
    parsed.bitcode = std::make_unique<dxil_spv::LLVMBCParser>();
    if (!(parsed.dxbc_binary ? parsed.bitcode->parseDXBCBinary(data, size)
                             : parsed.bitcode->parse(container.get_blob().data(), container.get_blob().size())))
      return refused("the DXIL did not parse");
    parsed.container.assign(data, data + size);
    if (remembered.size() == REMEMBERED)
      remembered.pop_back();
    found = remembered.insert(remembered.begin(), std::move(parsed));
  } else {
    std::rotate(remembered.begin(), found, found + 1);
    found = remembered.begin();
  }
  if (found->dxbc_binary && !dxbc) {
    *error = wcs::copy("not a DXIL container");
    return nullptr;
  }
  return found->bitcode.get();
}

// The node of the module's resource list (`dx.resources`) holding its lists, or null where it has none.
const llvm::MDNode* resource_lists(dxil_spv::LLVMBCParser& bitcode) {
  auto* resources = bitcode.get_module().getNamedMetadata("dx.resources");
  return resources && resources->getNumOperands() ? resources->getOperand(0) : nullptr;
}

// Calls `f(list, node)` for every resource node of the module's resource list.
template <typename F>
bool each_resource(const uint8_t* data, size_t size, char** error, F&& f) {
  auto* bitcode = parse(data, size, true, error);
  if (!bitcode)
    return false;
  auto* lists = resource_lists(*bitcode);
  for (unsigned list = 0; lists && list < std::size(LIST_CLASSES) && list < lists->getNumOperands(); list++) {
    auto* nodes = llvm::dyn_cast<llvm::MDNode>(lists->getOperand(list));
    for (unsigned i = 0; nodes && i < nodes->getNumOperands(); i++)
      if (auto* node = llvm::dyn_cast<llvm::MDNode>(nodes->getOperand(i)))
        f(list, node);
  }
  return true;
}

// `words` copied into a malloc'd `out`.
int give(const std::vector<uint32_t>& words, uint32_t** out, size_t* out_words, char** error) {
  if (!(*out = static_cast<uint32_t*>(std::malloc(words.size() * sizeof(uint32_t) + sizeof(uint32_t))))) {
    *error = wcs::copy("out of memory");
    return 3;
  }
  std::memcpy(*out, words.data(), words.size() * sizeof(uint32_t));
  *out_words = words.size();
  return 0;
}

// (space, register, size in bytes) of every constant buffer the module's resource list declares.
int constant_sizes(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  std::vector<uint32_t> sizes;
  if (!each_resource(data, size, error, [&](unsigned list, const llvm::MDNode* node) {
        uint32_t space, reg, bytes;
        if (list == CONSTANTS_LIST && integer(node, eSpace, &space) && integer(node, eLowerBound, &reg) &&
            integer(node, eConstantBytes, &bytes))
          sizes.insert(sizes.end(), { space, reg, bytes });
      }))
    return 1;
  return give(sizes, out, out_words, error);
}

// (class, space, register) of every register a range of the module's resource list covers, used by
// the program or not; an unbounded range gives its first register.
int declared_registers(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  std::vector<uint32_t> registers;
  if (!each_resource(data, size, error, [&](unsigned list, const llvm::MDNode* node) {
        uint32_t space, first, count;
        if (!integer(node, eSpace, &space) || !integer(node, eLowerBound, &first) ||
            !integer(node, eRangeSize, &count))
          return;
        uint32_t covered = count == UNBOUNDED || count == 0 ? 1 : count;
        for (uint32_t r = 0; r < covered; r++)
          registers.insert(registers.end(), { uint32_t(LIST_CLASSES[list]), space, first + r });
      }))
    return 1;
  return give(registers, out, out_words, error);
}

// The lists of the resource list `views` reads (the shader resource views', the unordered access
// views'), and where each list's views hold their extended properties (`DXIL.rst`, "Resources").
constexpr unsigned VIEW_LISTS = 2;
constexpr unsigned EXTENDED_PROPERTIES[VIEW_LISTS] = { 8, 10 };
// The tag of the extended property giving the element type (DirectXShaderCompiler's
// `kDxilTypedBufferElementTypeTag`).
constexpr int64_t ELEMENT_TYPE_TAG = 0;

// The shape of a view of resource kind `kind`.
wcs::ViewShape shape(int64_t kind) {
  using K = DXIL::ResourceKind;
  if (kind == int64_t(K::Texture2DMSArray))
    return wcs::ViewShape::eMultisampledArray;
  if (kind == int64_t(K::TypedBuffer))
    return wcs::ViewShape::eTypedBuffer;
  if (kind >= int64_t(K::Texture1D) && kind <= int64_t(K::TextureCubeArray))
    return wcs::ViewShape::eTexture;
  return wcs::ViewShape::eOther;
}

// The kind of value of element type `type`.
wcs::ViewValue value(int64_t type) {
  using C = DXIL::ComponentType;
  for (C sint : { C::I1, C::I16, C::I32, C::I64 })
    if (type == int64_t(sint))
      return wcs::ViewValue::eSint;
  for (C uint : { C::U16, C::U32, C::U64 })
    if (type == int64_t(uint))
      return wcs::ViewValue::eUint;
  if (type >= int64_t(C::F16) && type <= int64_t(C::UNormF64))
    return wcs::ViewValue::eFloat;
  return wcs::ViewValue::eNone;
}

// The number of the two view lists the module's resource list holds (both where it has none), then for
// every view of them its `ViewWord`s. A container whose checksum does not match is not read.
int views(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words, char** error) {
  {
    // dxbc-spirv's container reader reports into this, not to stderr.
    wcs::ErrorLog container;
    if (!signed_or_bypassed(data, size)) {
      *error = wcs::copy(container.first.empty() ? "the container's checksum does not match its bytes"
                                                 : container.first);
      return 1;
    }
  }
  auto* bitcode = parse(data, size, false, error);
  if (!bitcode)
    return 1;
  auto* lists = resource_lists(*bitcode);
  unsigned held = lists ? std::min(lists->getNumOperands(), VIEW_LISTS) : VIEW_LISTS;
  std::vector<uint32_t> words = { held };
  for (unsigned list = 0; lists && list < held; list++) {
    auto* nodes = llvm::dyn_cast<llvm::MDNode>(lists->getOperand(list));
    for (unsigned i = 0; nodes && i < nodes->getNumOperands(); i++) {
      using W = wcs::ViewWord;
      uint32_t view[size_t(W::eCount)] = { };
      auto at = [&](W w) -> uint32_t& { return view[size_t(w)]; };
      auto* node = llvm::dyn_cast<llvm::MDNode>(nodes->getOperand(i));
      int64_t kind = 0, count = 0;
      at(W::eList) = list;
      at(W::eReadable) = node && integer64(node, eKind, &kind) && word(node, eSpace, &at(W::eSpace)) &&
                         word(node, eLowerBound, &at(W::eLowerBound)) && integer64(node, eRangeSize, &count);
      // DXIL writes an unbounded range's size as -1; a size of no 32-bit unsigned value is one too.
      at(W::eRangeSize) = count >= 0 && count <= int64_t(UNBOUNDED) ? uint32_t(count) : UNBOUNDED;
      at(W::eShape) = uint32_t(shape(kind));
      unsigned extended = EXTENDED_PROPERTIES[list];
      auto* tags = at(W::eReadable) && extended < node->getNumOperands()
                       ? llvm::dyn_cast<llvm::MDNode>(node->getOperand(extended))
                       : nullptr;
      for (unsigned t = 0; tags && t < tags->getNumOperands(); t += 2) {
        int64_t tag, type;
        if (integer64(tags, t, &tag) && tag == ELEMENT_TYPE_TAG) {
          if (integer64(tags, t + 1, &type))
            at(W::eValue) = uint32_t(value(type));
          break;
        }
      }
      words.insert(words.end(), std::begin(view), std::end(view));
    }
  }
  return give(words, out, out_words, error);
}

}

// Returns 0 and the views of the resource list (`views`) in `out` (free with wcs_free), or nonzero with
// the reason in `error`.
extern "C" int wcs_dxil_views(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words,
                              char** error) {
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error, [&] { return views(data, size, out, out_words, error); });
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

// Returns 0 and the (class, space, register) triples of every register the resource list declares in
// `out` (free with wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_declared_registers(const uint8_t* data, size_t size, uint32_t** out, size_t* out_words,
                                           char** error) {
  *out = nullptr;
  *out_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error,
                        [&] { return declared_registers(data, size, out, out_words, error); });
}

// Returns 0, the SPIR-V in `out` and the resources' (class, space, register) triples in `table` (both
// freed with wcs_free), or nonzero with the reason in `error`.
extern "C" int wcs_dxil_to_spirv(const uint8_t* data, size_t size, uint32_t samples, uint32_t** out,
                                 size_t* out_words, uint32_t** table, size_t* table_words, char** error) {
  *out = nullptr;
  *out_words = 0;
  *table = nullptr;
  *table_words = 0;
  *error = nullptr;
  return wcs::contained("dxil-spirv", error, [&] {
    int status = convert(data, size, samples, out, out_words, table, table_words, error);
    if (status != 0 && *out) {
      std::free(*out);
      *out = nullptr;
    }
    return status;
  });
}
