/* Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
   The hostile C caller (tools/api-crate-generator/src/glue_c_caller.rs). */

#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { CALLS, SWEPT, SKIPPED, SUCCEEDED, FAILED, ADOPTED, RELEASED, STALE_THIS, WRONG_THIS, NULL_THIS, GARBAGE_THIS, BIG_COUNTS, PAST_LIMIT, NULL_POINTERS, EXPORTS_CALLED, QUERY_INTERFACES, QUERY_LEFT_POINTER, REFUSED, REFUSED_WRONG, COUNTED };

typedef struct GuidAbi { uint32_t data1; uint16_t data2, data3; uint8_t data4[8]; } GuidAbi;

#define POOL 24
#define STALE 64

typedef struct State State;
static int is_a(int have, int want);

/* The caller's own COM object, foreign to the glue: answers no interface, counts its references. */
typedef struct Foreign { void **table; atomic_long references; } Foreign;
static int32_t foreign_query(void *self, const uint8_t *riid, void **out) { (void)self; (void)riid; if (out) *out = NULL; return (int32_t)0x80004002u; }
static uint32_t foreign_add_ref(void *self) { return (uint32_t)atomic_fetch_add(&((Foreign *)self)->references, 1) + 1; }
static uint32_t foreign_release(void *self) { return (uint32_t)atomic_fetch_sub(&((Foreign *)self)->references, 1) - 1; }
static void *FOREIGN_TABLE[3] = {(void *)foreign_query, (void *)foreign_add_ref, (void *)foreign_release};
static Foreign FOREIGN = {FOREIGN_TABLE, 1};
#define INTERFACES 126
#define MOST 65536u

static const uint8_t IIDS[INTERFACES][16] = {
    {0x08, 0xfb, 0xa5, 0x8b, 0x95, 0x51, 0xe2, 0x40, 0xac, 0x58, 0x0d, 0x98, 0x9c, 0x3a, 0x01, 0x02}, /* ID3DBlob */
    {0x9a, 0xb3, 0x6e, 0xa0, 0xda, 0x50, 0x5b, 0x42, 0x8c, 0x31, 0x4e, 0xec, 0xd6, 0xc2, 0x70, 0xf3}, /* ID3DDestructionNotifier */
    {0xcd, 0xd0, 0x35, 0x4b, 0x15, 0x1e, 0x58, 0x42, 0x9c, 0x98, 0x1b, 0x13, 0x33, 0xf6, 0xdd, 0x3b}, /* ID3D11Asynchronous */
    {0x08, 0xa3, 0x15, 0x30, 0xbd, 0xdc, 0xaa, 0x47, 0xa7, 0x47, 0x19, 0x24, 0x86, 0xd1, 0x4d, 0x4a}, /* ID3D11AuthenticatedChannel */
    {0xaa, 0x8f, 0xb6, 0x75, 0x7d, 0x34, 0x59, 0x41, 0x8f, 0x45, 0xa0, 0x64, 0x0f, 0x01, 0xcd, 0x9a}, /* ID3D11BlendState */
    {0xbe, 0xfa, 0x86, 0xcc, 0x55, 0xda, 0x1d, 0x40, 0x85, 0xe7, 0xe3, 0xc9, 0xde, 0x28, 0x77, 0xe9}, /* ID3D11BlendState1 */
    {0x85, 0x0b, 0x57, 0x48, 0xee, 0xd1, 0xcd, 0x4f, 0xa2, 0x50, 0xeb, 0x35, 0x07, 0x22, 0xb0, 0x37}, /* ID3D11Buffer */
    {0xaa, 0x7f, 0xcd, 0xa6, 0xb7, 0xb0, 0x2f, 0x4a, 0x94, 0x36, 0x86, 0x62, 0xa6, 0x57, 0x97, 0xcb}, /* ID3D11ClassInstance */
    {0xba, 0x7c, 0xf5, 0xdd, 0x43, 0x95, 0xe4, 0x46, 0xa1, 0x2b, 0xf2, 0x07, 0xa0, 0xfe, 0x7f, 0xed}, /* ID3D11ClassLinkage */
    {0xd1, 0xc4, 0x4b, 0xa2, 0x9e, 0x76, 0xf7, 0x43, 0x80, 0x13, 0x98, 0xff, 0x56, 0x6c, 0x18, 0xe2}, /* ID3D11CommandList */
    {0x6e, 0x19, 0x5b, 0x4f, 0xbd, 0xc2, 0x5e, 0x49, 0xbd, 0x01, 0x1f, 0xde, 0xd3, 0x8e, 0x49, 0x69}, /* ID3D11ComputeShader */
    {0xfb, 0x49, 0x8c, 0x6e, 0x71, 0xa3, 0x70, 0x47, 0xb4, 0x40, 0x29, 0x08, 0x60, 0x22, 0xb7, 0x41}, /* ID3D11Counter */
    {0xad, 0xf9, 0x32, 0x9b, 0xcc, 0xbd, 0xa6, 0x40, 0xa3, 0x9d, 0xd5, 0xc8, 0x65, 0x84, 0x57, 0x20}, /* ID3D11CryptoSession */
    {0x33, 0x22, 0xcf, 0x79, 0x36, 0x75, 0x48, 0x49, 0x9d, 0x36, 0x1e, 0x46, 0x92, 0xdc, 0x57, 0x60}, /* ID3D11Debug */
    {0xfb, 0x3e, 0x82, 0x03, 0x8f, 0x8d, 0x1c, 0x4e, 0x9a, 0xa2, 0xf6, 0x4b, 0xb2, 0xcb, 0xfd, 0xf1}, /* ID3D11DepthStencilState */
    {0x2a, 0xc9, 0xda, 0x9f, 0x76, 0x18, 0xc3, 0x48, 0xaf, 0xad, 0x25, 0xb9, 0x4f, 0x84, 0xa9, 0xb6}, /* ID3D11DepthStencilView */
    {0xdb, 0x6d, 0x6f, 0xdb, 0x77, 0xac, 0x88, 0x4e, 0x82, 0x53, 0x81, 0x9d, 0xf9, 0xbb, 0xf1, 0x40}, /* ID3D11Device */
    {0x29, 0xfb, 0x4b, 0xa0, 0xef, 0x08, 0xd6, 0x43, 0xa4, 0x9c, 0xa9, 0xbd, 0xbd, 0xcb, 0xe6, 0x86}, /* ID3D11Device1 */
    {0xfa, 0xdf, 0x06, 0x9d, 0xe5, 0xd1, 0x07, 0x4d, 0x83, 0xa8, 0x1b, 0xb1, 0x23, 0xf2, 0xf8, 0x41}, /* ID3D11Device2 */
    {0x37, 0x8c, 0x5c, 0xa0, 0xc6, 0xd2, 0x32, 0x47, 0xb3, 0xa0, 0x9c, 0xe0, 0xb0, 0xdc, 0x9a, 0xe6}, /* ID3D11Device3 */
    {0x71, 0xab, 0x92, 0x89, 0xe6, 0x02, 0x8d, 0x4b, 0xba, 0x48, 0xb0, 0x56, 0xdc, 0xda, 0x42, 0xc4}, /* ID3D11Device4 */
    {0x02, 0xe2, 0xfd, 0x8f, 0xe7, 0xa0, 0xdf, 0x45, 0x9e, 0x01, 0xe8, 0x37, 0x80, 0x1b, 0x5e, 0xa0}, /* ID3D11Device5 */
    {0xc8, 0xe5, 0x41, 0x18, 0xb0, 0x16, 0x9b, 0x48, 0xbc, 0xc8, 0x44, 0xcf, 0xb0, 0xd5, 0xde, 0xae}, /* ID3D11DeviceChild */
    {0x6c, 0xa9, 0xbf, 0xc0, 0x89, 0xe0, 0xfb, 0x44, 0x8e, 0xaf, 0x26, 0xf8, 0x79, 0x61, 0x90, 0xda}, /* ID3D11DeviceContext */
    {0xaa, 0x6f, 0x2c, 0xbb, 0xfb, 0xb5, 0x82, 0x40, 0x8e, 0x6b, 0x38, 0x8b, 0x8c, 0xfa, 0x90, 0xe1}, /* ID3D11DeviceContext1 */
    {0x32, 0x5b, 0x0d, 0x42, 0x0c, 0xb9, 0xa4, 0x4d, 0xbe, 0xf0, 0x35, 0x9f, 0x6a, 0x24, 0xa8, 0x3a}, /* ID3D11DeviceContext2 */
    {0x1d, 0xc0, 0xe3, 0xb4, 0x9e, 0xe7, 0x37, 0x46, 0x91, 0xb2, 0x51, 0x0e, 0x9f, 0x4c, 0x9b, 0x8f}, /* ID3D11DeviceContext3 */
    {0xda, 0x00, 0x76, 0x91, 0x8c, 0xf5, 0x33, 0x4c, 0x98, 0xd8, 0x3e, 0x15, 0xb3, 0x90, 0xfa, 0x24}, /* ID3D11DeviceContext4 */
    {0x08, 0xc5, 0x82, 0xf5, 0x36, 0x0f, 0x0c, 0x49, 0x99, 0x77, 0x31, 0xee, 0xce, 0x26, 0x8c, 0xfa}, /* ID3D11DomainShader */
    {0xd1, 0xe9, 0xfd, 0xaf, 0xf7, 0x1d, 0xb7, 0x4b, 0x8a, 0x34, 0x0f, 0x46, 0x25, 0x1d, 0xab, 0x80}, /* ID3D11Fence */
    {0x96, 0x5b, 0x32, 0x38, 0xfb, 0xef, 0x22, 0x40, 0xba, 0x02, 0x2e, 0x79, 0x5b, 0x70, 0x27, 0x5c}, /* ID3D11GeometryShader */
    {0x61, 0x60, 0x5c, 0x8e, 0x8a, 0x62, 0x8e, 0x4c, 0x82, 0x64, 0xbb, 0xe4, 0x5c, 0xb3, 0xd5, 0xdd}, /* ID3D11HullShader */
    {0xb6, 0xdb, 0x43, 0x65, 0x48, 0x1b, 0xf5, 0x42, 0xab, 0x82, 0xe9, 0x7e, 0xc7, 0x43, 0x26, 0xf6}, /* ID3D11InfoQueue */
    {0xdc, 0x9d, 0x81, 0xe4, 0xf0, 0x4c, 0x25, 0x40, 0xbd, 0x26, 0x5d, 0xe8, 0x2a, 0x3e, 0x07, 0xb7}, /* ID3D11InputLayout */
    {0x00, 0x4e, 0x7e, 0x9b, 0x2c, 0x34, 0x06, 0x41, 0xa1, 0x9f, 0x4f, 0x27, 0x04, 0xf6, 0x89, 0xf0}, /* ID3D11Multithread */
    {0x0d, 0xe4, 0x82, 0xea, 0xdc, 0x51, 0x33, 0x4f, 0x93, 0xd4, 0xdb, 0x7c, 0x91, 0x25, 0xae, 0x8c}, /* ID3D11PixelShader */
    {0xdd, 0x76, 0xb5, 0x9e, 0x77, 0x9f, 0x86, 0x4d, 0x81, 0xaa, 0x8b, 0xab, 0x5f, 0xe4, 0x90, 0xe2}, /* ID3D11Predicate */
    {0x47, 0x07, 0xc0, 0xd6, 0xb7, 0x87, 0x5e, 0x42, 0xb8, 0x4d, 0x44, 0xd1, 0x08, 0x56, 0x0a, 0xfd}, /* ID3D11Query */
    {0x66, 0x47, 0x1b, 0x63, 0xdc, 0x36, 0x1d, 0x46, 0x8d, 0xb6, 0xc4, 0x7e, 0x13, 0xe6, 0x09, 0x16}, /* ID3D11Query1 */
    {0x81, 0xab, 0xb4, 0x9b, 0x1a, 0xab, 0x8f, 0x4d, 0xb5, 0x06, 0xfc, 0x04, 0x20, 0x0b, 0x6e, 0xe7}, /* ID3D11RasterizerState */
    {0xa6, 0xd7, 0x17, 0x12, 0x39, 0x50, 0x8c, 0x41, 0xb0, 0x42, 0x9c, 0xbe, 0x25, 0x6a, 0xfd, 0x6e}, /* ID3D11RasterizerState1 */
    {0xfb, 0x02, 0xbd, 0x6f, 0x9f, 0x20, 0xc4, 0x46, 0xb0, 0x59, 0x2e, 0xd1, 0x55, 0x86, 0xa6, 0xac}, /* ID3D11RasterizerState2 */
    {0x15, 0x66, 0x91, 0x03, 0x44, 0xc6, 0x8c, 0x41, 0x9b, 0xf4, 0x75, 0xdb, 0x5b, 0xe6, 0x3c, 0xa0}, /* ID3D11RefDefaultTrackingOptions */
    {0xdf, 0xac, 0x3d, 0x19, 0xb2, 0x0d, 0x05, 0x4c, 0xa5, 0x5c, 0xef, 0x06, 0xca, 0xc5, 0x6f, 0xd9}, /* ID3D11RefTrackingOptions */
    {0x67, 0xa0, 0xdb, 0xdf, 0x8d, 0x0b, 0x65, 0x48, 0x87, 0x5b, 0xd7, 0xb4, 0x51, 0x6c, 0xc1, 0x64}, /* ID3D11RenderTargetView */
    {0x23, 0x2e, 0xbe, 0xff, 0x11, 0xf0, 0x8a, 0x41, 0xac, 0x56, 0x5c, 0xee, 0xd7, 0xc5, 0xb9, 0x4b}, /* ID3D11RenderTargetView1 */
    {0xf3, 0x63, 0x8e, 0xdc, 0x2b, 0xd1, 0x52, 0x49, 0xb4, 0x7b, 0x5e, 0x45, 0x02, 0x6a, 0x86, 0x2d}, /* ID3D11Resource */
    {0x51, 0xea, 0x6f, 0xda, 0x4c, 0x56, 0x87, 0x44, 0x98, 0x10, 0xf0, 0xd0, 0xf9, 0xb4, 0xe3, 0xa5}, /* ID3D11SamplerState */
    {0xe0, 0x6f, 0xe0, 0xb0, 0x92, 0x81, 0x1a, 0x4e, 0xb1, 0xca, 0x36, 0xd7, 0x41, 0x47, 0x10, 0xb2}, /* ID3D11ShaderResourceView */
    {0x87, 0x8b, 0x30, 0x91, 0x40, 0x90, 0x1d, 0x41, 0x8c, 0x67, 0xc3, 0x92, 0x53, 0xce, 0x38, 0x02}, /* ID3D11ShaderResourceView1 */
    {0xe6, 0x13, 0xb0, 0x36, 0x11, 0x28, 0x45, 0x48, 0xba, 0xa7, 0xd6, 0x23, 0xfe, 0x0d, 0xf1, 0x04}, /* ID3D11ShaderTrace */
    {0x29, 0xd4, 0xba, 0x1f, 0xab, 0x66, 0xcc, 0x41, 0x96, 0x17, 0x66, 0x7a, 0xc1, 0x0e, 0x44, 0x59}, /* ID3D11ShaderTraceFactory */
    {0xe3, 0x37, 0xf3, 0x1e, 0xe7, 0x58, 0x83, 0x4f, 0xa6, 0x92, 0xdb, 0x22, 0x1f, 0x5e, 0xd4, 0x7e}, /* ID3D11SwitchToRef */
    {0x27, 0x5c, 0xfb, 0xf8, 0xb3, 0xc6, 0x75, 0x4f, 0xa4, 0xc8, 0x43, 0x9a, 0xf2, 0xef, 0x56, 0x4c}, /* ID3D11Texture1D */
    {0xf2, 0xaa, 0x15, 0x6f, 0x08, 0xd2, 0x89, 0x4e, 0x9a, 0xb4, 0x48, 0x95, 0x35, 0xd3, 0x4f, 0x9c}, /* ID3D11Texture2D */
    {0x51, 0x82, 0x21, 0x51, 0x33, 0x1e, 0x17, 0x46, 0x9c, 0xcb, 0x4d, 0x3a, 0x43, 0x67, 0xe7, 0xbb}, /* ID3D11Texture2D1 */
    {0x6e, 0x86, 0x7e, 0x03, 0x6d, 0xf5, 0x57, 0x43, 0xa8, 0xaf, 0x9d, 0xab, 0xbe, 0x6e, 0x25, 0x0e}, /* ID3D11Texture3D */
    {0x83, 0x16, 0x71, 0x0c, 0x53, 0x28, 0x46, 0x48, 0x9b, 0xb0, 0xf3, 0xe6, 0x06, 0x39, 0xe4, 0x6a}, /* ID3D11Texture3D1 */
    {0x71, 0xc7, 0x11, 0x19, 0x87, 0x15, 0x3e, 0x41, 0xa7, 0xe0, 0xfb, 0x26, 0xc3, 0xde, 0x02, 0x68}, /* ID3D11TracingDevice */
    {0x09, 0xf5, 0xac, 0x28, 0x5c, 0x7f, 0xf6, 0x48, 0x86, 0x11, 0xf3, 0x16, 0x01, 0x0a, 0x63, 0x80}, /* ID3D11UnorderedAccessView */
    {0x53, 0x61, 0x3b, 0x7b, 0x86, 0xa8, 0x44, 0x45, 0xab, 0x37, 0x65, 0x37, 0xc8, 0x50, 0x04, 0x03}, /* ID3D11UnorderedAccessView1 */
    {0x64, 0x1d, 0x30, 0x3b, 0x78, 0xd6, 0x89, 0x42, 0x88, 0x97, 0x22, 0xf8, 0x92, 0x8b, 0x72, 0xf3}, /* ID3D11VertexShader */
    {0x45, 0x1c, 0xf2, 0x61, 0x0e, 0x3c, 0x74, 0x4a, 0x9c, 0xea, 0x67, 0x10, 0x0d, 0x9a, 0xd5, 0xe4}, /* ID3D11VideoContext */
    {0xda, 0x26, 0xf0, 0xa7, 0xf8, 0xa5, 0x87, 0x44, 0xa5, 0x64, 0x15, 0xe3, 0x43, 0x57, 0x65, 0x1e}, /* ID3D11VideoContext1 */
    {0x4c, 0x37, 0xe7, 0xc4, 0x43, 0x62, 0x1b, 0x4d, 0xae, 0x87, 0x52, 0xb4, 0xf7, 0x40, 0xe2, 0x61}, /* ID3D11VideoContext2 */
    {0xa0, 0xfa, 0xe2, 0xa9, 0x39, 0xcb, 0x8f, 0x41, 0xa0, 0xb7, 0xd8, 0xaa, 0xd4, 0xde, 0x67, 0x2e}, /* ID3D11VideoContext3 */
    {0x51, 0x5b, 0x9c, 0x3c, 0x5d, 0x99, 0xd1, 0x48, 0x9b, 0x8d, 0xfa, 0x5c, 0xae, 0xde, 0xd6, 0x5c}, /* ID3D11VideoDecoder */
    {0xea, 0x1a, 0x93, 0xc2, 0x85, 0x2a, 0x20, 0x4f, 0x86, 0x0f, 0xfb, 0xa1, 0xfd, 0x25, 0x6e, 0x18}, /* ID3D11VideoDecoderOutputView */
    {0x5b, 0x4d, 0xec, 0x10, 0x5a, 0x97, 0x89, 0x46, 0xb9, 0xe4, 0xd0, 0xaa, 0xc3, 0x0f, 0xe3, 0x33}, /* ID3D11VideoDevice */
    {0x51, 0x1d, 0xda, 0x29, 0x21, 0x13, 0x54, 0x44, 0x80, 0x4b, 0xf5, 0xfc, 0x9f, 0x86, 0x1f, 0x0f}, /* ID3D11VideoDevice1 */
    {0x01, 0xcb, 0xc0, 0x59, 0xf0, 0x35, 0x70, 0x4a, 0x8f, 0x67, 0x87, 0x90, 0x5c, 0x90, 0x6a, 0x53}, /* ID3D11VideoDevice2 */
    {0x52, 0x06, 0x7b, 0x1d, 0x5f, 0x18, 0xc6, 0x41, 0x85, 0xce, 0x0c, 0x5b, 0xe3, 0xd4, 0xae, 0x6c}, /* ID3D11VideoProcessor */
    {0x37, 0x70, 0x62, 0x31, 0xab, 0x53, 0x00, 0x42, 0x90, 0x61, 0x05, 0xfa, 0xa9, 0xab, 0x45, 0xf9}, /* ID3D11VideoProcessorEnumerator */
    {0xf2, 0x17, 0x52, 0x46, 0x68, 0x55, 0xcf, 0x43, 0xb5, 0xb9, 0xf6, 0x1d, 0x54, 0x53, 0x1c, 0xa1}, /* ID3D11VideoProcessorEnumerator1 */
    {0x5f, 0x5a, 0xec, 0x11, 0xdc, 0x51, 0x45, 0x49, 0xab, 0x34, 0x6e, 0x8c, 0x21, 0x30, 0x0e, 0xa5}, /* ID3D11VideoProcessorInputView */
    {0x5e, 0x28, 0x48, 0xa0, 0xa9, 0x25, 0x27, 0x45, 0xbd, 0x93, 0xd6, 0x8b, 0x68, 0xc4, 0x42, 0x54}, /* ID3D11VideoProcessorOutputView */
    {0x16, 0x12, 0x9d, 0x83, 0x2e, 0xbb, 0x2b, 0x41, 0xb7, 0xf4, 0xa9, 0xdb, 0xeb, 0xe0, 0x8e, 0xd1}, /* ID3D11View */
    {0x8a, 0x0d, 0x1e, 0x5c, 0x23, 0x7c, 0xf9, 0x48, 0x8c, 0x59, 0xa9, 0x29, 0x58, 0xce, 0xff, 0x11}, /* ID3DDeviceContextState */
    {0x8b, 0xad, 0xda, 0xb2, 0xd4, 0x03, 0xbf, 0x4d, 0x95, 0xeb, 0x32, 0xab, 0x4b, 0x63, 0xd0, 0xab}, /* ID3DUserDefinedAnnotation */
    {0xe1, 0xe7, 0x11, 0x24, 0xac, 0x12, 0xcf, 0x4c, 0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0}, /* IDXGIAdapter */
    {0x61, 0x8f, 0x03, 0x29, 0x39, 0x38, 0x26, 0x46, 0x91, 0xfd, 0x08, 0x68, 0x79, 0x01, 0x1a, 0x05}, /* IDXGIAdapter1 */
    {0x0a, 0xae, 0xa1, 0x0a, 0x0e, 0xfa, 0x84, 0x4b, 0x86, 0x44, 0xe0, 0x5f, 0xf8, 0xe5, 0xac, 0xb5}, /* IDXGIAdapter2 */
    {0xa4, 0x67, 0x59, 0x64, 0x92, 0x13, 0x10, 0x43, 0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd}, /* IDXGIAdapter3 */
    {0xd1, 0x99, 0x8d, 0x3c, 0xbf, 0x4f, 0x81, 0x41, 0xa8, 0x2c, 0xaf, 0x66, 0xbf, 0x7b, 0xd2, 0x4e}, /* IDXGIAdapter4 */
    {0x52, 0x74, 0x9e, 0x11, 0x9e, 0xde, 0xfe, 0x40, 0x88, 0x06, 0x88, 0xf9, 0x0c, 0x12, 0xb4, 0x41}, /* IDXGIDebug */
    {0x0c, 0x5f, 0xa0, 0xc5, 0xf2, 0x16, 0xdf, 0x4a, 0x9f, 0x4d, 0xa8, 0xc4, 0xd5, 0x8a, 0xc5, 0x50}, /* IDXGIDebug1 */
    {0x6b, 0x06, 0x33, 0x26, 0x14, 0x45, 0x7a, 0x4c, 0x8f, 0xd8, 0x12, 0xea, 0x98, 0x05, 0x9d, 0x18}, /* IDXGIDecodeSwapChain */
    {0xfa, 0x77, 0xec, 0x54, 0x77, 0x13, 0xe6, 0x44, 0x8c, 0x32, 0x88, 0xfd, 0x5f, 0x44, 0xc8, 0x4c}, /* IDXGIDevice */
    {0x0f, 0x97, 0xdb, 0x77, 0x76, 0x62, 0xba, 0x48, 0xba, 0x28, 0x07, 0x01, 0x43, 0xb4, 0x39, 0x2c}, /* IDXGIDevice1 */
    {0x17, 0x86, 0x00, 0x05, 0xfd, 0xfb, 0x51, 0x40, 0xa7, 0x90, 0x14, 0x48, 0x84, 0xb4, 0xf6, 0xa9}, /* IDXGIDevice2 */
    {0x6c, 0x89, 0x07, 0x60, 0x44, 0x32, 0xfd, 0x4a, 0xbf, 0x18, 0xa6, 0xd3, 0xbe, 0xda, 0x50, 0x23}, /* IDXGIDevice3 */
    {0x5f, 0xf9, 0xb4, 0x95, 0xda, 0xd8, 0xa4, 0x4c, 0x9e, 0xe6, 0x3b, 0x76, 0xd5, 0x96, 0x8a, 0x10}, /* IDXGIDevice4 */
    {0x79, 0x03, 0x3e, 0x3d, 0xde, 0xf9, 0x58, 0x4d, 0xbb, 0x6c, 0x18, 0xd6, 0x29, 0x92, 0xf1, 0xa6}, /* IDXGIDeviceSubObject */
    {0x1a, 0xbf, 0x9d, 0xea, 0x8e, 0xc8, 0x86, 0x44, 0x85, 0x4a, 0x98, 0xaa, 0x01, 0x38, 0xf3, 0x0c}, /* IDXGIDisplayControl */
    {0xec, 0x66, 0x71, 0x7b, 0xc7, 0x21, 0xae, 0x44, 0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69}, /* IDXGIFactory */
    {0x78, 0xae, 0x0a, 0x77, 0x6f, 0xf2, 0xba, 0x4d, 0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87}, /* IDXGIFactory1 */
    {0x1c, 0x3a, 0xc8, 0x50, 0x72, 0xe0, 0x48, 0x4c, 0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0}, /* IDXGIFactory2 */
    {0x23, 0x38, 0x48, 0x25, 0x46, 0xcd, 0x7d, 0x4c, 0x86, 0xca, 0x47, 0xaa, 0x95, 0xb8, 0x37, 0xbd}, /* IDXGIFactory3 */
    {0x02, 0xea, 0xc6, 0x1b, 0x36, 0xef, 0x4f, 0x46, 0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a}, /* IDXGIFactory4 */
    {0xf5, 0xe1, 0x32, 0x76, 0x65, 0xee, 0xca, 0x4d, 0x87, 0xfd, 0x84, 0xcd, 0x75, 0xf8, 0x83, 0x8d}, /* IDXGIFactory5 */
    {0x4f, 0x69, 0xb6, 0xc1, 0x09, 0xff, 0xa9, 0x44, 0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17}, /* IDXGIFactory6 */
    {0xed, 0x6e, 0x96, 0xa4, 0xdb, 0x76, 0xda, 0x44, 0x84, 0xc1, 0xee, 0x9a, 0x7a, 0xfb, 0x20, 0xa8}, /* IDXGIFactory7 */
    {0xf2, 0xd1, 0xe7, 0x41, 0x91, 0xa5, 0x7b, 0x4f, 0xa2, 0xe5, 0xfa, 0x9c, 0x84, 0x3e, 0x1c, 0x12}, /* IDXGIFactoryMedia */
    {0xc7, 0x41, 0x74, 0xd6, 0x2a, 0x67, 0x6f, 0x47, 0x9e, 0x82, 0xcd, 0x55, 0xb4, 0x49, 0x49, 0xce}, /* IDXGIInfoQueue */
    {0x89, 0x12, 0x8e, 0x9d, 0xb3, 0xd7, 0x5f, 0x46, 0x81, 0x26, 0x25, 0x0e, 0x34, 0x9a, 0xf8, 0x5d}, /* IDXGIKeyedMutex */
    {0xb8, 0x2f, 0xc2, 0xae, 0xf3, 0x76, 0x39, 0x46, 0x9b, 0xe0, 0x28, 0xeb, 0x43, 0xa6, 0x7a, 0x2e}, /* IDXGIObject */
    {0xdb, 0xee, 0x02, 0xae, 0x35, 0xc7, 0x90, 0x46, 0x8d, 0x52, 0x5a, 0x8d, 0xc2, 0x02, 0x13, 0xaa}, /* IDXGIOutput */
    {0xa8, 0xde, 0xcd, 0x00, 0x9b, 0x93, 0x83, 0x4b, 0xa3, 0x40, 0xa6, 0x85, 0x22, 0x66, 0x66, 0xcc}, /* IDXGIOutput1 */
    {0xd1, 0x39, 0x5e, 0x59, 0x24, 0x27, 0x63, 0x46, 0x99, 0xb1, 0xda, 0x96, 0x9d, 0xe2, 0x83, 0x64}, /* IDXGIOutput2 */
    {0x01, 0xb3, 0x6b, 0x8a, 0x7e, 0x7e, 0xf4, 0x41, 0xa8, 0xe0, 0x5b, 0x32, 0xf7, 0xf9, 0x9b, 0x18}, /* IDXGIOutput3 */
    {0x35, 0xca, 0x7d, 0xdc, 0x96, 0x21, 0x4d, 0x41, 0x9f, 0x53, 0x61, 0x78, 0x84, 0x03, 0x2a, 0x60}, /* IDXGIOutput4 */
    {0x24, 0x74, 0xa0, 0x80, 0x52, 0xab, 0xeb, 0x42, 0x83, 0x3c, 0x0c, 0x42, 0xfd, 0x28, 0x2d, 0x98}, /* IDXGIOutput5 */
    {0xe8, 0x46, 0x83, 0x06, 0xec, 0xaa, 0x84, 0x4b, 0xad, 0xd7, 0x13, 0x7f, 0x51, 0x3f, 0x77, 0xa1}, /* IDXGIOutput6 */
    {0xc3, 0xfa, 0x1c, 0x19, 0x41, 0xa3, 0x0d, 0x47, 0xb2, 0x6e, 0xa8, 0x64, 0xf4, 0x28, 0x31, 0x9c}, /* IDXGIOutputDuplication */
    {0xb4, 0x3a, 0x5f, 0x03, 0x2e, 0x48, 0x50, 0x4e, 0xb4, 0x1f, 0x8a, 0x7f, 0x8b, 0xd8, 0x96, 0x0b}, /* IDXGIResource */
    {0x79, 0x13, 0x96, 0x30, 0x09, 0x46, 0x41, 0x4a, 0x99, 0x8e, 0x54, 0xfe, 0x56, 0x7e, 0xe0, 0xc1}, /* IDXGIResource1 */
    {0x6c, 0xb5, 0xfc, 0xca, 0xc3, 0x6a, 0x89, 0x48, 0xbf, 0x47, 0x9e, 0x23, 0xbb, 0xd2, 0x60, 0xec}, /* IDXGISurface */
    {0x92, 0x30, 0xe6, 0x4a, 0x27, 0x63, 0x1b, 0x4c, 0x80, 0xae, 0xbf, 0xe1, 0x2e, 0xa3, 0x2b, 0x86}, /* IDXGISurface1 */
    {0xdd, 0x96, 0xa4, 0xab, 0x17, 0xb6, 0xb8, 0x4c, 0xa8, 0x66, 0xbc, 0x44, 0xd7, 0xeb, 0x1f, 0xa2}, /* IDXGISurface2 */
    {0xa0, 0x36, 0x0d, 0x31, 0xe7, 0xd2, 0x0a, 0x4c, 0xaa, 0x04, 0x6a, 0x9d, 0x23, 0xb8, 0x88, 0x6a}, /* IDXGISwapChain */
    {0xf7, 0x45, 0x0a, 0x79, 0x42, 0x0d, 0x76, 0x48, 0x98, 0x3a, 0x0a, 0x55, 0xcf, 0xe6, 0xf4, 0xaa}, /* IDXGISwapChain1 */
    {0xc4, 0x2a, 0xbe, 0xa8, 0x9f, 0x19, 0x46, 0x49, 0xb3, 0x31, 0x79, 0x59, 0x9f, 0xb9, 0x8d, 0xe7}, /* IDXGISwapChain2 */
    {0xdb, 0x9b, 0xd9, 0x94, 0xf8, 0xf1, 0xb0, 0x4a, 0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1}, /* IDXGISwapChain3 */
    {0x5a, 0x5d, 0x58, 0x3d, 0x4a, 0xbd, 0x9e, 0x48, 0xb1, 0xf4, 0x3d, 0xbc, 0xb6, 0x45, 0x2f, 0xfb}, /* IDXGISwapChain4 */
    {0x0b, 0xb9, 0x95, 0xdd, 0x5f, 0xf0, 0x6a, 0x4f, 0xbd, 0x65, 0x25, 0xbf, 0xb2, 0x64, 0xbd, 0x84}, /* IDXGISwapChainMedia */
    {0x14, 0x15, 0x25, 0x9f, 0x4d, 0x9d, 0x02, 0x49, 0x9d, 0x60, 0x18, 0x98, 0x8a, 0xb7, 0xd4, 0xb5}, /* IDXGraphicsAnalysis */
};

