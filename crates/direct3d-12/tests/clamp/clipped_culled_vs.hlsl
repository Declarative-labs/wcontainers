// Passes position and colour and writes both: a clip distance cutting off the left, a cull distance culling nothing.
struct O { float4 p : SV_Position; float4 c : COLOR;
    float d : SV_ClipDistance; float e : SV_CullDistance; };
O main(float4 p : POSITION, float4 c : COLOR) {
    O o; o.p = p; o.c = c; o.d = p.x + 0.25 * p.w; o.e = 1.0; return o; }
