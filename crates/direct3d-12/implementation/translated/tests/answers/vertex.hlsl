// Fetches one vertex input of the component kind KIND and passes it on; the three vertices cover
// the target.
struct O { float4 p : SV_Position; nointerpolation KIND v : VALUE; };
O main(KIND v : VALUE, uint id : SV_VertexID)
{
    O o;
    o.p = float4(id == 1 ? 3 : -1, id == 2 ? 3 : -1, 0.5, 1);
    o.v = v;
    return o;
}