static const int BASE[INTERFACES] = {
    -1, /* ID3DBlob */
    -1, /* ID3DDestructionNotifier */
    22, /* ID3D11Asynchronous */
    22, /* ID3D11AuthenticatedChannel */
    22, /* ID3D11BlendState */
    4, /* ID3D11BlendState1 */
    46, /* ID3D11Buffer */
    22, /* ID3D11ClassInstance */
    22, /* ID3D11ClassLinkage */
    22, /* ID3D11CommandList */
    22, /* ID3D11ComputeShader */
    2, /* ID3D11Counter */
    22, /* ID3D11CryptoSession */
    -1, /* ID3D11Debug */
    22, /* ID3D11DepthStencilState */
    76, /* ID3D11DepthStencilView */
    -1, /* ID3D11Device */
    16, /* ID3D11Device1 */
    17, /* ID3D11Device2 */
    18, /* ID3D11Device3 */
    19, /* ID3D11Device4 */
    20, /* ID3D11Device5 */
    -1, /* ID3D11DeviceChild */
    22, /* ID3D11DeviceContext */
    23, /* ID3D11DeviceContext1 */
    24, /* ID3D11DeviceContext2 */
    25, /* ID3D11DeviceContext3 */
    26, /* ID3D11DeviceContext4 */
    22, /* ID3D11DomainShader */
    22, /* ID3D11Fence */
    22, /* ID3D11GeometryShader */
    22, /* ID3D11HullShader */
    -1, /* ID3D11InfoQueue */
    22, /* ID3D11InputLayout */
    -1, /* ID3D11Multithread */
    22, /* ID3D11PixelShader */
    37, /* ID3D11Predicate */
    2, /* ID3D11Query */
    37, /* ID3D11Query1 */
    22, /* ID3D11RasterizerState */
    39, /* ID3D11RasterizerState1 */
    40, /* ID3D11RasterizerState2 */
    -1, /* ID3D11RefDefaultTrackingOptions */
    -1, /* ID3D11RefTrackingOptions */
    76, /* ID3D11RenderTargetView */
    44, /* ID3D11RenderTargetView1 */
    22, /* ID3D11Resource */
    22, /* ID3D11SamplerState */
    76, /* ID3D11ShaderResourceView */
    48, /* ID3D11ShaderResourceView1 */
    -1, /* ID3D11ShaderTrace */
    -1, /* ID3D11ShaderTraceFactory */
    -1, /* ID3D11SwitchToRef */
    46, /* ID3D11Texture1D */
    46, /* ID3D11Texture2D */
    54, /* ID3D11Texture2D1 */
    46, /* ID3D11Texture3D */
    56, /* ID3D11Texture3D1 */
    -1, /* ID3D11TracingDevice */
    76, /* ID3D11UnorderedAccessView */
    59, /* ID3D11UnorderedAccessView1 */
    22, /* ID3D11VertexShader */
    22, /* ID3D11VideoContext */
    62, /* ID3D11VideoContext1 */
    63, /* ID3D11VideoContext2 */
    64, /* ID3D11VideoContext3 */
    22, /* ID3D11VideoDecoder */
    76, /* ID3D11VideoDecoderOutputView */
    -1, /* ID3D11VideoDevice */
    68, /* ID3D11VideoDevice1 */
    69, /* ID3D11VideoDevice2 */
    22, /* ID3D11VideoProcessor */
    22, /* ID3D11VideoProcessorEnumerator */
    72, /* ID3D11VideoProcessorEnumerator1 */
    76, /* ID3D11VideoProcessorInputView */
    76, /* ID3D11VideoProcessorOutputView */
    22, /* ID3D11View */
    22, /* ID3DDeviceContextState */
    -1, /* ID3DUserDefinedAnnotation */
    105, /* IDXGIAdapter */
    79, /* IDXGIAdapter1 */
    80, /* IDXGIAdapter2 */
    81, /* IDXGIAdapter3 */
    82, /* IDXGIAdapter4 */
    -1, /* IDXGIDebug */
    84, /* IDXGIDebug1 */
    -1, /* IDXGIDecodeSwapChain */
    105, /* IDXGIDevice */
    87, /* IDXGIDevice1 */
    88, /* IDXGIDevice2 */
    89, /* IDXGIDevice3 */
    90, /* IDXGIDevice4 */
    105, /* IDXGIDeviceSubObject */
    -1, /* IDXGIDisplayControl */
    105, /* IDXGIFactory */
    94, /* IDXGIFactory1 */
    95, /* IDXGIFactory2 */
    96, /* IDXGIFactory3 */
    97, /* IDXGIFactory4 */
    98, /* IDXGIFactory5 */
    99, /* IDXGIFactory6 */
    100, /* IDXGIFactory7 */
    -1, /* IDXGIFactoryMedia */
    -1, /* IDXGIInfoQueue */
    92, /* IDXGIKeyedMutex */
    -1, /* IDXGIObject */
    105, /* IDXGIOutput */
    106, /* IDXGIOutput1 */
    107, /* IDXGIOutput2 */
    108, /* IDXGIOutput3 */
    109, /* IDXGIOutput4 */
    110, /* IDXGIOutput5 */
    111, /* IDXGIOutput6 */
    105, /* IDXGIOutputDuplication */
    92, /* IDXGIResource */
    114, /* IDXGIResource1 */
    92, /* IDXGISurface */
    116, /* IDXGISurface1 */
    117, /* IDXGISurface2 */
    92, /* IDXGISwapChain */
    119, /* IDXGISwapChain1 */
    120, /* IDXGISwapChain2 */
    121, /* IDXGISwapChain3 */
    122, /* IDXGISwapChain4 */
    -1, /* IDXGISwapChainMedia */
    -1, /* IDXGraphicsAnalysis */
};

#define CASES 635
#define EXPORTS 8

/* The interface each case's entry is in. */
static const uint16_t CASE_INTERFACE[CASES ? CASES : 1] = {0, 0, 1, 1, 2, 3, 3, 3, 4, 5, 6, 7, 7, 7, 7, 8, 8, 9, 11, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 13, 14, 15, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 20, 20, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 26, 26, 26, 27, 27, 29, 29, 29, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 34, 34, 34, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 46, 46, 47, 48, 49, 50, 50, 50, 50, 50, 50, 50, 50, 51, 52, 52, 53, 54, 55, 56, 57, 58, 58, 59, 60, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 62, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 64, 64, 64, 64, 65, 65, 66, 66, 67, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 68, 69, 69, 69, 69, 70, 70, 71, 71, 72, 72, 72, 72, 72, 72, 73, 74, 75, 76, 78, 78, 78, 78, 79, 79, 79, 80, 81, 82, 82, 82, 82, 82, 82, 83, 84, 85, 85, 85, 86, 86, 86, 86, 86, 86, 86, 86, 86, 87, 87, 87, 87, 87, 88, 88, 89, 89, 89, 90, 91, 91, 92, 93, 93, 94, 94, 94, 94, 94, 95, 95, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 97, 98, 98, 99, 100, 101, 101, 102, 102, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 103, 104, 104, 105, 105, 105, 105, 106, 106, 106, 106, 106, 106, 106, 106, 106, 106, 106, 106, 107, 107, 107, 107, 108, 109, 110, 111, 112, 112, 113, 113, 113, 113, 113, 113, 113, 113, 114, 114, 114, 114, 115, 115, 116, 116, 116, 117, 117, 118, 119, 119, 119, 119, 119, 119, 119, 119, 119, 119, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 120, 121, 121, 121, 121, 121, 121, 121, 122, 122, 122, 122, 123, 124, 124, 124, 125, 125};

/* Whether each case, and each export, takes the id of the interface it hands out. */
static const uint8_t CASE_RIID[CASES ? CASES : 1] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
static const uint8_t EXPORT_RIID[EXPORTS ? EXPORTS : 1] = {1, 1, 1, 0, 0, 1, 0, 0};
/* Whether each case hands out an object of a kind its parameters do not name by id. */
static const uint8_t CASE_MAKES[CASES ? CASES : 1] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};

