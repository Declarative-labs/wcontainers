// Raw and structured shader resource views read by index and by their sizes, written out through a
// storage view, so views starting at any element can be compared.

StructuredBuffer<uint> structured : register(t0);
ByteAddressBuffer raw : register(t1);
RWStructuredBuffer<uint> written : register(u0);

[numthreads(64, 1, 1)]
void cs_readonly(uint i : SV_DispatchThreadID) {
    uint count, stride, bytes;
    structured.GetDimensions(count, stride);
    raw.GetDimensions(bytes);
    written[2 * i] = structured[i] ^ (count << 16);
    written[2 * i + 1] = raw.Load(4 * i) ^ bytes;
}