static const uint16_t CASES_OF_0[] = {0, 1};
#define COUNT_OF_0 2
static const uint16_t CASES_OF_1[] = {2, 3};
#define COUNT_OF_1 2
static const uint16_t CASES_OF_2[] = {4, 101, 102, 103, 104};
#define COUNT_OF_2 5
static const uint16_t CASES_OF_3[] = {5, 6, 7, 101, 102, 103, 104};
#define COUNT_OF_3 7
static const uint16_t CASES_OF_4[] = {8, 101, 102, 103, 104};
#define COUNT_OF_4 5
static const uint16_t CASES_OF_5[] = {8, 9, 101, 102, 103, 104};
#define COUNT_OF_5 6
static const uint16_t CASES_OF_6[] = {10, 101, 102, 103, 104, 298, 299, 300};
#define COUNT_OF_6 8
static const uint16_t CASES_OF_7[] = {11, 12, 13, 14, 101, 102, 103, 104};
#define COUNT_OF_7 8
static const uint16_t CASES_OF_8[] = {15, 16, 101, 102, 103, 104};
#define COUNT_OF_8 6
static const uint16_t CASES_OF_9[] = {17, 101, 102, 103, 104};
#define COUNT_OF_9 5
static const uint16_t CASES_OF_10[] = {101, 102, 103, 104};
#define COUNT_OF_10 4
static const uint16_t CASES_OF_11[] = {4, 18, 101, 102, 103, 104};
#define COUNT_OF_11 6
static const uint16_t CASES_OF_12[] = {19, 20, 21, 22, 23, 101, 102, 103, 104};
#define COUNT_OF_12 9
static const uint16_t CASES_OF_13[] = {24, 25, 26, 27, 28, 29, 30, 31, 32};
#define COUNT_OF_13 9
static const uint16_t CASES_OF_14[] = {33, 101, 102, 103, 104};
#define COUNT_OF_14 5
static const uint16_t CASES_OF_15[] = {34, 101, 102, 103, 104, 439};
#define COUNT_OF_15 6
static const uint16_t CASES_OF_16[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74};
#define COUNT_OF_16 40
static const uint16_t CASES_OF_17[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81};
#define COUNT_OF_17 47
static const uint16_t CASES_OF_18[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85};
#define COUNT_OF_18 51
static const uint16_t CASES_OF_19[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96};
#define COUNT_OF_19 62
static const uint16_t CASES_OF_20[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98};
#define COUNT_OF_20 64
static const uint16_t CASES_OF_21[] = {35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100};
#define COUNT_OF_21 66
static const uint16_t CASES_OF_22[] = {101, 102, 103, 104};
#define COUNT_OF_22 4
static const uint16_t CASES_OF_23[] = {101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212};
#define COUNT_OF_23 112
static const uint16_t CASES_OF_24[] = {101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231};
#define COUNT_OF_24 131
static const uint16_t CASES_OF_25[] = {101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241};
#define COUNT_OF_25 141
static const uint16_t CASES_OF_26[] = {101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244};
#define COUNT_OF_26 144
static const uint16_t CASES_OF_27[] = {101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246};
#define COUNT_OF_27 146
static const uint16_t CASES_OF_28[] = {101, 102, 103, 104};
#define COUNT_OF_28 4
static const uint16_t CASES_OF_29[] = {101, 102, 103, 104, 247, 248, 249};
#define COUNT_OF_29 7
static const uint16_t CASES_OF_30[] = {101, 102, 103, 104};
#define COUNT_OF_30 4
static const uint16_t CASES_OF_31[] = {101, 102, 103, 104};
#define COUNT_OF_31 4
static const uint16_t CASES_OF_32[] = {250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284};
#define COUNT_OF_32 35
static const uint16_t CASES_OF_33[] = {101, 102, 103, 104};
#define COUNT_OF_33 4
static const uint16_t CASES_OF_34[] = {285, 286, 287, 288};
#define COUNT_OF_34 4
static const uint16_t CASES_OF_35[] = {101, 102, 103, 104};
#define COUNT_OF_35 4
static const uint16_t CASES_OF_36[] = {4, 101, 102, 103, 104, 289};
#define COUNT_OF_36 6
static const uint16_t CASES_OF_37[] = {4, 101, 102, 103, 104, 289};
#define COUNT_OF_37 6
static const uint16_t CASES_OF_38[] = {4, 101, 102, 103, 104, 289, 290};
#define COUNT_OF_38 7
static const uint16_t CASES_OF_39[] = {101, 102, 103, 104, 291};
#define COUNT_OF_39 5
static const uint16_t CASES_OF_40[] = {101, 102, 103, 104, 291, 292};
#define COUNT_OF_40 6
static const uint16_t CASES_OF_41[] = {101, 102, 103, 104, 291, 292, 293};
#define COUNT_OF_41 7
static const uint16_t CASES_OF_42[] = {294};
#define COUNT_OF_42 1
static const uint16_t CASES_OF_43[] = {295};
#define COUNT_OF_43 1
static const uint16_t CASES_OF_44[] = {101, 102, 103, 104, 296, 439};
#define COUNT_OF_44 6
static const uint16_t CASES_OF_45[] = {101, 102, 103, 104, 296, 297, 439};
#define COUNT_OF_45 7
static const uint16_t CASES_OF_46[] = {101, 102, 103, 104, 298, 299, 300};
#define COUNT_OF_46 7
static const uint16_t CASES_OF_47[] = {101, 102, 103, 104, 301};
#define COUNT_OF_47 5
static const uint16_t CASES_OF_48[] = {101, 102, 103, 104, 302, 439};
#define COUNT_OF_48 6
static const uint16_t CASES_OF_49[] = {101, 102, 103, 104, 302, 303, 439};
#define COUNT_OF_49 7
static const uint16_t CASES_OF_50[] = {304, 305, 306, 307, 308, 309, 310, 311};
#define COUNT_OF_50 8
static const uint16_t CASES_OF_51[] = {312};
#define COUNT_OF_51 1
static const uint16_t CASES_OF_52[] = {313, 314};
#define COUNT_OF_52 2
static const uint16_t CASES_OF_53[] = {101, 102, 103, 104, 298, 299, 300, 315};
#define COUNT_OF_53 8
static const uint16_t CASES_OF_54[] = {101, 102, 103, 104, 298, 299, 300, 316};
#define COUNT_OF_54 8
static const uint16_t CASES_OF_55[] = {101, 102, 103, 104, 298, 299, 300, 316, 317};
#define COUNT_OF_55 9
static const uint16_t CASES_OF_56[] = {101, 102, 103, 104, 298, 299, 300, 318};
#define COUNT_OF_56 8
static const uint16_t CASES_OF_57[] = {101, 102, 103, 104, 298, 299, 300, 318, 319};
#define COUNT_OF_57 9
static const uint16_t CASES_OF_58[] = {320, 321};
#define COUNT_OF_58 2
static const uint16_t CASES_OF_59[] = {101, 102, 103, 104, 322, 439};
#define COUNT_OF_59 6
static const uint16_t CASES_OF_60[] = {101, 102, 103, 104, 322, 323, 439};
#define COUNT_OF_60 7
static const uint16_t CASES_OF_61[] = {101, 102, 103, 104};
#define COUNT_OF_61 4
static const uint16_t CASES_OF_62[] = {101, 102, 103, 104, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381};
#define COUNT_OF_62 62
static const uint16_t CASES_OF_63[] = {101, 102, 103, 104, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395};
#define COUNT_OF_63 76
static const uint16_t CASES_OF_64[] = {101, 102, 103, 104, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399};
#define COUNT_OF_64 80
static const uint16_t CASES_OF_65[] = {101, 102, 103, 104, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401};
#define COUNT_OF_65 82
static const uint16_t CASES_OF_66[] = {101, 102, 103, 104, 402, 403};
#define COUNT_OF_66 6
static const uint16_t CASES_OF_67[] = {101, 102, 103, 104, 404, 439};
#define COUNT_OF_67 6
static const uint16_t CASES_OF_68[] = {405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421};
#define COUNT_OF_68 17
static const uint16_t CASES_OF_69[] = {405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425};
#define COUNT_OF_69 21
static const uint16_t CASES_OF_70[] = {405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425, 426, 427};
#define COUNT_OF_70 23
static const uint16_t CASES_OF_71[] = {101, 102, 103, 104, 428, 429};
#define COUNT_OF_71 6
static const uint16_t CASES_OF_72[] = {101, 102, 103, 104, 430, 431, 432, 433, 434, 435};
#define COUNT_OF_72 10
static const uint16_t CASES_OF_73[] = {101, 102, 103, 104, 430, 431, 432, 433, 434, 435, 436};
#define COUNT_OF_73 11
static const uint16_t CASES_OF_74[] = {101, 102, 103, 104, 437, 439};
#define COUNT_OF_74 6
static const uint16_t CASES_OF_75[] = {101, 102, 103, 104, 438, 439};
#define COUNT_OF_75 6
static const uint16_t CASES_OF_76[] = {101, 102, 103, 104, 439};
#define COUNT_OF_76 5
static const uint16_t CASES_OF_77[] = {101, 102, 103, 104};
#define COUNT_OF_77 4
static const uint16_t CASES_OF_78[] = {440, 441, 442, 443};
#define COUNT_OF_78 4
static const uint16_t CASES_OF_79[] = {444, 445, 446, 551, 552, 553, 554};
#define COUNT_OF_79 7
static const uint16_t CASES_OF_80[] = {444, 445, 446, 447, 551, 552, 553, 554};
#define COUNT_OF_80 8
static const uint16_t CASES_OF_81[] = {444, 445, 446, 447, 448, 551, 552, 553, 554};
#define COUNT_OF_81 9
static const uint16_t CASES_OF_82[] = {444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 551, 552, 553, 554};
#define COUNT_OF_82 15
static const uint16_t CASES_OF_83[] = {444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 551, 552, 553, 554};
#define COUNT_OF_83 16
static const uint16_t CASES_OF_84[] = {456};
#define COUNT_OF_84 1
static const uint16_t CASES_OF_85[] = {456, 457, 458, 459};
#define COUNT_OF_85 4
static const uint16_t CASES_OF_86[] = {460, 461, 462, 463, 464, 465, 466, 467, 468};
#define COUNT_OF_86 9
static const uint16_t CASES_OF_87[] = {469, 470, 471, 472, 473, 551, 552, 553, 554};
#define COUNT_OF_87 9
static const uint16_t CASES_OF_88[] = {469, 470, 471, 472, 473, 474, 475, 551, 552, 553, 554};
#define COUNT_OF_88 11
static const uint16_t CASES_OF_89[] = {469, 470, 471, 472, 473, 474, 475, 476, 477, 478, 551, 552, 553, 554};
#define COUNT_OF_89 14
static const uint16_t CASES_OF_90[] = {469, 470, 471, 472, 473, 474, 475, 476, 477, 478, 479, 551, 552, 553, 554};
#define COUNT_OF_90 15
static const uint16_t CASES_OF_91[] = {469, 470, 471, 472, 473, 474, 475, 476, 477, 478, 479, 480, 481, 551, 552, 553, 554};
#define COUNT_OF_91 17
static const uint16_t CASES_OF_92[] = {482, 551, 552, 553, 554};
#define COUNT_OF_92 5
static const uint16_t CASES_OF_93[] = {483, 484};
#define COUNT_OF_93 2
static const uint16_t CASES_OF_94[] = {485, 486, 487, 488, 489, 551, 552, 553, 554};
#define COUNT_OF_94 9
static const uint16_t CASES_OF_95[] = {485, 486, 487, 488, 489, 490, 491, 551, 552, 553, 554};
#define COUNT_OF_95 11
static const uint16_t CASES_OF_96[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 551, 552, 553, 554};
#define COUNT_OF_96 22
static const uint16_t CASES_OF_97[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 551, 552, 553, 554};
#define COUNT_OF_97 23
static const uint16_t CASES_OF_98[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 551, 552, 553, 554};
#define COUNT_OF_98 25
static const uint16_t CASES_OF_99[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 551, 552, 553, 554};
#define COUNT_OF_99 26
static const uint16_t CASES_OF_100[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 507, 551, 552, 553, 554};
#define COUNT_OF_100 27
static const uint16_t CASES_OF_101[] = {485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504, 505, 506, 507, 508, 509, 551, 552, 553, 554};
#define COUNT_OF_101 29
static const uint16_t CASES_OF_102[] = {510, 511};
#define COUNT_OF_102 2
static const uint16_t CASES_OF_103[] = {512, 513, 514, 515, 516, 517, 518, 519, 520, 521, 522, 523, 524, 525, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 536, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548};
#define COUNT_OF_103 37
static const uint16_t CASES_OF_104[] = {482, 549, 550, 551, 552, 553, 554};
#define COUNT_OF_104 7
static const uint16_t CASES_OF_105[] = {551, 552, 553, 554};
#define COUNT_OF_105 4
static const uint16_t CASES_OF_106[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566};
#define COUNT_OF_106 16
static const uint16_t CASES_OF_107[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570};
#define COUNT_OF_107 20
static const uint16_t CASES_OF_108[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571};
#define COUNT_OF_108 21
static const uint16_t CASES_OF_109[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572};
#define COUNT_OF_109 22
static const uint16_t CASES_OF_110[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572, 573};
#define COUNT_OF_110 23
static const uint16_t CASES_OF_111[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572, 573, 574};
#define COUNT_OF_111 24
static const uint16_t CASES_OF_112[] = {551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572, 573, 574, 575, 576};
#define COUNT_OF_112 26
static const uint16_t CASES_OF_113[] = {551, 552, 553, 554, 577, 578, 579, 580, 581, 582, 583, 584};
#define COUNT_OF_113 12
static const uint16_t CASES_OF_114[] = {482, 551, 552, 553, 554, 585, 586, 587, 588};
#define COUNT_OF_114 9
static const uint16_t CASES_OF_115[] = {482, 551, 552, 553, 554, 585, 586, 587, 588, 589, 590};
#define COUNT_OF_115 11
static const uint16_t CASES_OF_116[] = {482, 551, 552, 553, 554, 591, 592, 593};
#define COUNT_OF_116 8
static const uint16_t CASES_OF_117[] = {482, 551, 552, 553, 554, 591, 592, 593, 594, 595};
#define COUNT_OF_117 10
static const uint16_t CASES_OF_118[] = {482, 551, 552, 553, 554, 591, 592, 593, 594, 595, 596};
#define COUNT_OF_118 11
static const uint16_t CASES_OF_119[] = {482, 551, 552, 553, 554, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606};
#define COUNT_OF_119 15
static const uint16_t CASES_OF_120[] = {482, 551, 552, 553, 554, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615, 616, 617};
#define COUNT_OF_120 26
static const uint16_t CASES_OF_121[] = {482, 551, 552, 553, 554, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615, 616, 617, 618, 619, 620, 621, 622, 623, 624};
#define COUNT_OF_121 33
static const uint16_t CASES_OF_122[] = {482, 551, 552, 553, 554, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615, 616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626, 627, 628};
#define COUNT_OF_122 37
static const uint16_t CASES_OF_123[] = {482, 551, 552, 553, 554, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615, 616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626, 627, 628, 629};
#define COUNT_OF_123 38
static const uint16_t CASES_OF_124[] = {630, 631, 632};
#define COUNT_OF_124 3
static const uint16_t CASES_OF_125[] = {633, 634};
#define COUNT_OF_125 2
static const uint16_t *const CASES_OF[INTERFACES] = {CASES_OF_0, CASES_OF_1, CASES_OF_2, CASES_OF_3, CASES_OF_4, CASES_OF_5, CASES_OF_6, CASES_OF_7, CASES_OF_8, CASES_OF_9, CASES_OF_10, CASES_OF_11, CASES_OF_12, CASES_OF_13, CASES_OF_14, CASES_OF_15, CASES_OF_16, CASES_OF_17, CASES_OF_18, CASES_OF_19, CASES_OF_20, CASES_OF_21, CASES_OF_22, CASES_OF_23, CASES_OF_24, CASES_OF_25, CASES_OF_26, CASES_OF_27, CASES_OF_28, CASES_OF_29, CASES_OF_30, CASES_OF_31, CASES_OF_32, CASES_OF_33, CASES_OF_34, CASES_OF_35, CASES_OF_36, CASES_OF_37, CASES_OF_38, CASES_OF_39, CASES_OF_40, CASES_OF_41, CASES_OF_42, CASES_OF_43, CASES_OF_44, CASES_OF_45, CASES_OF_46, CASES_OF_47, CASES_OF_48, CASES_OF_49, CASES_OF_50, CASES_OF_51, CASES_OF_52, CASES_OF_53, CASES_OF_54, CASES_OF_55, CASES_OF_56, CASES_OF_57, CASES_OF_58, CASES_OF_59, CASES_OF_60, CASES_OF_61, CASES_OF_62, CASES_OF_63, CASES_OF_64, CASES_OF_65, CASES_OF_66, CASES_OF_67, CASES_OF_68, CASES_OF_69, CASES_OF_70, CASES_OF_71, CASES_OF_72, CASES_OF_73, CASES_OF_74, CASES_OF_75, CASES_OF_76, CASES_OF_77, CASES_OF_78, CASES_OF_79, CASES_OF_80, CASES_OF_81, CASES_OF_82, CASES_OF_83, CASES_OF_84, CASES_OF_85, CASES_OF_86, CASES_OF_87, CASES_OF_88, CASES_OF_89, CASES_OF_90, CASES_OF_91, CASES_OF_92, CASES_OF_93, CASES_OF_94, CASES_OF_95, CASES_OF_96, CASES_OF_97, CASES_OF_98, CASES_OF_99, CASES_OF_100, CASES_OF_101, CASES_OF_102, CASES_OF_103, CASES_OF_104, CASES_OF_105, CASES_OF_106, CASES_OF_107, CASES_OF_108, CASES_OF_109, CASES_OF_110, CASES_OF_111, CASES_OF_112, CASES_OF_113, CASES_OF_114, CASES_OF_115, CASES_OF_116, CASES_OF_117, CASES_OF_118, CASES_OF_119, CASES_OF_120, CASES_OF_121, CASES_OF_122, CASES_OF_123, CASES_OF_124, CASES_OF_125};
static const uint32_t CASE_COUNT[INTERFACES] = {COUNT_OF_0, COUNT_OF_1, COUNT_OF_2, COUNT_OF_3, COUNT_OF_4, COUNT_OF_5, COUNT_OF_6, COUNT_OF_7, COUNT_OF_8, COUNT_OF_9, COUNT_OF_10, COUNT_OF_11, COUNT_OF_12, COUNT_OF_13, COUNT_OF_14, COUNT_OF_15, COUNT_OF_16, COUNT_OF_17, COUNT_OF_18, COUNT_OF_19, COUNT_OF_20, COUNT_OF_21, COUNT_OF_22, COUNT_OF_23, COUNT_OF_24, COUNT_OF_25, COUNT_OF_26, COUNT_OF_27, COUNT_OF_28, COUNT_OF_29, COUNT_OF_30, COUNT_OF_31, COUNT_OF_32, COUNT_OF_33, COUNT_OF_34, COUNT_OF_35, COUNT_OF_36, COUNT_OF_37, COUNT_OF_38, COUNT_OF_39, COUNT_OF_40, COUNT_OF_41, COUNT_OF_42, COUNT_OF_43, COUNT_OF_44, COUNT_OF_45, COUNT_OF_46, COUNT_OF_47, COUNT_OF_48, COUNT_OF_49, COUNT_OF_50, COUNT_OF_51, COUNT_OF_52, COUNT_OF_53, COUNT_OF_54, COUNT_OF_55, COUNT_OF_56, COUNT_OF_57, COUNT_OF_58, COUNT_OF_59, COUNT_OF_60, COUNT_OF_61, COUNT_OF_62, COUNT_OF_63, COUNT_OF_64, COUNT_OF_65, COUNT_OF_66, COUNT_OF_67, COUNT_OF_68, COUNT_OF_69, COUNT_OF_70, COUNT_OF_71, COUNT_OF_72, COUNT_OF_73, COUNT_OF_74, COUNT_OF_75, COUNT_OF_76, COUNT_OF_77, COUNT_OF_78, COUNT_OF_79, COUNT_OF_80, COUNT_OF_81, COUNT_OF_82, COUNT_OF_83, COUNT_OF_84, COUNT_OF_85, COUNT_OF_86, COUNT_OF_87, COUNT_OF_88, COUNT_OF_89, COUNT_OF_90, COUNT_OF_91, COUNT_OF_92, COUNT_OF_93, COUNT_OF_94, COUNT_OF_95, COUNT_OF_96, COUNT_OF_97, COUNT_OF_98, COUNT_OF_99, COUNT_OF_100, COUNT_OF_101, COUNT_OF_102, COUNT_OF_103, COUNT_OF_104, COUNT_OF_105, COUNT_OF_106, COUNT_OF_107, COUNT_OF_108, COUNT_OF_109, COUNT_OF_110, COUNT_OF_111, COUNT_OF_112, COUNT_OF_113, COUNT_OF_114, COUNT_OF_115, COUNT_OF_116, COUNT_OF_117, COUNT_OF_118, COUNT_OF_119, COUNT_OF_120, COUNT_OF_121, COUNT_OF_122, COUNT_OF_123, COUNT_OF_124, COUNT_OF_125};


struct State {
    uint64_t rng;
    void *held[INTERFACES][POOL];
    uint32_t count[INTERFACES];
    void *stale[STALE];
    uint32_t stale_at;
    uint64_t counted[COUNTED];
    /* How many times each case (then each export) was called on its own object. */
    uint64_t reached[CASES + EXPORTS + 1];
    /* While sweeping: `this` is always the object whose table is used, and an interface id asked is this one's
       (when not negative). */
    int exact;
    int forced_riid;
};

static uint64_t next(State *s) {
    uint64_t z = (s->rng += 0x9e3779b97f4a7c15ull);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
    return z ^ (z >> 31);
}

static uint64_t below(State *s, uint64_t bound) { return bound ? next(s) % bound : 0; }

static int is_a(int have, int want) {
    for (int at = have; at >= 0; at = BASE[at]) if (at == want) return 1;
    return 0;
}

static uint64_t pick_value(State *s) {
    static const uint64_t EDGES[] = {0, 1, 2, 3, 4, 7, 8, 16, 0x7f, 0x80, 0xff, 0x100, 0xffff, 0x10000, 0x7fffffff, 0x80000000u, 0xffffffffu, 0x100000000ull, 0x7fffffffffffffffull, 0xffffffffffffffffull};
    switch (below(s, 4)) {
    case 0: return EDGES[below(s, sizeof EDGES / sizeof EDGES[0])];
    case 1: return below(s, 16);
    case 2: return below(s, 1024);
    default: return next(s);
    }
}

static GuidAbi pick_guid(State *s) {
    GuidAbi guid;
    uint64_t words[2] = {next(s), next(s)};
    memcpy(&guid, words, sizeof guid);
    return guid;
}

static double pick_float(State *s) {
    static const double EDGES[] = {0.0, -0.0, 1.0, -1.0, 0.5, 1e30, -1e30, 1e-30};
    if (below(s, 2)) return EDGES[below(s, sizeof EDGES / sizeof EDGES[0])];
    uint32_t bits = (uint32_t)next(s);
    float value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

/* A count of C's choosing: mostly small, sometimes at the glue's limit, past it, or all ones. */
static uint64_t pick_count(State *s) {
    uint64_t roll = below(s, 1024);
    if (roll == 0) { s->counted[BIG_COUNTS]++; return MOST; }
    if (roll <= 8) { s->counted[PAST_LIMIT]++; return MOST + 1 + below(s, 1000); }
    if (roll <= 16) { s->counted[PAST_LIMIT]++; return 0xffffffffu; }
    if (roll < 64) return 0;
    if (roll < 256) return below(s, 300);
    return below(s, 9);
}

static void fill(State *s, uint8_t *bytes, uint64_t length) {
    if (!bytes) return;
    if (length > 4096) {
        memset(bytes, (int)below(s, 256), length);
        return;
    }
    for (uint64_t at = 0; at < length; at += 8) {
        uint64_t word = next(s);
        memcpy(bytes + at, &word, length - at < 8 ? length - at : 8);
    }
}

/* `count` elements of `size` bytes at exactly that size, or one element's memory past the glue's limit (which the glue
   must refuse before reading), or null now and then. */
static uint8_t *elements(State *s, uint64_t count, uint64_t size) {
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint64_t length = (count > MOST ? 1 : count) * size;
    uint8_t *bytes = malloc(length ? length : 1);
    if (bytes) fill(s, bytes, length);
    return bytes;
}

/* Bytes the callee writes: exactly `count` of them, or null now and then and past a mebibyte. */
static uint8_t *out_bytes(State *s, uint64_t count) {
    if (count > (1u << 20) || below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(count ? count : 1);
    if (bytes) memset(bytes, 0xa5, count);
    return bytes;
}

/* Elements the callee writes: exactly `count` of them, or one element's memory past the glue's limit (which the glue
   must refuse before writing), or null now and then. */
static uint8_t *out_elements(State *s, uint64_t count, uint64_t size) {
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint64_t length = (count > MOST ? 1 : count) * size;
    uint8_t *bytes = malloc(length ? length : 1);
    if (bytes) memset(bytes, 0xa5, length);
    return bytes;
}

/* A count by pointer: the room `count` (cut to 32 bits), or null now and then. */
static uint32_t *count_cell(State *s, uint64_t count) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint32_t *cell = malloc(sizeof(uint32_t));
    if (cell) *cell = (uint32_t)count;
    return cell;
}

static uint8_t *out_bytes_fixed(State *s, uint64_t size) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(size);
    if (bytes) memset(bytes, 0xa5, size);
    return bytes;
}

/* A structure in: zeros, words of small values (which a description's ranges let through now and then), or random. */
static uint8_t *maybe_bytes(State *s, uint64_t size) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(size);
    if (bytes) {
        switch (below(s, 4)) {
        case 0: memset(bytes, 0, size); break;
        case 1:
            for (uint64_t at = 0; at < size; at += 4) {
                uint32_t word = (uint32_t)(below(s, 3) ? 1 + below(s, 4) : below(s, 64));
                memcpy(bytes + at, &word, size - at < 4 ? size - at : 4);
            }
            break;
        default: fill(s, bytes, size); break;
        }
    }
    return bytes;
}

/* A text of `unit`-byte units ending at a zero unit, of a length of C's choosing (some long), or null now and then. */
static uint8_t *pick_text(State *s, uint64_t unit) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint64_t units = below(s, 8) == 0 ? below(s, 70000) : below(s, 40);
    uint8_t *bytes = malloc((units + 1) * unit);
    if (!bytes) return NULL;
    for (uint64_t at = 0; at < units * unit; at++) bytes[at] = (uint8_t)(1 + below(s, 255));
    memset(bytes + units * unit, 0, unit);
    return bytes;
}

/* A text past the glue's limit of 65,536 units (a range designator, which GCC and Clang take). */
static const char LONG_TEXT[70001] = { [0 ... 69999] = 'A' };

/* Writes at `at` the address of a text of ours inside a structure: short, empty, past the glue's limit, or null now and
   then; never the address random bytes would make. */
static void put_text(State *s, uint8_t *at) {
    static const char *const TEXTS[] = { "POSITION", "TEXCOORD", "SV_Position", "", "COLOR" };
    const char *text;
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; text = NULL; }
    else if (below(s, 16) == 0) text = LONG_TEXT;
    else text = TEXTS[below(s, sizeof TEXTS / sizeof TEXTS[0])];
    memcpy(at, &text, sizeof text);
}

static uint8_t *pick_riid(State *s, int *interface) {
    if (s->forced_riid >= 0) {
        uint8_t *forced = malloc(16);
        if (forced) { memcpy(forced, IIDS[s->forced_riid], 16); *interface = s->forced_riid; }
        return forced;
    }
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(16);
    if (!bytes) return NULL;
    if (below(s, 8) == 0) { fill(s, bytes, 16); return bytes; }
    int at = (int)below(s, INTERFACES);
    memcpy(bytes, IIDS[at], 16);
    *interface = at;
    return bytes;
}

static void *any_held(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (s->count[at]) return s->held[at][below(s, s->count[at])];
    }
    return NULL;
}

/* An object parameter: one of ours (any kind), null, a released one, an address that is no object, or (`foreign`) the
   caller's own COM object. Without `foreign`, only what a COM call may be made on. */
static void *pick_object(State *s, int any_address) {
    switch (below(s, 16)) {
    case 0: s->counted[NULL_POINTERS]++; return NULL;
    case 1: { void *stale = s->stale[below(s, STALE)]; if (stale) return stale; break; }
    case 2: if (any_address) return (void *)(uintptr_t)((next(s) & 0x00007ffffffffff8ull) | 0x1000); break;
    case 3: return &FOREIGN;
    default: break;
    }
    return any_held(s);
}

/* A held object and the interface it was obtained as. */
static void *held_as(State *s, int *interface) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (s->count[at]) { *interface = (int)at; return s->held[at][below(s, s->count[at])]; }
    }
    return NULL;
}

/* `this`: mostly the object whose table is used, else null, a released object, an object of any kind, or an address
   that is no object (the glue finds its objects by address, never reading one it does not know). */
static void *pick_this(State *s, void *right) {
    if (s->exact) return right;
    switch (below(s, 16)) {
    case 0: s->counted[NULL_THIS]++; return NULL;
    case 1: { void *stale = s->stale[below(s, STALE)]; if (stale) { s->counted[STALE_THIS]++; return stale; } return right; }
    case 2: s->counted[GARBAGE_THIS]++; return (void *)(uintptr_t)((next(s) & 0x00007ffffffffff8ull) | 0x1000);
    case 3: { void *other = any_held(s); s->counted[WRONG_THIS]++; return other ? other : right; }
    default: return right;
    }
}

typedef uint32_t (*Counting)(void *);
typedef int32_t (*Querying)(void *, const uint8_t *, void **);

static void release(State *s, void *object) {
    if (!object) return;
    ((Counting)(*(void ***)object)[2])(object);
    s->counted[RELEASED]++;
}

/* One reference handed out, kept as the interface it was asked as; given back when the pool is full or the
   interface is not one the caller knows. */
static void adopt(State *s, void *object, int interface) {
    if (!object) return;
    if (interface < 0 || s->count[interface] >= POOL) { release(s, object); return; }
    s->held[interface][s->count[interface]++] = object;
    s->counted[ADOPTED]++;
}

/* Gives one held reference back and keeps its address as a released one. */
static void release_one(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (!s->count[at]) continue;
        uint32_t which = (uint32_t)below(s, s->count[at]);
        void *object = s->held[at][which];
        s->held[at][which] = s->held[at][--s->count[at]];
        release(s, object);
        s->stale[s->stale_at++ % STALE] = object;
        return;
    }
}

static void query_one(State *s) {
    void *object = any_held(s);
    if (!object) return;
    int interface = -1;
    uint8_t *riid = pick_riid(s, &interface);
    void *out = (void *)0x1;
    void **into = below(s, 16) ? &out : NULL;
    int32_t status = ((Querying)(*(void ***)object)[0])(pick_this(s, object), riid, into);
    s->counted[QUERY_INTERFACES]++;
    if (into && status >= 0) adopt(s, out, interface);
    else if (into && out != NULL) s->counted[QUERY_LEFT_POINTER]++;
    free(riid);
}

static void add_ref_one(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (!s->count[at] || s->count[at] >= POOL) continue;
        void *object = s->held[at][below(s, s->count[at])];
        ((Counting)(*(void ***)object)[1])(object);
        s->held[at][s->count[at]++] = object;
        return;
    }
}
static void call_slot(State *s, uint64_t which, void *right) {
    switch (which) {
    case 0: { /* ID3DBlob::GetBufferPointer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[0]++;
        typedef uint8_t * (*F)(void *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        s->counted[REFUSED]++;
        break;
    }
    case 1: { /* ID3DBlob::GetBufferSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[1]++;
        typedef uintptr_t (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 2: { /* ID3DDestructionNotifier::RegisterDestructionCallback */
        void *this = pick_this(s, right);
        if (this == right) s->reached[2]++;
        uint8_t *w2 = out_bytes_fixed(s, 4);
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uintptr_t)pick_value(s), (uint8_t *)pick_object(s, 1), w2) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w2) { for (uint64_t i = 0; i < 4; i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        s->counted[REFUSED]++;
        break;
    }
    case 3: { /* ID3DDestructionNotifier::UnregisterDestructionCallback */
        void *this = pick_this(s, right);
        if (this == right) s->reached[3]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 4: { /* ID3D11Asynchronous::GetDataSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[4]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 5: { /* ID3D11AuthenticatedChannel::GetCertificateSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[5]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 6: { /* ID3D11AuthenticatedChannel::GetCertificate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[6]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = out_bytes(s, n0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 7: { /* ID3D11AuthenticatedChannel::GetChannelHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[7]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 8: { /* ID3D11BlendState::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[8]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 264);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 9: { /* ID3D11BlendState1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[9]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 328);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 10: { /* ID3D11Buffer::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[10]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 11: { /* ID3D11ClassInstance::GetClassLinkage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[11]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 8);
        free((void *)a0);
        break;
    }
    case 12: { /* ID3D11ClassInstance::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[12]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 32);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 13: { /* ID3D11ClassInstance::GetInstanceName */
        void *this = pick_this(s, right);
        if (this == right) s->reached[13]++;
        typedef void (*F)(void *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (const uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 14: { /* ID3D11ClassInstance::GetTypeName */
        void *this = pick_this(s, right);
        if (this == right) s->reached[14]++;
        typedef void (*F)(void *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (const uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 15: { /* ID3D11ClassLinkage::GetClassInstance */
        void *this = pick_this(s, right);
        if (this == right) s->reached[15]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 7);
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 16: { /* ID3D11ClassLinkage::CreateClassInstance */
        void *this = pick_this(s, right);
        if (this == right) s->reached[16]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, uint32_t, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint32_t a4 = (uint32_t)pick_value(s);
        void **a5 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4, a5) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a5) adopt(s, ok ? *a5 : NULL, 7);
        free((void *)a0);
        free((void *)a5);
        break;
    }
    case 17: { /* ID3D11CommandList::GetContextFlags */
        void *this = pick_this(s, right);
        if (this == right) s->reached[17]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 18: { /* ID3D11Counter::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[18]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 19: { /* ID3D11CryptoSession::GetCryptoType */
        void *this = pick_this(s, right);
        if (this == right) s->reached[19]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 20: { /* ID3D11CryptoSession::GetDecoderProfile */
        void *this = pick_this(s, right);
        if (this == right) s->reached[20]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 21: { /* ID3D11CryptoSession::GetCertificateSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[21]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 22: { /* ID3D11CryptoSession::GetCertificate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[22]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = out_bytes(s, n0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 23: { /* ID3D11CryptoSession::GetCryptoSessionHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[23]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 24: { /* ID3D11Debug::SetFeatureMask */
        void *this = pick_this(s, right);
        if (this == right) s->reached[24]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 25: { /* ID3D11Debug::GetFeatureMask */
        void *this = pick_this(s, right);
        if (this == right) s->reached[25]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 26: { /* ID3D11Debug::SetPresentPerRenderOpDelay */
        void *this = pick_this(s, right);
        if (this == right) s->reached[26]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 27: { /* ID3D11Debug::GetPresentPerRenderOpDelay */
        void *this = pick_this(s, right);
        if (this == right) s->reached[27]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 28: { /* ID3D11Debug::SetSwapChain */
        void *this = pick_this(s, right);
        if (this == right) s->reached[28]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 29: { /* ID3D11Debug::GetSwapChain */
        void *this = pick_this(s, right);
        if (this == right) s->reached[29]++;
        typedef int32_t (*F)(void *, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 119);
        free((void *)a0);
        break;
    }
    case 30: { /* ID3D11Debug::ValidateContext */
        void *this = pick_this(s, right);
        if (this == right) s->reached[30]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 31: { /* ID3D11Debug::ReportLiveDeviceObjects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[31]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 32: { /* ID3D11Debug::ValidateContextForDispatch */
        void *this = pick_this(s, right);
        if (this == right) s->reached[32]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 33: { /* ID3D11DepthStencilState::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[33]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 52);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 34: { /* ID3D11DepthStencilView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[34]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 35: { /* ID3D11Device::CreateBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[35]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 24);
        const uint8_t *a1 = NULL;
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 6);
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 36: { /* ID3D11Device::CreateTexture1D */
        void *this = pick_this(s, right);
        if (this == right) s->reached[36]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 32);
        const uint8_t *a1 = NULL;
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 53);
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 37: { /* ID3D11Device::CreateTexture2D */
        void *this = pick_this(s, right);
        if (this == right) s->reached[37]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 44);
        const uint8_t *a1 = NULL;
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 54);
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 38: { /* ID3D11Device::CreateTexture3D */
        void *this = pick_this(s, right);
        if (this == right) s->reached[38]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 36);
        const uint8_t *a1 = NULL;
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 56);
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 39: { /* ID3D11Device::CreateShaderResourceView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[39]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 24);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 48);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 40: { /* ID3D11Device::CreateUnorderedAccessView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[40]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 20);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 59);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 41: { /* ID3D11Device::CreateRenderTargetView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[41]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 20);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 44);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 42: { /* ID3D11Device::CreateDepthStencilView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[42]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 24);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 15);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 43: { /* ID3D11Device::CreateInputLayout */
        void *this = pick_this(s, right);
        if (this == right) s->reached[43]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *, uintptr_t, void **);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint64_t n3 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 32);
        if (a0) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) put_text(s, a0 + i * 32 + 0);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n3, 1);
        uintptr_t a3 = (uintptr_t)n3;
        void **a4 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a4) adopt(s, ok ? *a4 : NULL, 33);
        free((void *)a0);
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 44: { /* ID3D11Device::CreateVertexShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[44]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 61);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 45: { /* ID3D11Device::CreateGeometryShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[45]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 30);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 46: { /* ID3D11Device::CreateGeometryShaderWithStreamOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[46]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, const uint8_t *, uint32_t, const uint8_t *, uint32_t, uint32_t, void *, void **);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint64_t n3 = pick_count(s);
        uint64_t n5 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        uint8_t *a2 = elements(s, n3, 24);
        if (a2) for (uint64_t i = 0; i < (n3 > MOST ? 1 : n3); i++) put_text(s, a2 + i * 24 + 8);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = elements(s, n5, 4);
        uint32_t a5 = (uint32_t)n5;
        uint32_t a6 = (uint32_t)pick_value(s);
        void *a7 = pick_object(s, 1);
        void **a8 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6, a7, a8) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a8) adopt(s, ok ? *a8 : NULL, 30);
        free((void *)a0);
        free((void *)a2);
        free((void *)a4);
        free((void *)a8);
        break;
    }
    case 47: { /* ID3D11Device::CreatePixelShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[47]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 35);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 48: { /* ID3D11Device::CreateHullShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[48]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 31);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 49: { /* ID3D11Device::CreateDomainShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[49]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 28);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 50: { /* ID3D11Device::CreateComputeShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[50]++;
        typedef int32_t (*F)(void *, const uint8_t *, uintptr_t, void *, void **);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = elements(s, n1, 1);
        uintptr_t a1 = (uintptr_t)n1;
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 10);
        free((void *)a0);
        free((void *)a3);
        break;
    }
    case 51: { /* ID3D11Device::CreateClassLinkage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[51]++;
        typedef int32_t (*F)(void *, void **);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 8);
        free((void *)a0);
        break;
    }
    case 52: { /* ID3D11Device::CreateBlendState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[52]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 264);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 4);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 53: { /* ID3D11Device::CreateDepthStencilState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[53]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 52);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 14);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 54: { /* ID3D11Device::CreateRasterizerState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[54]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 40);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 39);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 55: { /* ID3D11Device::CreateSamplerState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[55]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 52);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 47);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 56: { /* ID3D11Device::CreateQuery */
        void *this = pick_this(s, right);
        if (this == right) s->reached[56]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 8);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 37);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 57: { /* ID3D11Device::CreatePredicate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[57]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 8);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 36);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 58: { /* ID3D11Device::CreateCounter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[58]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 8);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 11);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 59: { /* ID3D11Device::CreateDeferredContext */
        void *this = pick_this(s, right);
        if (this == right) s->reached[59]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 23);
        free((void *)a1);
        break;
    }
    case 60: { /* ID3D11Device::OpenSharedResource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[60]++;
        typedef int32_t (*F)(void *, uintptr_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 61: { /* ID3D11Device::CheckFormatSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[61]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 62: { /* ID3D11Device::CheckMultisampleQualityLevels */
        void *this = pick_this(s, right);
        if (this == right) s->reached[62]++;
        typedef int32_t (*F)(void *, int32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 63: { /* ID3D11Device::CheckCounterInfo */
        void *this = pick_this(s, right);
        if (this == right) s->reached[63]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 12);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 64: { /* ID3D11Device::CheckCounter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[64]++;
        uint8_t *w1 = out_bytes_fixed(s, 4);
        uint8_t *w2 = out_bytes_fixed(s, 4);
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *, const uint8_t *, uint8_t *, const uint8_t *, uint8_t *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), w1, w2, (const uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (const uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (const uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w1) { for (uint64_t i = 0; i < 4; i++) if (w1[i]) { s->counted[REFUSED_WRONG]++; break; } free(w1); }
        if (w2) { for (uint64_t i = 0; i < 4; i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        s->counted[REFUSED]++;
        break;
    }
    case 65: { /* ID3D11Device::CheckFeatureSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[65]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes(s, n2);
        uint32_t a2 = (uint32_t)n2;
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 66: { /* ID3D11Device::GetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[66]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 67: { /* ID3D11Device::SetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[67]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = maybe_bytes(s, 16);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 68: { /* ID3D11Device::SetPrivateDataInterface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[68]++;
        typedef int32_t (*F)(void *, const uint8_t *, void *);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        void *a1 = pick_object(s, 0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 69: { /* ID3D11Device::GetFeatureLevel */
        void *this = pick_this(s, right);
        if (this == right) s->reached[69]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 70: { /* ID3D11Device::GetCreationFlags */
        void *this = pick_this(s, right);
        if (this == right) s->reached[70]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[38]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 71: { /* ID3D11Device::GetDeviceRemovedReason */
        void *this = pick_this(s, right);
        if (this == right) s->reached[71]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[39]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 72: { /* ID3D11Device::GetImmediateContext */
        void *this = pick_this(s, right);
        if (this == right) s->reached[72]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[40]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 23);
        free((void *)a0);
        break;
    }
    case 73: { /* ID3D11Device::SetExceptionMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[73]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[41]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 74: { /* ID3D11Device::GetExceptionMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[74]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[42]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 75: { /* ID3D11Device1::GetImmediateContext1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[75]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[43]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 24);
        free((void *)a0);
        break;
    }
    case 76: { /* ID3D11Device1::CreateDeferredContext1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[76]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[44]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 24);
        free((void *)a1);
        break;
    }
    case 77: { /* ID3D11Device1::CreateBlendState1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[77]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[45]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 328);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 5);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 78: { /* ID3D11Device1::CreateRasterizerState1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[78]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[46]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 44);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 40);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 79: { /* ID3D11Device1::CreateDeviceContextState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[79]++;
        typedef int32_t (*F)(void *, uint32_t, const uint8_t *, uint32_t, uint32_t, const uint8_t *, uint8_t *, void **);
        F f = (F)((*(void ***)right)[47]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = elements(s, n2, 4);
        uint32_t a2 = (uint32_t)n2;
        uint32_t a3 = (uint32_t)pick_value(s);
        uint8_t *a4 = maybe_bytes(s, 16);
        uint8_t *a5 = out_bytes_fixed(s, 4);
        void **a6 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a6) adopt(s, ok ? *a6 : NULL, 77);
        free((void *)a1);
        free((void *)a4);
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 80: { /* ID3D11Device1::OpenSharedResource1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[80]++;
        typedef int32_t (*F)(void *, uintptr_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[48]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 81: { /* ID3D11Device1::OpenSharedResourceByName */
        void *this = pick_this(s, right);
        if (this == right) s->reached[81]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[49]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 2);
        uint32_t a1 = (uint32_t)pick_value(s);
        int a2_interface = -1;
        uint8_t *a2 = pick_riid(s, &a2_interface);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, a2_interface);
        free((void *)a0);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 82: { /* ID3D11Device2::GetImmediateContext2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[82]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[50]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 25);
        free((void *)a0);
        break;
    }
    case 83: { /* ID3D11Device2::CreateDeferredContext2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[83]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[51]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 25);
        free((void *)a1);
        break;
    }
    case 84: { /* ID3D11Device2::GetResourceTiling */
        void *this = pick_this(s, right);
        if (this == right) s->reached[84]++;
        uint8_t *w1 = out_bytes_fixed(s, 4);
        uint8_t *w2 = out_bytes_fixed(s, 12);
        uint8_t *w3 = out_bytes_fixed(s, 12);
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *, uint8_t *, uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[52]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (void *)pick_object(s, 1), w1, w2, w3, (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1));
        if (w1) { for (uint64_t i = 0; i < 4; i++) if (w1[i]) { s->counted[REFUSED_WRONG]++; break; } free(w1); }
        if (w2) { for (uint64_t i = 0; i < 12; i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        if (w3) { for (uint64_t i = 0; i < 12; i++) if (w3[i]) { s->counted[REFUSED_WRONG]++; break; } free(w3); }
        s->counted[REFUSED]++;
        break;
    }
    case 85: { /* ID3D11Device2::CheckMultisampleQualityLevels1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[85]++;
        typedef int32_t (*F)(void *, int32_t, uint32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[53]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 86: { /* ID3D11Device3::CreateTexture2D1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[86]++;
        uint8_t *w2 = out_bytes_fixed(s, sizeof(void *));
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[54]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), w2) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w2) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        s->counted[REFUSED]++;
        break;
    }
    case 87: { /* ID3D11Device3::CreateTexture3D1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[87]++;
        uint8_t *w2 = out_bytes_fixed(s, sizeof(void *));
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[55]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), w2) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w2) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        s->counted[REFUSED]++;
        break;
    }
    case 88: { /* ID3D11Device3::CreateRasterizerState2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[88]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[56]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 48);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 41);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 89: { /* ID3D11Device3::CreateShaderResourceView1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[89]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[57]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 28);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 49);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 90: { /* ID3D11Device3::CreateUnorderedAccessView1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[90]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[58]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 24);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 60);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 91: { /* ID3D11Device3::CreateRenderTargetView1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[91]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[59]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 24);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 45);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 92: { /* ID3D11Device3::CreateQuery1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[92]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[60]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 12);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 38);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 93: { /* ID3D11Device3::GetImmediateContext3 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[93]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[61]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 26);
        free((void *)a0);
        break;
    }
    case 94: { /* ID3D11Device3::CreateDeferredContext3 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[94]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[62]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 26);
        free((void *)a1);
        break;
    }
    case 95: { /* ID3D11Device3::WriteToSubresource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[95]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[63]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (uint32_t)pick_value(s));
        s->counted[REFUSED]++;
        break;
    }
    case 96: { /* ID3D11Device3::ReadFromSubresource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[96]++;
        typedef void (*F)(void *, uint8_t *, uint32_t, uint32_t, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[64]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (uint32_t)pick_value(s), (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 97: { /* ID3D11Device4::RegisterDeviceRemovedEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[97]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[65]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 98: { /* ID3D11Device4::UnregisterDeviceRemoved */
        void *this = pick_this(s, right);
        if (this == right) s->reached[98]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[66]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 99: { /* ID3D11Device5::OpenSharedFence */
        void *this = pick_this(s, right);
        if (this == right) s->reached[99]++;
        typedef int32_t (*F)(void *, uintptr_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[67]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 100: { /* ID3D11Device5::CreateFence */
        void *this = pick_this(s, right);
        if (this == right) s->reached[100]++;
        typedef int32_t (*F)(void *, uint64_t, int32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[68]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t a0 = (uint64_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int a2_interface = -1;
        uint8_t *a2 = pick_riid(s, &a2_interface);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, a2_interface);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 101: { /* ID3D11DeviceChild::GetDevice */
        void *this = pick_this(s, right);
        if (this == right) s->reached[101]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 16);
        free((void *)a0);
        break;
    }
    case 102: { /* ID3D11DeviceChild::GetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[102]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 103: { /* ID3D11DeviceChild::SetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[103]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = maybe_bytes(s, 16);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 104: { /* ID3D11DeviceChild::SetPrivateDataInterface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[104]++;
        typedef int32_t (*F)(void *, const uint8_t *, void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        void *a1 = pick_object(s, 0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 105: { /* ID3D11DeviceContext::VSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[105]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 106: { /* ID3D11DeviceContext::PSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[106]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 107: { /* ID3D11DeviceContext::PSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[107]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 108: { /* ID3D11DeviceContext::PSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[108]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 109: { /* ID3D11DeviceContext::VSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[109]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 110: { /* ID3D11DeviceContext::DrawIndexed */
        void *this = pick_this(s, right);
        if (this == right) s->reached[110]++;
        typedef void (*F)(void *, uint32_t, uint32_t, int32_t);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 111: { /* ID3D11DeviceContext::Draw */
        void *this = pick_this(s, right);
        if (this == right) s->reached[111]++;
        typedef void (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 112: { /* ID3D11DeviceContext::Map */
        void *this = pick_this(s, right);
        if (this == right) s->reached[112]++;
        typedef int32_t (*F)(void *, void *, uint32_t, int32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint8_t *a4 = out_bytes_fixed(s, 16);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 113: { /* ID3D11DeviceContext::Unmap */
        void *this = pick_this(s, right);
        if (this == right) s->reached[113]++;
        typedef void (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 114: { /* ID3D11DeviceContext::PSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[114]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 115: { /* ID3D11DeviceContext::IASetInputLayout */
        void *this = pick_this(s, right);
        if (this == right) s->reached[115]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 116: { /* ID3D11DeviceContext::IASetVertexBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[116]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 117: { /* ID3D11DeviceContext::IASetIndexBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[117]++;
        typedef void (*F)(void *, void *, int32_t, uint32_t);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 118: { /* ID3D11DeviceContext::DrawIndexedInstanced */
        void *this = pick_this(s, right);
        if (this == right) s->reached[118]++;
        typedef void (*F)(void *, uint32_t, uint32_t, uint32_t, int32_t, uint32_t);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint32_t a4 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 119: { /* ID3D11DeviceContext::DrawInstanced */
        void *this = pick_this(s, right);
        if (this == right) s->reached[119]++;
        typedef void (*F)(void *, uint32_t, uint32_t, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 120: { /* ID3D11DeviceContext::GSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[120]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 121: { /* ID3D11DeviceContext::GSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[121]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 122: { /* ID3D11DeviceContext::IASetPrimitiveTopology */
        void *this = pick_this(s, right);
        if (this == right) s->reached[122]++;
        typedef void (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 123: { /* ID3D11DeviceContext::VSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[123]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 124: { /* ID3D11DeviceContext::VSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[124]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 125: { /* ID3D11DeviceContext::Begin */
        void *this = pick_this(s, right);
        if (this == right) s->reached[125]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 126: { /* ID3D11DeviceContext::End */
        void *this = pick_this(s, right);
        if (this == right) s->reached[126]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 127: { /* ID3D11DeviceContext::GetData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[127]++;
        typedef int32_t (*F)(void *, void *, uint8_t *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes(s, n2);
        uint32_t a2 = (uint32_t)n2;
        uint32_t a3 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 128: { /* ID3D11DeviceContext::SetPredication */
        void *this = pick_this(s, right);
        if (this == right) s->reached[128]++;
        typedef void (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 129: { /* ID3D11DeviceContext::GSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[129]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 130: { /* ID3D11DeviceContext::GSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[130]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 131: { /* ID3D11DeviceContext::OMSetRenderTargets */
        void *this = pick_this(s, right);
        if (this == right) s->reached[131]++;
        typedef void (*F)(void *, uint32_t, void *const *, void *);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        void *a2 = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 132: { /* ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews */
        void *this = pick_this(s, right);
        if (this == right) s->reached[132]++;
        typedef void (*F)(void *, uint32_t, void *const *, void *, uint32_t, uint32_t, void *const *, const uint8_t *);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint64_t n4 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        void *a2 = pick_object(s, 1);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint32_t a4 = (uint32_t)n4;
        void **a5 = (void **)elements(s, n4, sizeof(void *));
        if (a5) for (uint64_t i = 0; i < (n4 > MOST ? 1 : n4); i++) a5[i] = pick_object(s, 1);
        uint8_t *a6 = elements(s, n4, 4);
        f(this, a0, a1, a2, a3, a4, a5, a6);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 133: { /* ID3D11DeviceContext::OMSetBlendState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[133]++;
        typedef void (*F)(void *, void *, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint32_t a2 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 134: { /* ID3D11DeviceContext::OMSetDepthStencilState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[134]++;
        typedef void (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 135: { /* ID3D11DeviceContext::SOSetTargets */
        void *this = pick_this(s, right);
        if (this == right) s->reached[135]++;
        typedef void (*F)(void *, uint32_t, void *const *, const uint8_t *);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        uint8_t *a2 = elements(s, n0, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 136: { /* ID3D11DeviceContext::DrawAuto */
        void *this = pick_this(s, right);
        if (this == right) s->reached[136]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[38]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 137: { /* ID3D11DeviceContext::DrawIndexedInstancedIndirect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[137]++;
        typedef void (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[39]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 138: { /* ID3D11DeviceContext::DrawInstancedIndirect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[138]++;
        typedef void (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[40]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 139: { /* ID3D11DeviceContext::Dispatch */
        void *this = pick_this(s, right);
        if (this == right) s->reached[139]++;
        typedef void (*F)(void *, uint32_t, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[41]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 140: { /* ID3D11DeviceContext::DispatchIndirect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[140]++;
        typedef void (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[42]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 141: { /* ID3D11DeviceContext::RSSetState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[141]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[43]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 142: { /* ID3D11DeviceContext::RSSetViewports */
        void *this = pick_this(s, right);
        if (this == right) s->reached[142]++;
        typedef void (*F)(void *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[44]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = elements(s, n0, 24);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 143: { /* ID3D11DeviceContext::RSSetScissorRects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[143]++;
        typedef void (*F)(void *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[45]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = elements(s, n0, 16);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 144: { /* ID3D11DeviceContext::CopySubresourceRegion */
        void *this = pick_this(s, right);
        if (this == right) s->reached[144]++;
        typedef void (*F)(void *, void *, uint32_t, uint32_t, uint32_t, uint32_t, void *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[46]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint32_t a4 = (uint32_t)pick_value(s);
        void *a5 = pick_object(s, 1);
        uint32_t a6 = (uint32_t)pick_value(s);
        uint8_t *a7 = maybe_bytes(s, 24);
        f(this, a0, a1, a2, a3, a4, a5, a6, a7);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a7);
        break;
    }
    case 145: { /* ID3D11DeviceContext::CopyResource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[145]++;
        typedef void (*F)(void *, void *, void *);
        F f = (F)((*(void ***)right)[47]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 146: { /* ID3D11DeviceContext::UpdateSubresource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[146]++;
        typedef void (*F)(void *, void *, uint32_t, const uint8_t *, const uint8_t *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[48]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 24);
        const uint8_t *a3 = NULL;
        uint32_t a4 = (uint32_t)pick_value(s);
        uint32_t a5 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4, a5);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 147: { /* ID3D11DeviceContext::CopyStructureCount */
        void *this = pick_this(s, right);
        if (this == right) s->reached[147]++;
        typedef void (*F)(void *, void *, uint32_t, void *);
        F f = (F)((*(void ***)right)[49]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        void *a2 = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 148: { /* ID3D11DeviceContext::ClearRenderTargetView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[148]++;
        typedef void (*F)(void *, void *, const uint8_t *);
        F f = (F)((*(void ***)right)[50]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 149: { /* ID3D11DeviceContext::ClearUnorderedAccessViewUint */
        void *this = pick_this(s, right);
        if (this == right) s->reached[149]++;
        typedef void (*F)(void *, void *, const uint8_t *);
        F f = (F)((*(void ***)right)[51]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 150: { /* ID3D11DeviceContext::ClearUnorderedAccessViewFloat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[150]++;
        typedef void (*F)(void *, void *, const uint8_t *);
        F f = (F)((*(void ***)right)[52]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 151: { /* ID3D11DeviceContext::ClearDepthStencilView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[151]++;
        typedef void (*F)(void *, void *, uint32_t, float, uint8_t);
        F f = (F)((*(void ***)right)[53]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        float a2 = (float)pick_float(s);
        uint8_t a3 = (uint8_t)pick_value(s);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 152: { /* ID3D11DeviceContext::GenerateMips */
        void *this = pick_this(s, right);
        if (this == right) s->reached[152]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[54]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 153: { /* ID3D11DeviceContext::SetResourceMinLOD */
        void *this = pick_this(s, right);
        if (this == right) s->reached[153]++;
        typedef void (*F)(void *, void *, float);
        F f = (F)((*(void ***)right)[55]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        float a1 = (float)pick_float(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 154: { /* ID3D11DeviceContext::GetResourceMinLOD */
        void *this = pick_this(s, right);
        if (this == right) s->reached[154]++;
        typedef float (*F)(void *, void *);
        F f = (F)((*(void ***)right)[56]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 155: { /* ID3D11DeviceContext::ResolveSubresource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[155]++;
        typedef void (*F)(void *, void *, uint32_t, void *, uint32_t, int32_t);
        F f = (F)((*(void ***)right)[57]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        void *a2 = pick_object(s, 1);
        uint32_t a3 = (uint32_t)pick_value(s);
        int32_t a4 = (int32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 156: { /* ID3D11DeviceContext::ExecuteCommandList */
        void *this = pick_this(s, right);
        if (this == right) s->reached[156]++;
        typedef void (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[58]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 157: { /* ID3D11DeviceContext::HSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[157]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[59]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 158: { /* ID3D11DeviceContext::HSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[158]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[60]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 159: { /* ID3D11DeviceContext::HSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[159]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[61]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 160: { /* ID3D11DeviceContext::HSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[160]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[62]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 161: { /* ID3D11DeviceContext::DSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[161]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[63]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 162: { /* ID3D11DeviceContext::DSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[162]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[64]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 163: { /* ID3D11DeviceContext::DSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[163]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[65]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 164: { /* ID3D11DeviceContext::DSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[164]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[66]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 165: { /* ID3D11DeviceContext::CSSetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[165]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[67]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 166: { /* ID3D11DeviceContext::CSSetUnorderedAccessViews */
        void *this = pick_this(s, right);
        if (this == right) s->reached[166]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *);
        F f = (F)((*(void ***)right)[68]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 167: { /* ID3D11DeviceContext::CSSetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[167]++;
        typedef void (*F)(void *, void *, void *const *, uint32_t);
        F f = (F)((*(void ***)right)[69]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)elements(s, n2, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n2 > MOST ? 1 : n2); i++) a1[i] = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 168: { /* ID3D11DeviceContext::CSSetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[168]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[70]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 169: { /* ID3D11DeviceContext::CSSetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[169]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *);
        F f = (F)((*(void ***)right)[71]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 170: { /* ID3D11DeviceContext::VSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[170]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[72]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 171: { /* ID3D11DeviceContext::PSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[171]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[73]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 172: { /* ID3D11DeviceContext::PSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[172]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[74]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 173: { /* ID3D11DeviceContext::PSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[173]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[75]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 174: { /* ID3D11DeviceContext::VSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[174]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[76]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 175: { /* ID3D11DeviceContext::PSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[175]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[77]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 176: { /* ID3D11DeviceContext::IAGetInputLayout */
        void *this = pick_this(s, right);
        if (this == right) s->reached[176]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[78]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 33);
        free((void *)a0);
        break;
    }
    case 177: { /* ID3D11DeviceContext::IAGetVertexBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[177]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[79]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 178: { /* ID3D11DeviceContext::IAGetIndexBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[178]++;
        typedef void (*F)(void *, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[80]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 6);
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 179: { /* ID3D11DeviceContext::GSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[179]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[81]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 180: { /* ID3D11DeviceContext::GSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[180]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[82]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 181: { /* ID3D11DeviceContext::IAGetPrimitiveTopology */
        void *this = pick_this(s, right);
        if (this == right) s->reached[181]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[83]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 182: { /* ID3D11DeviceContext::VSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[182]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[84]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 183: { /* ID3D11DeviceContext::VSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[183]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[85]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 184: { /* ID3D11DeviceContext::GetPredication */
        void *this = pick_this(s, right);
        if (this == right) s->reached[184]++;
        typedef void (*F)(void *, void **, uint8_t *);
        F f = (F)((*(void ***)right)[86]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 36);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 185: { /* ID3D11DeviceContext::GSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[185]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[87]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 186: { /* ID3D11DeviceContext::GSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[186]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[88]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 187: { /* ID3D11DeviceContext::OMGetRenderTargets */
        void *this = pick_this(s, right);
        if (this == right) s->reached[187]++;
        typedef void (*F)(void *, uint32_t, void **, void **);
        F f = (F)((*(void ***)right)[89]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)out_elements(s, n0, sizeof(void *));
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1 && n0 <= MOST) for (uint64_t i = 0; i < n0; i++) adopt(s, ok ? a1[i] : NULL, 44);
        if (a2) adopt(s, ok ? *a2 : NULL, 15);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 188: { /* ID3D11DeviceContext::OMGetRenderTargetsAndUnorderedAccessViews */
        void *this = pick_this(s, right);
        if (this == right) s->reached[188]++;
        typedef void (*F)(void *, uint32_t, void **, void **, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[90]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint64_t n4 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)out_elements(s, n0, sizeof(void *));
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint32_t a3 = (uint32_t)pick_value(s);
        uint32_t a4 = (uint32_t)n4;
        void **a5 = (void **)out_elements(s, n4, sizeof(void *));
        f(this, a0, a1, a2, a3, a4, a5);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1 && n0 <= MOST) for (uint64_t i = 0; i < n0; i++) adopt(s, ok ? a1[i] : NULL, 44);
        if (a2) adopt(s, ok ? *a2 : NULL, 15);
        if (a5 && n4 <= MOST) for (uint64_t i = 0; i < n4; i++) adopt(s, ok ? a5[i] : NULL, 59);
        free((void *)a1);
        free((void *)a2);
        free((void *)a5);
        break;
    }
    case 189: { /* ID3D11DeviceContext::OMGetBlendState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[189]++;
        typedef void (*F)(void *, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[91]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a1 = out_bytes_fixed(s, 16);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 4);
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 190: { /* ID3D11DeviceContext::OMGetDepthStencilState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[190]++;
        typedef void (*F)(void *, void **, uint8_t *);
        F f = (F)((*(void ***)right)[92]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 14);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 191: { /* ID3D11DeviceContext::SOGetTargets */
        void *this = pick_this(s, right);
        if (this == right) s->reached[191]++;
        typedef void (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[93]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)out_elements(s, n0, sizeof(void *));
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1 && n0 <= MOST) for (uint64_t i = 0; i < n0; i++) adopt(s, ok ? a1[i] : NULL, 6);
        free((void *)a1);
        break;
    }
    case 192: { /* ID3D11DeviceContext::RSGetState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[192]++;
        typedef void (*F)(void *, void **);
        F f = (F)((*(void ***)right)[94]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 39);
        free((void *)a0);
        break;
    }
    case 193: { /* ID3D11DeviceContext::RSGetViewports */
        void *this = pick_this(s, right);
        if (this == right) s->reached[193]++;
        typedef void (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[95]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint8_t *a0 = (uint8_t *)count_cell(s, n0);
        uint8_t *a1 = out_elements(s, n0, 24);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 194: { /* ID3D11DeviceContext::RSGetScissorRects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[194]++;
        typedef void (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[96]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint8_t *a0 = (uint8_t *)count_cell(s, n0);
        uint8_t *a1 = out_elements(s, n0, 16);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 195: { /* ID3D11DeviceContext::HSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[195]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[97]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 196: { /* ID3D11DeviceContext::HSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[196]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[98]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 197: { /* ID3D11DeviceContext::HSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[197]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[99]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 198: { /* ID3D11DeviceContext::HSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[198]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[100]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 199: { /* ID3D11DeviceContext::DSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[199]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[101]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 200: { /* ID3D11DeviceContext::DSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[200]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[102]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 201: { /* ID3D11DeviceContext::DSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[201]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[103]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 202: { /* ID3D11DeviceContext::DSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[202]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[104]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 203: { /* ID3D11DeviceContext::CSGetShaderResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[203]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[105]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 48);
        free((void *)a2);
        break;
    }
    case 204: { /* ID3D11DeviceContext::CSGetUnorderedAccessViews */
        void *this = pick_this(s, right);
        if (this == right) s->reached[204]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[106]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 59);
        free((void *)a2);
        break;
    }
    case 205: { /* ID3D11DeviceContext::CSGetShader */
        void *this = pick_this(s, right);
        if (this == right) s->reached[205]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[107]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 206: { /* ID3D11DeviceContext::CSGetSamplers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[206]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[108]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 47);
        free((void *)a2);
        break;
    }
    case 207: { /* ID3D11DeviceContext::CSGetConstantBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[207]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **);
        F f = (F)((*(void ***)right)[109]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        break;
    }
    case 208: { /* ID3D11DeviceContext::ClearState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[208]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[110]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 209: { /* ID3D11DeviceContext::Flush */
        void *this = pick_this(s, right);
        if (this == right) s->reached[209]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[111]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 210: { /* ID3D11DeviceContext::GetType */
        void *this = pick_this(s, right);
        if (this == right) s->reached[210]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[112]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 211: { /* ID3D11DeviceContext::GetContextFlags */
        void *this = pick_this(s, right);
        if (this == right) s->reached[211]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[113]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 212: { /* ID3D11DeviceContext::FinishCommandList */
        void *this = pick_this(s, right);
        if (this == right) s->reached[212]++;
        typedef int32_t (*F)(void *, int32_t, void **);
        F f = (F)((*(void ***)right)[114]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 9);
        free((void *)a1);
        break;
    }
    case 213: { /* ID3D11DeviceContext1::CopySubresourceRegion1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[213]++;
        typedef void (*F)(void *, void *, uint32_t, uint32_t, uint32_t, uint32_t, void *, uint32_t, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[115]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint32_t a4 = (uint32_t)pick_value(s);
        void *a5 = pick_object(s, 1);
        uint32_t a6 = (uint32_t)pick_value(s);
        uint8_t *a7 = maybe_bytes(s, 24);
        uint32_t a8 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4, a5, a6, a7, a8);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a7);
        break;
    }
    case 214: { /* ID3D11DeviceContext1::UpdateSubresource1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[214]++;
        typedef void (*F)(void *, void *, uint32_t, const uint8_t *, const uint8_t *, uint32_t, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[116]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 24);
        const uint8_t *a3 = NULL;
        uint32_t a4 = (uint32_t)pick_value(s);
        uint32_t a5 = (uint32_t)pick_value(s);
        uint32_t a6 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4, a5, a6);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 215: { /* ID3D11DeviceContext1::DiscardResource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[215]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[117]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 216: { /* ID3D11DeviceContext1::DiscardView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[216]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[118]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 217: { /* ID3D11DeviceContext1::VSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[217]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[119]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 218: { /* ID3D11DeviceContext1::HSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[218]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[120]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 219: { /* ID3D11DeviceContext1::DSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[219]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[121]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 220: { /* ID3D11DeviceContext1::GSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[220]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[122]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 221: { /* ID3D11DeviceContext1::PSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[221]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[123]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 222: { /* ID3D11DeviceContext1::CSSetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[222]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void *const *, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[124]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)elements(s, n1, sizeof(void *));
        if (a2) for (uint64_t i = 0; i < (n1 > MOST ? 1 : n1); i++) a2[i] = pick_object(s, 1);
        uint8_t *a3 = elements(s, n1, 4);
        uint8_t *a4 = elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 223: { /* ID3D11DeviceContext1::VSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[223]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[125]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 224: { /* ID3D11DeviceContext1::HSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[224]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[126]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 225: { /* ID3D11DeviceContext1::DSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[225]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[127]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 226: { /* ID3D11DeviceContext1::GSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[226]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[128]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 227: { /* ID3D11DeviceContext1::PSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[227]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[129]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 228: { /* ID3D11DeviceContext1::CSGetConstantBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[228]++;
        typedef void (*F)(void *, uint32_t, uint32_t, void **, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[130]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        void **a2 = (void **)out_elements(s, n1, sizeof(void *));
        uint8_t *a3 = out_elements(s, n1, 4);
        uint8_t *a4 = out_elements(s, n1, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a2[i] : NULL, 6);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 229: { /* ID3D11DeviceContext1::SwapDeviceContextState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[229]++;
        typedef void (*F)(void *, void *, void **);
        F f = (F)((*(void ***)right)[131]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 77);
        free((void *)a1);
        break;
    }
    case 230: { /* ID3D11DeviceContext1::ClearView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[230]++;
        typedef void (*F)(void *, void *, const uint8_t *, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[132]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint8_t *a2 = elements(s, n3, 16);
        uint32_t a3 = (uint32_t)n3;
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 231: { /* ID3D11DeviceContext1::DiscardView1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[231]++;
        typedef void (*F)(void *, void *, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[133]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = elements(s, n2, 16);
        uint32_t a2 = (uint32_t)n2;
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 232: { /* ID3D11DeviceContext2::UpdateTileMappings */
        void *this = pick_this(s, right);
        if (this == right) s->reached[232]++;
        typedef int32_t (*F)(void *, void *, uint32_t, const uint8_t *, const uint8_t *, void *, uint32_t, const uint8_t *, const uint8_t *, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[134]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint64_t n5 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 16);
        uint8_t *a3 = elements(s, n1, 16);
        void *a4 = pick_object(s, 1);
        uint32_t a5 = (uint32_t)n5;
        uint8_t *a6 = elements(s, n5, 4);
        uint8_t *a7 = elements(s, n5, 4);
        uint8_t *a8 = elements(s, n5, 4);
        uint32_t a9 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6, a7, a8, a9) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a6);
        free((void *)a7);
        free((void *)a8);
        break;
    }
    case 233: { /* ID3D11DeviceContext2::CopyTileMappings */
        void *this = pick_this(s, right);
        if (this == right) s->reached[233]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void *, const uint8_t *, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[135]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        void *a2 = pick_object(s, 1);
        uint8_t *a3 = maybe_bytes(s, 16);
        uint8_t *a4 = maybe_bytes(s, 16);
        uint32_t a5 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3, a4, a5) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 234: { /* ID3D11DeviceContext2::CopyTiles */
        void *this = pick_this(s, right);
        if (this == right) s->reached[234]++;
        typedef void (*F)(void *, void *, const uint8_t *, const uint8_t *, void *, uint64_t, uint32_t);
        F f = (F)((*(void ***)right)[136]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint8_t *a2 = maybe_bytes(s, 16);
        void *a3 = pick_object(s, 1);
        uint64_t a4 = (uint64_t)pick_value(s);
        uint32_t a5 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4, a5);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 235: { /* ID3D11DeviceContext2::UpdateTiles */
        void *this = pick_this(s, right);
        if (this == right) s->reached[235]++;
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[137]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (void *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s));
        s->counted[REFUSED]++;
        break;
    }
    case 236: { /* ID3D11DeviceContext2::ResizeTilePool */
        void *this = pick_this(s, right);
        if (this == right) s->reached[236]++;
        typedef int32_t (*F)(void *, void *, uint64_t);
        F f = (F)((*(void ***)right)[138]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint64_t a1 = (uint64_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 237: { /* ID3D11DeviceContext2::TiledResourceBarrier */
        void *this = pick_this(s, right);
        if (this == right) s->reached[237]++;
        typedef void (*F)(void *, void *, void *);
        F f = (F)((*(void ***)right)[139]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 238: { /* ID3D11DeviceContext2::IsAnnotationEnabled */
        void *this = pick_this(s, right);
        if (this == right) s->reached[238]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[140]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 239: { /* ID3D11DeviceContext2::SetMarkerInt */
        void *this = pick_this(s, right);
        if (this == right) s->reached[239]++;
        typedef void (*F)(void *, const uint8_t *, int32_t);
        F f = (F)((*(void ***)right)[141]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 2);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 240: { /* ID3D11DeviceContext2::BeginEventInt */
        void *this = pick_this(s, right);
        if (this == right) s->reached[240]++;
        typedef void (*F)(void *, const uint8_t *, int32_t);
        F f = (F)((*(void ***)right)[142]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 2);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 241: { /* ID3D11DeviceContext2::EndEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[241]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[143]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 242: { /* ID3D11DeviceContext3::Flush1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[242]++;
        typedef void (*F)(void *, int32_t, uintptr_t);
        F f = (F)((*(void ***)right)[144]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uintptr_t a1 = (uintptr_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 243: { /* ID3D11DeviceContext3::SetHardwareProtectionState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[243]++;
        typedef void (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[145]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 244: { /* ID3D11DeviceContext3::GetHardwareProtectionState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[244]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[146]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 245: { /* ID3D11DeviceContext4::Signal */
        void *this = pick_this(s, right);
        if (this == right) s->reached[245]++;
        typedef int32_t (*F)(void *, void *, uint64_t);
        F f = (F)((*(void ***)right)[147]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint64_t a1 = (uint64_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 246: { /* ID3D11DeviceContext4::Wait */
        void *this = pick_this(s, right);
        if (this == right) s->reached[246]++;
        typedef int32_t (*F)(void *, void *, uint64_t);
        F f = (F)((*(void ***)right)[148]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint64_t a1 = (uint64_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 247: { /* ID3D11Fence::CreateSharedHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[247]++;
        uint8_t *w3 = out_bytes_fixed(s, 8);
        typedef int32_t (*F)(void *, uint8_t *, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (const uint8_t *)pick_object(s, 1), w3) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w3) { for (uint64_t i = 0; i < 8; i++) if (w3[i]) { s->counted[REFUSED_WRONG]++; break; } free(w3); }
        s->counted[REFUSED]++;
        break;
    }
    case 248: { /* ID3D11Fence::GetCompletedValue */
        void *this = pick_this(s, right);
        if (this == right) s->reached[248]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 249: { /* ID3D11Fence::SetEventOnCompletion */
        void *this = pick_this(s, right);
        if (this == right) s->reached[249]++;
        typedef int32_t (*F)(void *, uint64_t, uintptr_t);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t a0 = (uint64_t)pick_value(s);
        uintptr_t a1 = (uintptr_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 250: { /* ID3D11InfoQueue::SetMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[250]++;
        typedef int32_t (*F)(void *, uint64_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t a0 = (uint64_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 251: { /* ID3D11InfoQueue::ClearStoredMessages */
        void *this = pick_this(s, right);
        if (this == right) s->reached[251]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 252: { /* ID3D11InfoQueue::GetMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[252]++;
        typedef int32_t (*F)(void *, uint64_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint64_t)pick_value(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 253: { /* ID3D11InfoQueue::GetNumMessagesAllowedByStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[253]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 254: { /* ID3D11InfoQueue::GetNumMessagesDeniedByStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[254]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 255: { /* ID3D11InfoQueue::GetNumStoredMessages */
        void *this = pick_this(s, right);
        if (this == right) s->reached[255]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 256: { /* ID3D11InfoQueue::GetNumStoredMessagesAllowedByRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[256]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 257: { /* ID3D11InfoQueue::GetNumMessagesDiscardedByMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[257]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 258: { /* ID3D11InfoQueue::GetMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[258]++;
        typedef uint64_t (*F)(void *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 259: { /* ID3D11InfoQueue::AddStorageFilterEntries */
        void *this = pick_this(s, right);
        if (this == right) s->reached[259]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 260: { /* ID3D11InfoQueue::GetStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[260]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 261: { /* ID3D11InfoQueue::ClearStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[261]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 262: { /* ID3D11InfoQueue::PushEmptyStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[262]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 263: { /* ID3D11InfoQueue::PushCopyOfStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[263]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 264: { /* ID3D11InfoQueue::PushStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[264]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 265: { /* ID3D11InfoQueue::PopStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[265]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 266: { /* ID3D11InfoQueue::GetStorageFilterStackSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[266]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 267: { /* ID3D11InfoQueue::AddRetrievalFilterEntries */
        void *this = pick_this(s, right);
        if (this == right) s->reached[267]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 268: { /* ID3D11InfoQueue::GetRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[268]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 269: { /* ID3D11InfoQueue::ClearRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[269]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 270: { /* ID3D11InfoQueue::PushEmptyRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[270]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 271: { /* ID3D11InfoQueue::PushCopyOfRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[271]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 272: { /* ID3D11InfoQueue::PushRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[272]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 273: { /* ID3D11InfoQueue::PopRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[273]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 274: { /* ID3D11InfoQueue::GetRetrievalFilterStackSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[274]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 275: { /* ID3D11InfoQueue::AddMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[275]++;
        typedef int32_t (*F)(void *, int32_t, int32_t, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint8_t *a3 = pick_text(s, 1);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 276: { /* ID3D11InfoQueue::AddApplicationMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[276]++;
        typedef int32_t (*F)(void *, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = pick_text(s, 1);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 277: { /* ID3D11InfoQueue::SetBreakOnCategory */
        void *this = pick_this(s, right);
        if (this == right) s->reached[277]++;
        typedef int32_t (*F)(void *, int32_t, int32_t);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 278: { /* ID3D11InfoQueue::SetBreakOnSeverity */
        void *this = pick_this(s, right);
        if (this == right) s->reached[278]++;
        typedef int32_t (*F)(void *, int32_t, int32_t);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 279: { /* ID3D11InfoQueue::SetBreakOnID */
        void *this = pick_this(s, right);
        if (this == right) s->reached[279]++;
        typedef int32_t (*F)(void *, int32_t, int32_t);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 280: { /* ID3D11InfoQueue::GetBreakOnCategory */
        void *this = pick_this(s, right);
        if (this == right) s->reached[280]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 281: { /* ID3D11InfoQueue::GetBreakOnSeverity */
        void *this = pick_this(s, right);
        if (this == right) s->reached[281]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 282: { /* ID3D11InfoQueue::GetBreakOnID */
        void *this = pick_this(s, right);
        if (this == right) s->reached[282]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 283: { /* ID3D11InfoQueue::SetMuteDebugOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[283]++;
        typedef void (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 284: { /* ID3D11InfoQueue::GetMuteDebugOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[284]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 285: { /* ID3D11Multithread::Enter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[285]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 286: { /* ID3D11Multithread::Leave */
        void *this = pick_this(s, right);
        if (this == right) s->reached[286]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 287: { /* ID3D11Multithread::SetMultithreadProtected */
        void *this = pick_this(s, right);
        if (this == right) s->reached[287]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 288: { /* ID3D11Multithread::GetMultithreadProtected */
        void *this = pick_this(s, right);
        if (this == right) s->reached[288]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 289: { /* ID3D11Query::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[289]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 290: { /* ID3D11Query1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[290]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 12);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 291: { /* ID3D11RasterizerState::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[291]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 40);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 292: { /* ID3D11RasterizerState1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[292]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 44);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 293: { /* ID3D11RasterizerState2::GetDesc2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[293]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 48);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 294: { /* ID3D11RefDefaultTrackingOptions::SetTrackingOptions */
        void *this = pick_this(s, right);
        if (this == right) s->reached[294]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 295: { /* ID3D11RefTrackingOptions::SetTrackingOptions */
        void *this = pick_this(s, right);
        if (this == right) s->reached[295]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 296: { /* ID3D11RenderTargetView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[296]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 20);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 297: { /* ID3D11RenderTargetView1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[297]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 298: { /* ID3D11Resource::GetType */
        void *this = pick_this(s, right);
        if (this == right) s->reached[298]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 299: { /* ID3D11Resource::SetEvictionPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[299]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 300: { /* ID3D11Resource::GetEvictionPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[300]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 301: { /* ID3D11SamplerState::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[301]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 52);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 302: { /* ID3D11ShaderResourceView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[302]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 303: { /* ID3D11ShaderResourceView1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[303]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 28);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 304: { /* ID3D11ShaderTrace::TraceReady */
        void *this = pick_this(s, right);
        if (this == right) s->reached[304]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 305: { /* ID3D11ShaderTrace::ResetTrace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[305]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 306: { /* ID3D11ShaderTrace::GetTraceStats */
        void *this = pick_this(s, right);
        if (this == right) s->reached[306]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8616);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 307: { /* ID3D11ShaderTrace::PSSelectStamp */
        void *this = pick_this(s, right);
        if (this == right) s->reached[307]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 308: { /* ID3D11ShaderTrace::GetInitialRegisterContents */
        void *this = pick_this(s, right);
        if (this == right) s->reached[308]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 12);
        uint8_t *a1 = out_bytes_fixed(s, 20);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 309: { /* ID3D11ShaderTrace::GetStep */
        void *this = pick_this(s, right);
        if (this == right) s->reached[309]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 24);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 310: { /* ID3D11ShaderTrace::GetWrittenRegister */
        void *this = pick_this(s, right);
        if (this == right) s->reached[310]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 12);
        uint8_t *a3 = out_bytes_fixed(s, 20);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 311: { /* ID3D11ShaderTrace::GetReadRegister */
        void *this = pick_this(s, right);
        if (this == right) s->reached[311]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 12);
        uint8_t *a3 = out_bytes_fixed(s, 20);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 312: { /* ID3D11ShaderTraceFactory::CreateShaderTrace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[312]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uint8_t *a1 = maybe_bytes(s, 40);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 50);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 313: { /* ID3D11SwitchToRef::SetUseRef */
        void *this = pick_this(s, right);
        if (this == right) s->reached[313]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 314: { /* ID3D11SwitchToRef::GetUseRef */
        void *this = pick_this(s, right);
        if (this == right) s->reached[314]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 315: { /* ID3D11Texture1D::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[315]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 32);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 316: { /* ID3D11Texture2D::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[316]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 44);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 317: { /* ID3D11Texture2D1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[317]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 48);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 318: { /* ID3D11Texture3D::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[318]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 36);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 319: { /* ID3D11Texture3D1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[319]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 40);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 320: { /* ID3D11TracingDevice::SetShaderTrackingOptionsByType */
        void *this = pick_this(s, right);
        if (this == right) s->reached[320]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 321: { /* ID3D11TracingDevice::SetShaderTrackingOptions */
        void *this = pick_this(s, right);
        if (this == right) s->reached[321]++;
        typedef int32_t (*F)(void *, void *, uint32_t);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 322: { /* ID3D11UnorderedAccessView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[322]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 20);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 323: { /* ID3D11UnorderedAccessView1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[323]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 324: { /* ID3D11VideoContext::GetDecoderBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[324]++;
        uint8_t *w2 = out_bytes_fixed(s, 4);
        uint8_t *w3 = out_bytes_fixed(s, sizeof(void *));
        typedef int32_t (*F)(void *, void *, int32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (void *)pick_object(s, 1), (int32_t)pick_value(s), w2, w3) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w2) { for (uint64_t i = 0; i < 4; i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        if (w3) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w3[i]) { s->counted[REFUSED_WRONG]++; break; } free(w3); }
        s->counted[REFUSED]++;
        break;
    }
    case 325: { /* ID3D11VideoContext::ReleaseDecoderBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[325]++;
        typedef int32_t (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 326: { /* ID3D11VideoContext::DecoderBeginFrame */
        void *this = pick_this(s, right);
        if (this == right) s->reached[326]++;
        typedef int32_t (*F)(void *, void *, void *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = elements(s, n2, 1);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 327: { /* ID3D11VideoContext::DecoderEndFrame */
        void *this = pick_this(s, right);
        if (this == right) s->reached[327]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 328: { /* ID3D11VideoContext::SubmitDecoderBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[328]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 329: { /* ID3D11VideoContext::DecoderExtension */
        void *this = pick_this(s, right);
        if (this == right) s->reached[329]++;
        typedef int32_t (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this, (void *)pick_object(s, 1), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 330: { /* ID3D11VideoContext::VideoProcessorSetOutputTargetRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[330]++;
        typedef void (*F)(void *, void *, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 16);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 331: { /* ID3D11VideoContext::VideoProcessorSetOutputBackgroundColor */
        void *this = pick_this(s, right);
        if (this == right) s->reached[331]++;
        typedef void (*F)(void *, void *, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 16);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 332: { /* ID3D11VideoContext::VideoProcessorSetOutputColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[332]++;
        typedef void (*F)(void *, void *, const uint8_t *);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 333: { /* ID3D11VideoContext::VideoProcessorSetOutputAlphaFillMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[333]++;
        typedef void (*F)(void *, void *, int32_t, uint32_t);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 334: { /* ID3D11VideoContext::VideoProcessorSetOutputConstriction */
        void *this = pick_this(s, right);
        if (this == right) s->reached[334]++;
        typedef void (*F)(void *, void *, int32_t, uint64_t);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, (void *)pick_object(s, 1), (int32_t)pick_value(s), (uint64_t)pick_value(s));
        s->counted[REFUSED]++;
        break;
    }
    case 335: { /* ID3D11VideoContext::VideoProcessorSetOutputStereoMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[335]++;
        typedef void (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 336: { /* ID3D11VideoContext::VideoProcessorSetOutputExtension */
        void *this = pick_this(s, right);
        if (this == right) s->reached[336]++;
        typedef int32_t (*F)(void *, void *, uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this, (void *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 337: { /* ID3D11VideoContext::VideoProcessorGetOutputTargetRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[337]++;
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 16);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 338: { /* ID3D11VideoContext::VideoProcessorGetOutputBackgroundColor */
        void *this = pick_this(s, right);
        if (this == right) s->reached[338]++;
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 16);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 339: { /* ID3D11VideoContext::VideoProcessorGetOutputColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[339]++;
        typedef void (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 340: { /* ID3D11VideoContext::VideoProcessorGetOutputAlphaFillMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[340]++;
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 341: { /* ID3D11VideoContext::VideoProcessorGetOutputConstriction */
        void *this = pick_this(s, right);
        if (this == right) s->reached[341]++;
        typedef void (*F)(void *, void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 8);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 342: { /* ID3D11VideoContext::VideoProcessorGetOutputStereoMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[342]++;
        typedef void (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 343: { /* ID3D11VideoContext::VideoProcessorGetOutputExtension */
        void *this = pick_this(s, right);
        if (this == right) s->reached[343]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = out_bytes(s, n2);
        (void)f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a3);
        break;
    }
    case 344: { /* ID3D11VideoContext::VideoProcessorSetStreamFrameFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[344]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 345: { /* ID3D11VideoContext::VideoProcessorSetStreamColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[345]++;
        typedef void (*F)(void *, void *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 346: { /* ID3D11VideoContext::VideoProcessorSetStreamOutputRate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[346]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint8_t *a4 = maybe_bytes(s, 8);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 347: { /* ID3D11VideoContext::VideoProcessorSetStreamSourceRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[347]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint8_t *a3 = maybe_bytes(s, 16);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 348: { /* ID3D11VideoContext::VideoProcessorSetStreamDestRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[348]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint8_t *a3 = maybe_bytes(s, 16);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 349: { /* ID3D11VideoContext::VideoProcessorSetStreamAlpha */
        void *this = pick_this(s, right);
        if (this == right) s->reached[349]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, float);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        float a3 = (float)pick_float(s);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 350: { /* ID3D11VideoContext::VideoProcessorSetStreamPalette */
        void *this = pick_this(s, right);
        if (this == right) s->reached[350]++;
        typedef void (*F)(void *, void *, uint32_t, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = elements(s, n2, 4);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 351: { /* ID3D11VideoContext::VideoProcessorSetStreamPixelAspectRatio */
        void *this = pick_this(s, right);
        if (this == right) s->reached[351]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, const uint8_t *, const uint8_t *);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint8_t *a3 = maybe_bytes(s, 8);
        uint8_t *a4 = maybe_bytes(s, 8);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 352: { /* ID3D11VideoContext::VideoProcessorSetStreamLumaKey */
        void *this = pick_this(s, right);
        if (this == right) s->reached[352]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, float, float);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        float a3 = (float)pick_float(s);
        float a4 = (float)pick_float(s);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 353: { /* ID3D11VideoContext::VideoProcessorSetStreamStereoFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[353]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, int32_t, int32_t, int32_t, int32_t, int32_t);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        int32_t a4 = (int32_t)pick_value(s);
        int32_t a5 = (int32_t)pick_value(s);
        int32_t a6 = (int32_t)pick_value(s);
        int32_t a7 = (int32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4, a5, a6, a7);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 354: { /* ID3D11VideoContext::VideoProcessorSetStreamAutoProcessingMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[354]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 355: { /* ID3D11VideoContext::VideoProcessorSetStreamFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[355]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, int32_t, int32_t);
        F f = (F)((*(void ***)right)[38]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        int32_t a4 = (int32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 356: { /* ID3D11VideoContext::VideoProcessorSetStreamExtension */
        void *this = pick_this(s, right);
        if (this == right) s->reached[356]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[39]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this, (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1));
        s->counted[REFUSED]++;
        break;
    }
    case 357: { /* ID3D11VideoContext::VideoProcessorGetStreamFrameFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[357]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[40]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 358: { /* ID3D11VideoContext::VideoProcessorGetStreamColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[358]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[41]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 359: { /* ID3D11VideoContext::VideoProcessorGetStreamOutputRate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[359]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[42]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 8);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 360: { /* ID3D11VideoContext::VideoProcessorGetStreamSourceRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[360]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[43]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 16);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 361: { /* ID3D11VideoContext::VideoProcessorGetStreamDestRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[361]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[44]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 16);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 362: { /* ID3D11VideoContext::VideoProcessorGetStreamAlpha */
        void *this = pick_this(s, right);
        if (this == right) s->reached[362]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[45]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 363: { /* ID3D11VideoContext::VideoProcessorGetStreamPalette */
        void *this = pick_this(s, right);
        if (this == right) s->reached[363]++;
        typedef void (*F)(void *, void *, uint32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[46]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = out_elements(s, n2, 4);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 364: { /* ID3D11VideoContext::VideoProcessorGetStreamPixelAspectRatio */
        void *this = pick_this(s, right);
        if (this == right) s->reached[364]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[47]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 8);
        uint8_t *a4 = out_bytes_fixed(s, 8);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 365: { /* ID3D11VideoContext::VideoProcessorGetStreamLumaKey */
        void *this = pick_this(s, right);
        if (this == right) s->reached[365]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[48]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 366: { /* ID3D11VideoContext::VideoProcessorGetStreamStereoFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[366]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint8_t *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[49]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        uint8_t *a5 = out_bytes_fixed(s, 4);
        uint8_t *a6 = out_bytes_fixed(s, 4);
        uint8_t *a7 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3, a4, a5, a6, a7);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        free((void *)a5);
        free((void *)a6);
        free((void *)a7);
        break;
    }
    case 367: { /* ID3D11VideoContext::VideoProcessorGetStreamAutoProcessingMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[367]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[50]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 368: { /* ID3D11VideoContext::VideoProcessorGetStreamFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[368]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[51]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 369: { /* ID3D11VideoContext::VideoProcessorGetStreamExtension */
        void *this = pick_this(s, right);
        if (this == right) s->reached[369]++;
        typedef int32_t (*F)(void *, void *, uint32_t, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[52]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 16);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = out_bytes(s, n3);
        (void)f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 370: { /* ID3D11VideoContext::VideoProcessorBlt */
        void *this = pick_this(s, right);
        if (this == right) s->reached[370]++;
        typedef int32_t (*F)(void *, void *, void *, uint32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[53]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (void *)pick_object(s, 1), (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 371: { /* ID3D11VideoContext::NegotiateCryptoSessionKeyExchange */
        void *this = pick_this(s, right);
        if (this == right) s->reached[371]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[54]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = out_bytes(s, n1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 372: { /* ID3D11VideoContext::EncryptionBlt */
        void *this = pick_this(s, right);
        if (this == right) s->reached[372]++;
        typedef void (*F)(void *, void *, void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[55]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        void *a2 = pick_object(s, 1);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = out_bytes(s, n3);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 373: { /* ID3D11VideoContext::DecryptionBlt */
        void *this = pick_this(s, right);
        if (this == right) s->reached[373]++;
        typedef void (*F)(void *, void *, void *, void *, const uint8_t *, uint32_t, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[56]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n4 = pick_count(s);
        uint64_t n6 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        void *a2 = pick_object(s, 1);
        uint8_t *a3 = maybe_bytes(s, 12);
        uint32_t a4 = (uint32_t)n4;
        uint8_t *a5 = elements(s, n4, 1);
        uint32_t a6 = (uint32_t)n6;
        uint8_t *a7 = out_bytes(s, n6);
        f(this, a0, a1, a2, a3, a4, a5, a6, a7);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        free((void *)a5);
        free((void *)a7);
        break;
    }
    case 374: { /* ID3D11VideoContext::StartSessionKeyRefresh */
        void *this = pick_this(s, right);
        if (this == right) s->reached[374]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[57]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = out_bytes(s, n1);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 375: { /* ID3D11VideoContext::FinishSessionKeyRefresh */
        void *this = pick_this(s, right);
        if (this == right) s->reached[375]++;
        typedef void (*F)(void *, void *);
        F f = (F)((*(void ***)right)[58]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 376: { /* ID3D11VideoContext::GetEncryptionBltKey */
        void *this = pick_this(s, right);
        if (this == right) s->reached[376]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[59]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = out_bytes(s, n1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 377: { /* ID3D11VideoContext::NegotiateAuthenticatedChannelKeyExchange */
        void *this = pick_this(s, right);
        if (this == right) s->reached[377]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[60]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = out_bytes(s, n1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 378: { /* ID3D11VideoContext::QueryAuthenticatedChannel */
        void *this = pick_this(s, right);
        if (this == right) s->reached[378]++;
        typedef int32_t (*F)(void *, void *, uint32_t, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[61]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = out_bytes(s, n3);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 379: { /* ID3D11VideoContext::ConfigureAuthenticatedChannel */
        void *this = pick_this(s, right);
        if (this == right) s->reached[379]++;
        typedef int32_t (*F)(void *, void *, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[62]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        uint8_t *a3 = out_bytes_fixed(s, 48);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 380: { /* ID3D11VideoContext::VideoProcessorSetStreamRotation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[380]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, int32_t);
        F f = (F)((*(void ***)right)[63]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 381: { /* ID3D11VideoContext::VideoProcessorGetStreamRotation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[381]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[64]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 382: { /* ID3D11VideoContext1::SubmitDecoderBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[382]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[65]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 383: { /* ID3D11VideoContext1::GetDataForNewHardwareKey */
        void *this = pick_this(s, right);
        if (this == right) s->reached[383]++;
        typedef int32_t (*F)(void *, void *, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[66]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        uint8_t *a3 = out_bytes_fixed(s, 8);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 384: { /* ID3D11VideoContext1::CheckCryptoSessionStatus */
        void *this = pick_this(s, right);
        if (this == right) s->reached[384]++;
        typedef int32_t (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[67]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 385: { /* ID3D11VideoContext1::DecoderEnableDownsampling */
        void *this = pick_this(s, right);
        if (this == right) s->reached[385]++;
        typedef int32_t (*F)(void *, void *, int32_t, const uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[68]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 16);
        uint32_t a3 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 386: { /* ID3D11VideoContext1::DecoderUpdateDownsampling */
        void *this = pick_this(s, right);
        if (this == right) s->reached[386]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *);
        F f = (F)((*(void ***)right)[69]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 16);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 387: { /* ID3D11VideoContext1::VideoProcessorSetOutputColorSpace1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[387]++;
        typedef void (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[70]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 388: { /* ID3D11VideoContext1::VideoProcessorSetOutputShaderUsage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[388]++;
        typedef void (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[71]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 389: { /* ID3D11VideoContext1::VideoProcessorGetOutputColorSpace1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[389]++;
        typedef void (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[72]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 390: { /* ID3D11VideoContext1::VideoProcessorGetOutputShaderUsage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[390]++;
        typedef void (*F)(void *, void *, uint8_t *);
        F f = (F)((*(void ***)right)[73]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 391: { /* ID3D11VideoContext1::VideoProcessorSetStreamColorSpace1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[391]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t);
        F f = (F)((*(void ***)right)[74]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 392: { /* ID3D11VideoContext1::VideoProcessorSetStreamMirror */
        void *this = pick_this(s, right);
        if (this == right) s->reached[392]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, int32_t, int32_t);
        F f = (F)((*(void ***)right)[75]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        int32_t a4 = (int32_t)pick_value(s);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 393: { /* ID3D11VideoContext1::VideoProcessorGetStreamColorSpace1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[393]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[76]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 394: { /* ID3D11VideoContext1::VideoProcessorGetStreamMirror */
        void *this = pick_this(s, right);
        if (this == right) s->reached[394]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[77]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 395: { /* ID3D11VideoContext1::VideoProcessorGetBehaviorHints */
        void *this = pick_this(s, right);
        if (this == right) s->reached[395]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint32_t, int32_t, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[78]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n4 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint32_t a4 = (uint32_t)n4;
        uint8_t *a5 = elements(s, n4, 16);
        uint8_t *a6 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 396: { /* ID3D11VideoContext2::VideoProcessorSetOutputHDRMetaData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[396]++;
        typedef void (*F)(void *, void *, int32_t, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[79]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = elements(s, n2, 1);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 397: { /* ID3D11VideoContext2::VideoProcessorGetOutputHDRMetaData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[397]++;
        typedef void (*F)(void *, void *, uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[80]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = out_bytes(s, n2);
        f(this, a0, a1, a2, a3);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a3);
        break;
    }
    case 398: { /* ID3D11VideoContext2::VideoProcessorSetStreamHDRMetaData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[398]++;
        typedef void (*F)(void *, void *, uint32_t, int32_t, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[81]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = elements(s, n3, 1);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 399: { /* ID3D11VideoContext2::VideoProcessorGetStreamHDRMetaData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[399]++;
        typedef void (*F)(void *, void *, uint32_t, uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[82]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n3 = pick_count(s);
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint32_t a3 = (uint32_t)n3;
        uint8_t *a4 = out_bytes(s, n3);
        f(this, a0, a1, a2, a3, a4);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 400: { /* ID3D11VideoContext3::DecoderBeginFrame1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[400]++;
        typedef int32_t (*F)(void *, void *, void *, uint32_t, const uint8_t *, uint32_t, const uint8_t *, void *const *);
        F f = (F)((*(void ***)right)[83]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        uint64_t n4 = pick_count(s);
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = elements(s, n2, 1);
        uint32_t a4 = (uint32_t)n4;
        uint8_t *a5 = elements(s, n4, 4);
        void **a6 = (void **)elements(s, n4, sizeof(void *));
        if (a6) for (uint64_t i = 0; i < (n4 > MOST ? 1 : n4); i++) a6[i] = pick_object(s, 1);
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 401: { /* ID3D11VideoContext3::SubmitDecoderBuffers2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[401]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[84]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (void *)pick_object(s, 1), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 402: { /* ID3D11VideoDecoder::GetCreationParameters */
        void *this = pick_this(s, right);
        if (this == right) s->reached[402]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 28);
        uint8_t *a1 = out_bytes_fixed(s, 100);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 403: { /* ID3D11VideoDecoder::GetDriverHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[403]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 404: { /* ID3D11VideoDecoderOutputView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[404]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 405: { /* ID3D11VideoDevice::CreateVideoDecoder */
        void *this = pick_this(s, right);
        if (this == right) s->reached[405]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        uint8_t *a1 = maybe_bytes(s, 100);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 66);
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 406: { /* ID3D11VideoDevice::CreateVideoProcessor */
        void *this = pick_this(s, right);
        if (this == right) s->reached[406]++;
        typedef int32_t (*F)(void *, void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint32_t a1 = (uint32_t)pick_value(s);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 71);
        free((void *)a2);
        break;
    }
    case 407: { /* ID3D11VideoDevice::CreateAuthenticatedChannel */
        void *this = pick_this(s, right);
        if (this == right) s->reached[407]++;
        typedef int32_t (*F)(void *, int32_t, void **);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 3);
        free((void *)a1);
        break;
    }
    case 408: { /* ID3D11VideoDevice::CreateCryptoSession */
        void *this = pick_this(s, right);
        if (this == right) s->reached[408]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint8_t *a2 = maybe_bytes(s, 16);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 12);
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 409: { /* ID3D11VideoDevice::CreateVideoDecoderOutputView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[409]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        uint8_t *a1 = maybe_bytes(s, 24);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 67);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 410: { /* ID3D11VideoDevice::CreateVideoProcessorInputView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[410]++;
        typedef int32_t (*F)(void *, void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        uint8_t *a2 = maybe_bytes(s, 16);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 74);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 411: { /* ID3D11VideoDevice::CreateVideoProcessorOutputView */
        void *this = pick_this(s, right);
        if (this == right) s->reached[411]++;
        typedef int32_t (*F)(void *, void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        void *a1 = pick_object(s, 1);
        uint8_t *a2 = maybe_bytes(s, 16);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 75);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 412: { /* ID3D11VideoDevice::CreateVideoProcessorEnumerator */
        void *this = pick_this(s, right);
        if (this == right) s->reached[412]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 40);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 72);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 413: { /* ID3D11VideoDevice::GetVideoDecoderProfileCount */
        void *this = pick_this(s, right);
        if (this == right) s->reached[413]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 414: { /* ID3D11VideoDevice::GetVideoDecoderProfile */
        void *this = pick_this(s, right);
        if (this == right) s->reached[414]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 16);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 415: { /* ID3D11VideoDevice::CheckVideoDecoderFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[415]++;
        typedef int32_t (*F)(void *, const uint8_t *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 416: { /* ID3D11VideoDevice::GetVideoDecoderConfigCount */
        void *this = pick_this(s, right);
        if (this == right) s->reached[416]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 417: { /* ID3D11VideoDevice::GetVideoDecoderConfig */
        void *this = pick_this(s, right);
        if (this == right) s->reached[417]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 100);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 418: { /* ID3D11VideoDevice::GetContentProtectionCaps */
        void *this = pick_this(s, right);
        if (this == right) s->reached[418]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint8_t *a2 = out_bytes_fixed(s, 24);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 419: { /* ID3D11VideoDevice::CheckCryptoKeyExchange */
        void *this = pick_this(s, right);
        if (this == right) s->reached[419]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint8_t *a3 = out_bytes_fixed(s, 16);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        free((void *)a3);
        break;
    }
    case 420: { /* ID3D11VideoDevice::SetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[420]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = maybe_bytes(s, 16);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 421: { /* ID3D11VideoDevice::SetPrivateDataInterface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[421]++;
        typedef int32_t (*F)(void *, const uint8_t *, void *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        void *a1 = pick_object(s, 0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 422: { /* ID3D11VideoDevice1::GetCryptoSessionPrivateDataSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[422]++;
        typedef int32_t (*F)(void *, const uint8_t *, const uint8_t *, const uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint8_t *a1 = maybe_bytes(s, 16);
        uint8_t *a2 = maybe_bytes(s, 16);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 423: { /* ID3D11VideoDevice1::GetVideoDecoderCaps */
        void *this = pick_this(s, right);
        if (this == right) s->reached[423]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, uint32_t, const uint8_t *, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        uint8_t *a3 = maybe_bytes(s, 8);
        uint32_t a4 = (uint32_t)pick_value(s);
        uint8_t *a5 = maybe_bytes(s, 16);
        uint8_t *a6 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a3);
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 424: { /* ID3D11VideoDevice1::CheckVideoDecoderDownsampling */
        void *this = pick_this(s, right);
        if (this == right) s->reached[424]++;
        typedef int32_t (*F)(void *, const uint8_t *, int32_t, const uint8_t *, const uint8_t *, const uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 100);
        uint8_t *a3 = maybe_bytes(s, 8);
        uint8_t *a4 = maybe_bytes(s, 16);
        uint8_t *a5 = out_bytes_fixed(s, 4);
        uint8_t *a6 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3, a4, a5, a6) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        free((void *)a5);
        free((void *)a6);
        break;
    }
    case 425: { /* ID3D11VideoDevice1::RecommendVideoDecoderDownsampleParameters */
        void *this = pick_this(s, right);
        if (this == right) s->reached[425]++;
        typedef int32_t (*F)(void *, const uint8_t *, int32_t, const uint8_t *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 100);
        uint8_t *a3 = maybe_bytes(s, 8);
        uint8_t *a4 = out_bytes_fixed(s, 16);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 426: { /* ID3D11VideoDevice2::CheckFeatureSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[426]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes(s, n2);
        uint32_t a2 = (uint32_t)n2;
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 427: { /* ID3D11VideoDevice2::NegotiateCryptoSessionKeyExchangeMT */
        void *this = pick_this(s, right);
        if (this == right) s->reached[427]++;
        typedef int32_t (*F)(void *, void *, int32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = out_bytes(s, n2);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 428: { /* ID3D11VideoProcessor::GetContentDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[428]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 40);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 429: { /* ID3D11VideoProcessor::GetRateConversionCaps */
        void *this = pick_this(s, right);
        if (this == right) s->reached[429]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 20);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 430: { /* ID3D11VideoProcessorEnumerator::GetVideoProcessorContentDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[430]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 40);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 431: { /* ID3D11VideoProcessorEnumerator::CheckVideoProcessorFormat */
        void *this = pick_this(s, right);
        if (this == right) s->reached[431]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 432: { /* ID3D11VideoProcessorEnumerator::GetVideoProcessorCaps */
        void *this = pick_this(s, right);
        if (this == right) s->reached[432]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 36);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 433: { /* ID3D11VideoProcessorEnumerator::GetVideoProcessorRateConversionCaps */
        void *this = pick_this(s, right);
        if (this == right) s->reached[433]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 20);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 434: { /* ID3D11VideoProcessorEnumerator::GetVideoProcessorCustomRate */
        void *this = pick_this(s, right);
        if (this == right) s->reached[434]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 20);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 435: { /* ID3D11VideoProcessorEnumerator::GetVideoProcessorFilterRange */
        void *this = pick_this(s, right);
        if (this == right) s->reached[435]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 16);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 436: { /* ID3D11VideoProcessorEnumerator1::CheckVideoProcessorFormatConversion */
        void *this = pick_this(s, right);
        if (this == right) s->reached[436]++;
        typedef int32_t (*F)(void *, int32_t, int32_t, int32_t, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint8_t *a4 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 437: { /* ID3D11VideoProcessorInputView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[437]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 438: { /* ID3D11VideoProcessorOutputView::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[438]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 439: { /* ID3D11View::GetResource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[439]++;
        uint8_t *w0 = out_bytes_fixed(s, sizeof(void *));
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this, w0);
        if (w0) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 440: { /* ID3DUserDefinedAnnotation::BeginEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[440]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 2);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 441: { /* ID3DUserDefinedAnnotation::EndEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[441]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 442: { /* ID3DUserDefinedAnnotation::SetMarker */
        void *this = pick_this(s, right);
        if (this == right) s->reached[442]++;
        typedef void (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = pick_text(s, 2);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 443: { /* ID3DUserDefinedAnnotation::GetStatus */
        void *this = pick_this(s, right);
        if (this == right) s->reached[443]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 444: { /* IDXGIAdapter::EnumOutputs */
        void *this = pick_this(s, right);
        if (this == right) s->reached[444]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 106);
        free((void *)a1);
        break;
    }
    case 445: { /* IDXGIAdapter::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[445]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 304);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 446: { /* IDXGIAdapter::CheckInterfaceSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[446]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        uint8_t *a1 = out_bytes_fixed(s, 8);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 447: { /* IDXGIAdapter1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[447]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 312);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 448: { /* IDXGIAdapter2::GetDesc2 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[448]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 320);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 449: { /* IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[449]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 450: { /* IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus */
        void *this = pick_this(s, right);
        if (this == right) s->reached[450]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 451: { /* IDXGIAdapter3::QueryVideoMemoryInfo */
        void *this = pick_this(s, right);
        if (this == right) s->reached[451]++;
        typedef int32_t (*F)(void *, uint32_t, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 32);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 452: { /* IDXGIAdapter3::SetVideoMemoryReservation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[452]++;
        typedef int32_t (*F)(void *, uint32_t, int32_t, uint64_t);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        uint64_t a2 = (uint64_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 453: { /* IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[453]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 454: { /* IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification */
        void *this = pick_this(s, right);
        if (this == right) s->reached[454]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 455: { /* IDXGIAdapter4::GetDesc3 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[455]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 320);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 456: { /* IDXGIDebug::ReportLiveObjects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[456]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 457: { /* IDXGIDebug1::EnableLeakTrackingForThread */
        void *this = pick_this(s, right);
        if (this == right) s->reached[457]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 458: { /* IDXGIDebug1::DisableLeakTrackingForThread */
        void *this = pick_this(s, right);
        if (this == right) s->reached[458]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 459: { /* IDXGIDebug1::IsLeakTrackingEnabledForThread */
        void *this = pick_this(s, right);
        if (this == right) s->reached[459]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 460: { /* IDXGIDecodeSwapChain::PresentBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[460]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 461: { /* IDXGIDecodeSwapChain::SetSourceRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[461]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 462: { /* IDXGIDecodeSwapChain::SetTargetRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[462]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 463: { /* IDXGIDecodeSwapChain::SetDestSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[463]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 464: { /* IDXGIDecodeSwapChain::GetSourceRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[464]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 465: { /* IDXGIDecodeSwapChain::GetTargetRect */
        void *this = pick_this(s, right);
        if (this == right) s->reached[465]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 466: { /* IDXGIDecodeSwapChain::GetDestSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[466]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 467: { /* IDXGIDecodeSwapChain::SetColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[467]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 468: { /* IDXGIDecodeSwapChain::GetColorSpace */
        void *this = pick_this(s, right);
        if (this == right) s->reached[468]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 469: { /* IDXGIDevice::GetAdapter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[469]++;
        typedef int32_t (*F)(void *, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 79);
        free((void *)a0);
        break;
    }
    case 470: { /* IDXGIDevice::CreateSurface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[470]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, uint32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = maybe_bytes(s, 20);
        uint32_t a1 = (uint32_t)n1;
        uint32_t a2 = (uint32_t)pick_value(s);
        uint8_t *a3 = maybe_bytes(s, 8);
        void **a4 = (void **)out_elements(s, n1, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a4 && n1 <= MOST) for (uint64_t i = 0; i < n1; i++) adopt(s, ok ? a4[i] : NULL, 116);
        free((void *)a0);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 471: { /* IDXGIDevice::QueryResourceResidency */
        void *this = pick_this(s, right);
        if (this == right) s->reached[471]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 472: { /* IDXGIDevice::SetGPUThreadPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[472]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 473: { /* IDXGIDevice::GetGPUThreadPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[473]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 474: { /* IDXGIDevice1::SetMaximumFrameLatency */
        void *this = pick_this(s, right);
        if (this == right) s->reached[474]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 475: { /* IDXGIDevice1::GetMaximumFrameLatency */
        void *this = pick_this(s, right);
        if (this == right) s->reached[475]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 476: { /* IDXGIDevice2::OfferResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[476]++;
        typedef int32_t (*F)(void *, uint32_t, void *const *, int32_t);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        int32_t a2 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 477: { /* IDXGIDevice2::ReclaimResources */
        void *this = pick_this(s, right);
        if (this == right) s->reached[477]++;
        typedef int32_t (*F)(void *, uint32_t, void *const *, uint8_t *);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        uint8_t *a2 = out_elements(s, n0, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 478: { /* IDXGIDevice2::EnqueueSetEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[478]++;
        typedef int32_t (*F)(void *, uintptr_t);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 479: { /* IDXGIDevice3::Trim */
        void *this = pick_this(s, right);
        if (this == right) s->reached[479]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 480: { /* IDXGIDevice4::OfferResources1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[480]++;
        typedef int32_t (*F)(void *, uint32_t, void *const *, int32_t, uint32_t);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        int32_t a2 = (int32_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 481: { /* IDXGIDevice4::ReclaimResources1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[481]++;
        typedef int32_t (*F)(void *, uint32_t, void *const *, uint8_t *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        void **a1 = (void **)elements(s, n0, sizeof(void *));
        if (a1) for (uint64_t i = 0; i < (n0 > MOST ? 1 : n0); i++) a1[i] = pick_object(s, 1);
        uint8_t *a2 = out_elements(s, n0, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 482: { /* IDXGIDeviceSubObject::GetDevice */
        void *this = pick_this(s, right);
        if (this == right) s->reached[482]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 483: { /* IDXGIDisplayControl::IsStereoEnabled */
        void *this = pick_this(s, right);
        if (this == right) s->reached[483]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 484: { /* IDXGIDisplayControl::SetStereoEnabled */
        void *this = pick_this(s, right);
        if (this == right) s->reached[484]++;
        typedef void (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 485: { /* IDXGIFactory::EnumAdapters */
        void *this = pick_this(s, right);
        if (this == right) s->reached[485]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 79);
        free((void *)a1);
        break;
    }
    case 486: { /* IDXGIFactory::MakeWindowAssociation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[486]++;
        typedef int32_t (*F)(void *, uintptr_t, uint32_t);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 487: { /* IDXGIFactory::GetWindowAssociation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[487]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 488: { /* IDXGIFactory::CreateSwapChain */
        void *this = pick_this(s, right);
        if (this == right) s->reached[488]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uint8_t *a1 = maybe_bytes(s, 72);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 119);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 489: { /* IDXGIFactory::CreateSoftwareAdapter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[489]++;
        typedef int32_t (*F)(void *, uintptr_t, void **);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 79);
        free((void *)a1);
        break;
    }
    case 490: { /* IDXGIFactory1::EnumAdapters1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[490]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 80);
        free((void *)a1);
        break;
    }
    case 491: { /* IDXGIFactory1::IsCurrent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[491]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 492: { /* IDXGIFactory2::IsWindowedStereoEnabled */
        void *this = pick_this(s, right);
        if (this == right) s->reached[492]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 493: { /* IDXGIFactory2::CreateSwapChainForHwnd */
        void *this = pick_this(s, right);
        if (this == right) s->reached[493]++;
        typedef int32_t (*F)(void *, void *, uintptr_t, const uint8_t *, const uint8_t *, void *, void **);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uintptr_t a1 = (uintptr_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 48);
        uint8_t *a3 = maybe_bytes(s, 20);
        void *a4 = pick_object(s, 1);
        void **a5 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4, a5) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a5) adopt(s, ok ? *a5 : NULL, 120);
        free((void *)a2);
        free((void *)a3);
        free((void *)a5);
        break;
    }
    case 494: { /* IDXGIFactory2::CreateSwapChainForCoreWindow */
        void *this = pick_this(s, right);
        if (this == right) s->reached[494]++;
        typedef int32_t (*F)(void *, void *, void *, const uint8_t *, void *, void **);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        void *a1 = pick_object(s, 0);
        uint8_t *a2 = maybe_bytes(s, 48);
        void *a3 = pick_object(s, 1);
        void **a4 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a4) adopt(s, ok ? *a4 : NULL, 120);
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 495: { /* IDXGIFactory2::GetSharedResourceAdapterLuid */
        void *this = pick_this(s, right);
        if (this == right) s->reached[495]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 8);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 496: { /* IDXGIFactory2::RegisterStereoStatusWindow */
        void *this = pick_this(s, right);
        if (this == right) s->reached[496]++;
        typedef int32_t (*F)(void *, uintptr_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 497: { /* IDXGIFactory2::RegisterStereoStatusEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[497]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 498: { /* IDXGIFactory2::UnregisterStereoStatus */
        void *this = pick_this(s, right);
        if (this == right) s->reached[498]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 499: { /* IDXGIFactory2::RegisterOcclusionStatusWindow */
        void *this = pick_this(s, right);
        if (this == right) s->reached[499]++;
        typedef int32_t (*F)(void *, uintptr_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 500: { /* IDXGIFactory2::RegisterOcclusionStatusEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[500]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 501: { /* IDXGIFactory2::UnregisterOcclusionStatus */
        void *this = pick_this(s, right);
        if (this == right) s->reached[501]++;
        typedef void (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 502: { /* IDXGIFactory2::CreateSwapChainForComposition */
        void *this = pick_this(s, right);
        if (this == right) s->reached[502]++;
        typedef int32_t (*F)(void *, void *, const uint8_t *, void *, void **);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uint8_t *a1 = maybe_bytes(s, 48);
        void *a2 = pick_object(s, 1);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, 120);
        free((void *)a1);
        free((void *)a3);
        break;
    }
    case 503: { /* IDXGIFactory3::GetCreationFlags */
        void *this = pick_this(s, right);
        if (this == right) s->reached[503]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 504: { /* IDXGIFactory4::EnumAdapterByLuid */
        void *this = pick_this(s, right);
        if (this == right) s->reached[504]++;
        uint8_t *w2 = out_bytes_fixed(s, sizeof(void *));
        typedef int32_t (*F)(void *, uint64_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint64_t)pick_value(s), (uint8_t *)pick_object(s, 1), w2) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w2) { for (uint64_t i = 0; i < sizeof(void *); i++) if (w2[i]) { s->counted[REFUSED_WRONG]++; break; } free(w2); }
        s->counted[REFUSED]++;
        break;
    }
    case 505: { /* IDXGIFactory4::EnumWarpAdapter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[505]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 506: { /* IDXGIFactory5::CheckFeatureSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[506]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes(s, n2);
        uint32_t a2 = (uint32_t)n2;
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 507: { /* IDXGIFactory6::EnumAdapterByGpuPreference */
        void *this = pick_this(s, right);
        if (this == right) s->reached[507]++;
        typedef int32_t (*F)(void *, uint32_t, int32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        int a2_interface = -1;
        uint8_t *a2 = pick_riid(s, &a2_interface);
        void **a3 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a3) adopt(s, ok ? *a3 : NULL, a2_interface);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 508: { /* IDXGIFactory7::RegisterAdaptersChangedEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[508]++;
        typedef int32_t (*F)(void *, uintptr_t, uint8_t *);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uintptr_t a0 = (uintptr_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 509: { /* IDXGIFactory7::UnregisterAdaptersChangedEvent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[509]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 510: { /* IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[510]++;
        typedef int32_t (*F)(void *, void *, uintptr_t, const uint8_t *, void *, void **);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uintptr_t a1 = (uintptr_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 48);
        void *a3 = pick_object(s, 1);
        void **a4 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a4) adopt(s, ok ? *a4 : NULL, 120);
        free((void *)a2);
        free((void *)a4);
        break;
    }
    case 511: { /* IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[511]++;
        typedef int32_t (*F)(void *, void *, uintptr_t, const uint8_t *, void *, void *, void **);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        uintptr_t a1 = (uintptr_t)pick_value(s);
        uint8_t *a2 = maybe_bytes(s, 4);
        void *a3 = pick_object(s, 1);
        void *a4 = pick_object(s, 1);
        void **a5 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4, a5) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a5) adopt(s, ok ? *a5 : NULL, 86);
        free((void *)a2);
        free((void *)a5);
        break;
    }
    case 512: { /* IDXGIInfoQueue::SetMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[512]++;
        typedef int32_t (*F)(void *, GuidAbi, uint64_t);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        uint64_t a1 = (uint64_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 513: { /* IDXGIInfoQueue::ClearStoredMessages */
        void *this = pick_this(s, right);
        if (this == right) s->reached[513]++;
        typedef void (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 514: { /* IDXGIInfoQueue::GetMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[514]++;
        typedef int32_t (*F)(void *, GuidAbi, uint64_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint64_t)pick_value(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 515: { /* IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters */
        void *this = pick_this(s, right);
        if (this == right) s->reached[515]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 516: { /* IDXGIInfoQueue::GetNumStoredMessages */
        void *this = pick_this(s, right);
        if (this == right) s->reached[516]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 517: { /* IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[517]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 518: { /* IDXGIInfoQueue::GetMessageCountLimit */
        void *this = pick_this(s, right);
        if (this == right) s->reached[518]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 519: { /* IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[519]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 520: { /* IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[520]++;
        typedef uint64_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 521: { /* IDXGIInfoQueue::AddStorageFilterEntries */
        void *this = pick_this(s, right);
        if (this == right) s->reached[521]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 522: { /* IDXGIInfoQueue::GetStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[522]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 523: { /* IDXGIInfoQueue::ClearStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[523]++;
        typedef void (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 524: { /* IDXGIInfoQueue::PushEmptyStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[524]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 525: { /* IDXGIInfoQueue::PushDenyAllStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[525]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 526: { /* IDXGIInfoQueue::PushCopyOfStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[526]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 527: { /* IDXGIInfoQueue::PushStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[527]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 528: { /* IDXGIInfoQueue::PopStorageFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[528]++;
        typedef void (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 529: { /* IDXGIInfoQueue::GetStorageFilterStackSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[529]++;
        typedef uint32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 530: { /* IDXGIInfoQueue::AddRetrievalFilterEntries */
        void *this = pick_this(s, right);
        if (this == right) s->reached[530]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 531: { /* IDXGIInfoQueue::GetRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[531]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 532: { /* IDXGIInfoQueue::ClearRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[532]++;
        typedef void (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 533: { /* IDXGIInfoQueue::PushEmptyRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[533]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 534: { /* IDXGIInfoQueue::PushDenyAllRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[534]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 535: { /* IDXGIInfoQueue::PushCopyOfRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[535]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 536: { /* IDXGIInfoQueue::PushRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[536]++;
        typedef int32_t (*F)(void *, GuidAbi, uint8_t *);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, pick_guid(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 537: { /* IDXGIInfoQueue::PopRetrievalFilter */
        void *this = pick_this(s, right);
        if (this == right) s->reached[537]++;
        typedef void (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 538: { /* IDXGIInfoQueue::GetRetrievalFilterStackSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[538]++;
        typedef uint32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 539: { /* IDXGIInfoQueue::AddMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[539]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t, int32_t, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint8_t *a4 = pick_text(s, 1);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a4);
        break;
    }
    case 540: { /* IDXGIInfoQueue::AddApplicationMessage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[540]++;
        typedef int32_t (*F)(void *, int32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = pick_text(s, 1);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 541: { /* IDXGIInfoQueue::SetBreakOnCategory */
        void *this = pick_this(s, right);
        if (this == right) s->reached[541]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t, int32_t);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 542: { /* IDXGIInfoQueue::SetBreakOnSeverity */
        void *this = pick_this(s, right);
        if (this == right) s->reached[542]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t, int32_t);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 543: { /* IDXGIInfoQueue::SetBreakOnID */
        void *this = pick_this(s, right);
        if (this == right) s->reached[543]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t, int32_t);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        int32_t a2 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 544: { /* IDXGIInfoQueue::GetBreakOnCategory */
        void *this = pick_this(s, right);
        if (this == right) s->reached[544]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        (void)f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 545: { /* IDXGIInfoQueue::GetBreakOnSeverity */
        void *this = pick_this(s, right);
        if (this == right) s->reached[545]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        (void)f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 546: { /* IDXGIInfoQueue::GetBreakOnID */
        void *this = pick_this(s, right);
        if (this == right) s->reached[546]++;
        typedef int32_t (*F)(void *, GuidAbi, int32_t);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        (void)f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 547: { /* IDXGIInfoQueue::SetMuteDebugOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[547]++;
        typedef void (*F)(void *, GuidAbi, int32_t);
        F f = (F)((*(void ***)right)[38]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        int32_t a1 = (int32_t)pick_value(s);
        f(this, a0, a1);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 548: { /* IDXGIInfoQueue::GetMuteDebugOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[548]++;
        typedef int32_t (*F)(void *, GuidAbi);
        F f = (F)((*(void ***)right)[39]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        GuidAbi a0 = pick_guid(s);
        (void)f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 549: { /* IDXGIKeyedMutex::AcquireSync */
        void *this = pick_this(s, right);
        if (this == right) s->reached[549]++;
        typedef int32_t (*F)(void *, uint64_t, uint32_t);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t a0 = (uint64_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 550: { /* IDXGIKeyedMutex::ReleaseSync */
        void *this = pick_this(s, right);
        if (this == right) s->reached[550]++;
        typedef int32_t (*F)(void *, uint64_t);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t a0 = (uint64_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 551: { /* IDXGIObject::SetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[551]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        uint8_t *a0 = maybe_bytes(s, 16);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a2);
        break;
    }
    case 552: { /* IDXGIObject::SetPrivateDataInterface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[552]++;
        typedef int32_t (*F)(void *, const uint8_t *, void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        void *a1 = pick_object(s, 0);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 553: { /* IDXGIObject::GetPrivateData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[553]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 554: { /* IDXGIObject::GetParent */
        void *this = pick_this(s, right);
        if (this == right) s->reached[554]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 555: { /* IDXGIOutput::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[555]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 96);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 556: { /* IDXGIOutput::GetDisplayModeList */
        void *this = pick_this(s, right);
        if (this == right) s->reached[556]++;
        typedef int32_t (*F)(void *, int32_t, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = (uint8_t *)count_cell(s, n2);
        uint8_t *a3 = out_elements(s, n2, 28);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 557: { /* IDXGIOutput::FindClosestMatchingMode */
        void *this = pick_this(s, right);
        if (this == right) s->reached[557]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint8_t *, void *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        uint8_t *a1 = out_bytes_fixed(s, 28);
        void *a2 = pick_object(s, 0);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 558: { /* IDXGIOutput::WaitForVBlank */
        void *this = pick_this(s, right);
        if (this == right) s->reached[558]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 559: { /* IDXGIOutput::TakeOwnership */
        void *this = pick_this(s, right);
        if (this == right) s->reached[559]++;
        typedef int32_t (*F)(void *, void *, int32_t);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        int32_t a1 = (int32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 560: { /* IDXGIOutput::ReleaseOwnership */
        void *this = pick_this(s, right);
        if (this == right) s->reached[560]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 561: { /* IDXGIOutput::GetGammaControlCapabilities */
        void *this = pick_this(s, right);
        if (this == right) s->reached[561]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4116);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 562: { /* IDXGIOutput::SetGammaControl */
        void *this = pick_this(s, right);
        if (this == right) s->reached[562]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 12324);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 563: { /* IDXGIOutput::GetGammaControl */
        void *this = pick_this(s, right);
        if (this == right) s->reached[563]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 12324);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 564: { /* IDXGIOutput::SetDisplaySurface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[564]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 565: { /* IDXGIOutput::GetDisplaySurfaceData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[565]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 566: { /* IDXGIOutput::GetFrameStatistics */
        void *this = pick_this(s, right);
        if (this == right) s->reached[566]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 32);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 567: { /* IDXGIOutput1::GetDisplayModeList1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[567]++;
        typedef int32_t (*F)(void *, int32_t, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint8_t *a2 = (uint8_t *)count_cell(s, n2);
        uint8_t *a3 = out_elements(s, n2, 32);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 568: { /* IDXGIOutput1::FindClosestMatchingMode1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[568]++;
        typedef int32_t (*F)(void *, const uint8_t *, uint8_t *, void *);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 32);
        uint8_t *a1 = out_bytes_fixed(s, 32);
        void *a2 = pick_object(s, 0);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 569: { /* IDXGIOutput1::GetDisplaySurfaceData1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[569]++;
        typedef int32_t (*F)(void *, void *);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 1);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 570: { /* IDXGIOutput1::DuplicateOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[570]++;
        typedef int32_t (*F)(void *, void *, void **);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void *a0 = pick_object(s, 0);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 113);
        free((void *)a1);
        break;
    }
    case 571: { /* IDXGIOutput2::SupportsOverlays */
        void *this = pick_this(s, right);
        if (this == right) s->reached[571]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 572: { /* IDXGIOutput3::CheckOverlaySupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[572]++;
        typedef int32_t (*F)(void *, int32_t, void *, uint8_t *);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        void *a1 = pick_object(s, 0);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 573: { /* IDXGIOutput4::CheckOverlayColorSpaceSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[573]++;
        typedef int32_t (*F)(void *, int32_t, int32_t, void *, uint8_t *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int32_t a1 = (int32_t)pick_value(s);
        void *a2 = pick_object(s, 0);
        uint8_t *a3 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a3);
        break;
    }
    case 574: { /* IDXGIOutput5::DuplicateOutput1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[574]++;
        typedef int32_t (*F)(void *, void *, uint32_t, uint32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n2 = pick_count(s);
        void *a0 = pick_object(s, 0);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)n2;
        uint8_t *a3 = elements(s, n2, 4);
        void **a4 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a4) adopt(s, ok ? *a4 : NULL, 113);
        free((void *)a3);
        free((void *)a4);
        break;
    }
    case 575: { /* IDXGIOutput6::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[575]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 152);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 576: { /* IDXGIOutput6::CheckHardwareCompositionSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[576]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 577: { /* IDXGIOutputDuplication::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[577]++;
        typedef void (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 36);
        f(this, a0);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 578: { /* IDXGIOutputDuplication::AcquireNextFrame */
        void *this = pick_this(s, right);
        if (this == right) s->reached[578]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *, void **);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 48);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, 114);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 579: { /* IDXGIOutputDuplication::GetFrameDirtyRects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[579]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = out_bytes(s, n0);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 580: { /* IDXGIOutputDuplication::GetFrameMoveRects */
        void *this = pick_this(s, right);
        if (this == right) s->reached[580]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = out_bytes(s, n0);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 581: { /* IDXGIOutputDuplication::GetFramePointerShape */
        void *this = pick_this(s, right);
        if (this == right) s->reached[581]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n0 = pick_count(s);
        uint32_t a0 = (uint32_t)n0;
        uint8_t *a1 = out_bytes(s, n0);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        uint8_t *a3 = out_bytes_fixed(s, 24);
        int ok = f(this, a0, a1, a2, a3) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        free((void *)a3);
        break;
    }
    case 582: { /* IDXGIOutputDuplication::MapDesktopSurface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[582]++;
        uint8_t *w0 = out_bytes_fixed(s, 16);
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, w0) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w0) { for (uint64_t i = 0; i < 16; i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 583: { /* IDXGIOutputDuplication::UnMapDesktopSurface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[583]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 584: { /* IDXGIOutputDuplication::ReleaseFrame */
        void *this = pick_this(s, right);
        if (this == right) s->reached[584]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 585: { /* IDXGIResource::GetSharedHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[585]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 586: { /* IDXGIResource::GetUsage */
        void *this = pick_this(s, right);
        if (this == right) s->reached[586]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 587: { /* IDXGIResource::SetEvictionPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[587]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 588: { /* IDXGIResource::GetEvictionPriority */
        void *this = pick_this(s, right);
        if (this == right) s->reached[588]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 589: { /* IDXGIResource1::CreateSubresourceSurface */
        void *this = pick_this(s, right);
        if (this == right) s->reached[589]++;
        typedef int32_t (*F)(void *, uint32_t, void **);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 118);
        free((void *)a1);
        break;
    }
    case 590: { /* IDXGIResource1::CreateSharedHandle */
        void *this = pick_this(s, right);
        if (this == right) s->reached[590]++;
        uint8_t *w3 = out_bytes_fixed(s, 8);
        typedef int32_t (*F)(void *, uint8_t *, uint32_t, const uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint8_t *)pick_object(s, 1), (uint32_t)pick_value(s), (const uint8_t *)pick_object(s, 1), w3) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w3) { for (uint64_t i = 0; i < 8; i++) if (w3[i]) { s->counted[REFUSED_WRONG]++; break; } free(w3); }
        s->counted[REFUSED]++;
        break;
    }
    case 591: { /* IDXGISurface::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[591]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 20);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 592: { /* IDXGISurface::Map */
        void *this = pick_this(s, right);
        if (this == right) s->reached[592]++;
        uint8_t *w0 = out_bytes_fixed(s, 16);
        typedef int32_t (*F)(void *, uint8_t *, uint32_t);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, w0, (uint32_t)pick_value(s)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        if (w0) { for (uint64_t i = 0; i < 16; i++) if (w0[i]) { s->counted[REFUSED_WRONG]++; break; } free(w0); }
        s->counted[REFUSED]++;
        break;
    }
    case 593: { /* IDXGISurface::Unmap */
        void *this = pick_this(s, right);
        if (this == right) s->reached[593]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f(this) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 594: { /* IDXGISurface1::GetDC */
        void *this = pick_this(s, right);
        if (this == right) s->reached[594]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 8);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 595: { /* IDXGISurface1::ReleaseDC */
        void *this = pick_this(s, right);
        if (this == right) s->reached[595]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 596: { /* IDXGISurface2::GetResource */
        void *this = pick_this(s, right);
        if (this == right) s->reached[596]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **, uint8_t *);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 597: { /* IDXGISwapChain::Present */
        void *this = pick_this(s, right);
        if (this == right) s->reached[597]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[8]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 598: { /* IDXGISwapChain::GetBuffer */
        void *this = pick_this(s, right);
        if (this == right) s->reached[598]++;
        typedef int32_t (*F)(void *, uint32_t, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[9]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 599: { /* IDXGISwapChain::SetFullscreenState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[599]++;
        typedef int32_t (*F)(void *, int32_t, void *);
        F f = (F)((*(void ***)right)[10]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        void *a1 = pick_object(s, 1);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 600: { /* IDXGISwapChain::GetFullscreenState */
        void *this = pick_this(s, right);
        if (this == right) s->reached[600]++;
        typedef int32_t (*F)(void *, uint8_t *, void **);
        F f = (F)((*(void ***)right)[11]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, 106);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 601: { /* IDXGISwapChain::GetDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[601]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[12]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 72);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 602: { /* IDXGISwapChain::ResizeBuffers */
        void *this = pick_this(s, right);
        if (this == right) s->reached[602]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint32_t, int32_t, uint32_t);
        F f = (F)((*(void ***)right)[13]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        uint32_t a2 = (uint32_t)pick_value(s);
        int32_t a3 = (int32_t)pick_value(s);
        uint32_t a4 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1, a2, a3, a4) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 603: { /* IDXGISwapChain::ResizeTarget */
        void *this = pick_this(s, right);
        if (this == right) s->reached[603]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[14]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 28);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 604: { /* IDXGISwapChain::GetContainingOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[604]++;
        typedef int32_t (*F)(void *, void **);
        F f = (F)((*(void ***)right)[15]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 106);
        free((void *)a0);
        break;
    }
    case 605: { /* IDXGISwapChain::GetFrameStatistics */
        void *this = pick_this(s, right);
        if (this == right) s->reached[605]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[16]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 32);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 606: { /* IDXGISwapChain::GetLastPresentCount */
        void *this = pick_this(s, right);
        if (this == right) s->reached[606]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[17]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 607: { /* IDXGISwapChain1::GetDesc1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[607]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[18]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 48);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 608: { /* IDXGISwapChain1::GetFullscreenDesc */
        void *this = pick_this(s, right);
        if (this == right) s->reached[608]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[19]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 20);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 609: { /* IDXGISwapChain1::GetHwnd */
        void *this = pick_this(s, right);
        if (this == right) s->reached[609]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[20]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 8);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 610: { /* IDXGISwapChain1::GetCoreWindow */
        void *this = pick_this(s, right);
        if (this == right) s->reached[610]++;
        typedef int32_t (*F)(void *, const uint8_t *, void **);
        F f = (F)((*(void ***)right)[21]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 611: { /* IDXGISwapChain1::Present1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[611]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint8_t *);
        F f = (F)((*(void ***)right)[22]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint32_t)pick_value(s), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 612: { /* IDXGISwapChain1::IsTemporaryMonoSupported */
        void *this = pick_this(s, right);
        if (this == right) s->reached[612]++;
        typedef int32_t (*F)(void *);
        F f = (F)((*(void ***)right)[23]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 613: { /* IDXGISwapChain1::GetRestrictToOutput */
        void *this = pick_this(s, right);
        if (this == right) s->reached[613]++;
        typedef int32_t (*F)(void *, void **);
        F f = (F)((*(void ***)right)[24]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        void **a0 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a0) adopt(s, ok ? *a0 : NULL, 106);
        free((void *)a0);
        break;
    }
    case 614: { /* IDXGISwapChain1::SetBackgroundColor */
        void *this = pick_this(s, right);
        if (this == right) s->reached[614]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[25]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 615: { /* IDXGISwapChain1::GetBackgroundColor */
        void *this = pick_this(s, right);
        if (this == right) s->reached[615]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[26]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 16);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 616: { /* IDXGISwapChain1::SetRotation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[616]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[27]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 617: { /* IDXGISwapChain1::GetRotation */
        void *this = pick_this(s, right);
        if (this == right) s->reached[617]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[28]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 618: { /* IDXGISwapChain2::SetSourceSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[618]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t);
        F f = (F)((*(void ***)right)[29]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint32_t a1 = (uint32_t)pick_value(s);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 619: { /* IDXGISwapChain2::GetSourceSize */
        void *this = pick_this(s, right);
        if (this == right) s->reached[619]++;
        typedef int32_t (*F)(void *, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[30]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 620: { /* IDXGISwapChain2::SetMaximumFrameLatency */
        void *this = pick_this(s, right);
        if (this == right) s->reached[620]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[31]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 621: { /* IDXGISwapChain2::GetMaximumFrameLatency */
        void *this = pick_this(s, right);
        if (this == right) s->reached[621]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[32]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 4);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 622: { /* IDXGISwapChain2::GetFrameLatencyWaitableObject */
        void *this = pick_this(s, right);
        if (this == right) s->reached[622]++;
        typedef uintptr_t (*F)(void *);
        F f = (F)((*(void ***)right)[33]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 623: { /* IDXGISwapChain2::SetMatrixTransform */
        void *this = pick_this(s, right);
        if (this == right) s->reached[623]++;
        typedef int32_t (*F)(void *, const uint8_t *);
        F f = (F)((*(void ***)right)[34]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = maybe_bytes(s, 24);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 624: { /* IDXGISwapChain2::GetMatrixTransform */
        void *this = pick_this(s, right);
        if (this == right) s->reached[624]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[35]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 24);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 625: { /* IDXGISwapChain3::GetCurrentBackBufferIndex */
        void *this = pick_this(s, right);
        if (this == right) s->reached[625]++;
        typedef uint32_t (*F)(void *);
        F f = (F)((*(void ***)right)[36]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        (void)f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 626: { /* IDXGISwapChain3::CheckColorSpaceSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[626]++;
        typedef int32_t (*F)(void *, int32_t, uint8_t *);
        F f = (F)((*(void ***)right)[37]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        break;
    }
    case 627: { /* IDXGISwapChain3::SetColorSpace1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[627]++;
        typedef int32_t (*F)(void *, int32_t);
        F f = (F)((*(void ***)right)[38]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int32_t a0 = (int32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 628: { /* IDXGISwapChain3::ResizeBuffers1 */
        void *this = pick_this(s, right);
        if (this == right) s->reached[628]++;
        typedef int32_t (*F)(void *, uint32_t, uint32_t, uint32_t, int32_t, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[39]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        if (f(this, (uint32_t)pick_value(s), (uint32_t)pick_value(s), (uint32_t)pick_value(s), (int32_t)pick_value(s), (uint32_t)pick_value(s), (uint8_t *)pick_object(s, 1), (uint8_t *)pick_object(s, 1)) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;
        s->counted[REFUSED]++;
        break;
    }
    case 629: { /* IDXGISwapChain4::SetHDRMetaData */
        void *this = pick_this(s, right);
        if (this == right) s->reached[629]++;
        typedef int32_t (*F)(void *, int32_t, uint32_t, const uint8_t *);
        F f = (F)((*(void ***)right)[40]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n1 = pick_count(s);
        int32_t a0 = (int32_t)pick_value(s);
        uint32_t a1 = (uint32_t)n1;
        uint8_t *a2 = elements(s, n1, 1);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a2);
        break;
    }
    case 630: { /* IDXGISwapChainMedia::GetFrameStatisticsMedia */
        void *this = pick_this(s, right);
        if (this == right) s->reached[630]++;
        typedef int32_t (*F)(void *, uint8_t *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint8_t *a0 = out_bytes_fixed(s, 40);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a0);
        break;
    }
    case 631: { /* IDXGISwapChainMedia::SetPresentDuration */
        void *this = pick_this(s, right);
        if (this == right) s->reached[631]++;
        typedef int32_t (*F)(void *, uint32_t);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int ok = f(this, a0) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 632: { /* IDXGISwapChainMedia::CheckPresentDurationSupport */
        void *this = pick_this(s, right);
        if (this == right) s->reached[632]++;
        typedef int32_t (*F)(void *, uint32_t, uint8_t *, uint8_t *);
        F f = (F)((*(void ***)right)[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        uint8_t *a1 = out_bytes_fixed(s, 4);
        uint8_t *a2 = out_bytes_fixed(s, 4);
        int ok = f(this, a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 633: { /* IDXGraphicsAnalysis::BeginCapture */
        void *this = pick_this(s, right);
        if (this == right) s->reached[633]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 634: { /* IDXGraphicsAnalysis::EndCapture */
        void *this = pick_this(s, right);
        if (this == right) s->reached[634]++;
        typedef void (*F)(void *);
        F f = (F)((*(void ***)right)[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        f(this);
        int ok = 1;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    default: break;
    }
}

static void call_export(State *s, const uintptr_t *exports, uint64_t which) {
    if (which < EXPORTS) s->reached[CASES + which]++;
    switch (which) {
    case 0: { /* CreateDXGIFactory */
        typedef int32_t (*F)(const uint8_t *, void **);
        F f = (F)(exports[0]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 1: { /* CreateDXGIFactory1 */
        typedef int32_t (*F)(const uint8_t *, void **);
        F f = (F)(exports[1]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int a0_interface = -1;
        uint8_t *a0 = pick_riid(s, &a0_interface);
        void **a1 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a1) adopt(s, ok ? *a1 : NULL, a0_interface);
        free((void *)a0);
        free((void *)a1);
        break;
    }
    case 2: { /* CreateDXGIFactory2 */
        typedef int32_t (*F)(uint32_t, const uint8_t *, void **);
        F f = (F)(exports[2]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 3: { /* DXGIDeclareAdapterRemovalSupport */
        typedef int32_t (*F)(void);
        F f = (F)(exports[3]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f() >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 4: { /* DXGIDisableVBlankVirtualization */
        typedef int32_t (*F)(void);
        F f = (F)(exports[4]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        int ok = f() >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        break;
    }
    case 5: { /* DXGIGetDebugInterface1 */
        typedef int32_t (*F)(uint32_t, const uint8_t *, void **);
        F f = (F)(exports[5]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint32_t a0 = (uint32_t)pick_value(s);
        int a1_interface = -1;
        uint8_t *a1 = pick_riid(s, &a1_interface);
        void **a2 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1, a2) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a2) adopt(s, ok ? *a2 : NULL, a1_interface);
        free((void *)a1);
        free((void *)a2);
        break;
    }
    case 6: { /* D3D11CreateDevice */
        typedef int32_t (*F)(void *, int32_t, uintptr_t, uint32_t, const uint8_t *, uint32_t, uint32_t, void **, uint8_t *, void **);
        F f = (F)(exports[6]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n5 = pick_count(s);
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uintptr_t a2 = (uintptr_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint8_t *a4 = elements(s, n5, 4);
        uint32_t a5 = (uint32_t)n5;
        uint32_t a6 = (uint32_t)pick_value(s);
        void **a7 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a8 = out_bytes_fixed(s, 4);
        void **a9 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a7) adopt(s, ok ? *a7 : NULL, 16);
        if (a9) adopt(s, ok ? *a9 : NULL, 23);
        free((void *)a4);
        free((void *)a7);
        free((void *)a8);
        free((void *)a9);
        break;
    }
    case 7: { /* D3D11CreateDeviceAndSwapChain */
        typedef int32_t (*F)(void *, int32_t, uintptr_t, uint32_t, const uint8_t *, uint32_t, uint32_t, const uint8_t *, void **, void **, uint8_t *, void **);
        F f = (F)(exports[7]);
        if (!f) { fprintf(stderr, "C CALLER: a null entry\n"); abort(); }
        uint64_t n5 = pick_count(s);
        void *a0 = pick_object(s, 1);
        int32_t a1 = (int32_t)pick_value(s);
        uintptr_t a2 = (uintptr_t)pick_value(s);
        uint32_t a3 = (uint32_t)pick_value(s);
        uint8_t *a4 = elements(s, n5, 4);
        uint32_t a5 = (uint32_t)n5;
        uint32_t a6 = (uint32_t)pick_value(s);
        uint8_t *a7 = maybe_bytes(s, 72);
        void **a8 = (void **)out_bytes_fixed(s, sizeof(void *));
        void **a9 = (void **)out_bytes_fixed(s, sizeof(void *));
        uint8_t *a10 = out_bytes_fixed(s, 4);
        void **a11 = (void **)out_bytes_fixed(s, sizeof(void *));
        int ok = f(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11) >= 0;
        s->counted[ok ? SUCCEEDED : FAILED]++;
        if (a8) adopt(s, ok ? *a8 : NULL, 119);
        if (a9) adopt(s, ok ? *a9 : NULL, 16);
        if (a11) adopt(s, ok ? *a11 : NULL, 23);
        free((void *)a4);
        free((void *)a7);
        free((void *)a8);
        free((void *)a9);
        free((void *)a10);
        free((void *)a11);
        break;
    }
    default: (void)exports; break;
    }
}


/* Sweeps at the start of each run: each asks every held kind for every interface it is not yet held as, calls each export
   and each case that takes an interface id once for every interface not yet held, then calls every case twice on an
   object held as its interface or one derived from it. A sweep's calls make objects the next one uses; sweeps go on
   until two in a row hold no new interface. */
#define SWEEPS 12

static int interfaces_held(State *s) {
    int held = 0;
    for (int at = 0; at < INTERFACES; at++) held += s->count[at] != 0;
    return held;
}

static void query_every_interface(State *s) {
    for (int have = 0; have < INTERFACES; have++) {
        if (!s->count[have]) continue;
        void *object = s->held[have][0];
        for (int want = 0; want < INTERFACES; want++) {
            if (s->count[want] || is_a(have, want)) continue;
            void *out = NULL;
            int32_t status = ((Querying)(*(void ***)object)[0])(object, IIDS[want], &out);
            s->counted[QUERY_INTERFACES]++;
            if (status >= 0) adopt(s, out, want);
            else if (out != NULL) s->counted[QUERY_LEFT_POINTER]++;
        }
    }
}

static void sweep(State *s, const uintptr_t *exports) {
    s->exact = 1;
    int quiet = 0;
    for (int pass = 0; pass < SWEEPS && quiet < 2; pass++) {
        int before = interfaces_held(s);
        query_every_interface(s);
        for (uint64_t which = 0; which < EXPORTS; which++) {
            if (!EXPORT_RIID[which]) continue;
            for (int want = 0; want < INTERFACES; want++) {
                if (s->count[want]) continue;
                s->forced_riid = want;
                call_export(s, exports, which);
                s->forced_riid = -1;
                s->counted[SWEPT]++;
            }
        }
        for (uint64_t which = 0; which < CASES; which++) {
            void *right = NULL;
            uint64_t start = below(s, INTERFACES);
            for (uint64_t step = 0; step < INTERFACES && !right; step++) {
                uint64_t at = (start + step) % INTERFACES;
                if (s->count[at] && is_a((int)at, CASE_INTERFACE[which])) right = s->held[at][below(s, s->count[at])];
            }
            if (!right) continue;
            if (CASE_RIID[which]) {
                for (int want = 0; want < INTERFACES; want++) {
                    if (s->count[want]) continue;
                    s->forced_riid = want;
                    call_slot(s, which, right);
                    s->forced_riid = -1;
                    s->counted[SWEPT]++;
                }
            }
            /* A case that makes objects from a description is tried until it hands one out, up to 48 times. */
            int tries = CASE_MAKES[which] ? 48 : 2;
            for (int again = 0; again < tries; again++) {
                uint64_t adopted = s->counted[ADOPTED];
                call_slot(s, which, right);
                s->counted[SWEPT]++;
                if (CASE_MAKES[which] && again >= 1 && s->counted[ADOPTED] > adopted) break;
            }
        }
        quiet = interfaces_held(s) == before ? quiet + 1 : 0;
    }
    s->exact = 0;
}

typedef struct Job { uint64_t seed, calls; const uintptr_t *exports; State *state; } Job;

static void *run_thread(void *argument) {
    Job *job = argument;
    State *s = job->state;
    s->rng = job->seed;
    s->forced_riid = -1;
    /* The exports first, until some object is held. */
    for (int tries = 0; tries < 4096; tries++) {
        call_export(s, job->exports, below(s, EXPORTS ? EXPORTS : 1));
        s->counted[EXPORTS_CALLED]++;
        if (any_held(s) && tries > 64) break;
    }
    sweep(s, job->exports);
    for (uint64_t at = 0; at < job->calls; at++) {
        s->counted[CALLS]++;
        uint64_t roll = below(s, 256);
        if (roll == 0 && EXPORTS) { call_export(s, job->exports, below(s, EXPORTS)); s->counted[EXPORTS_CALLED]++; }
        else if (roll < 6) release_one(s);
        else if (roll < 14) query_one(s);
        else if (roll < 16) add_ref_one(s);
        else {
            int interface = -1;
            void *right = held_as(s, &interface);
            if (!right) s->counted[SKIPPED]++;
            else if (CASE_COUNT[interface]) call_slot(s, CASES_OF[interface][below(s, CASE_COUNT[interface])], right);
            else s->counted[SKIPPED]++;
        }
    }
    for (int at = 0; at < INTERFACES; at++) while (s->count[at]) release(s, s->held[at][--s->count[at]]);
    return NULL;
}

int hostile_c_run(uint64_t seed, uint64_t calls, uint32_t threads, const uintptr_t *exports, size_t count, uint64_t *report, uint64_t *reached, size_t reached_count) {
    if (count != EXPORTS || reached_count != CASES + EXPORTS || threads == 0 || threads > 64) return 1;
    pthread_t ids[64];
    Job jobs[64];
    State *states = calloc(threads, sizeof(State));
    if (!states) return 2;
    for (uint32_t at = 0; at < threads; at++) {
        jobs[at] = (Job){seed * 0x9e3779b97f4a7c15ull + at + 1, calls, exports, &states[at]};
        if (pthread_create(&ids[at], NULL, run_thread, &jobs[at]) != 0) { free(states); return 3; }
    }
    for (uint32_t at = 0; at < threads; at++) pthread_join(ids[at], NULL);
    for (int counted = 0; counted < COUNTED; counted++) {
        report[counted] = 0;
        for (uint32_t at = 0; at < threads; at++) report[counted] += states[at].counted[counted];
    }
    for (size_t which = 0; which < reached_count; which++) {
        reached[which] = 0;
        for (uint32_t at = 0; at < threads; at++) reached[which] += states[at].reached[which];
    }
    free(states);
    return 0;
}
