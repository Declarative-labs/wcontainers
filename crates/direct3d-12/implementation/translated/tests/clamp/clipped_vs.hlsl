// Passes position and colour and writes a clip distance cutting off the left.
struct O { float4 p : SV_Position; float4 c : COLOR; float d : SV_ClipDistance; };
O main(float4 p : POSITION, float4 c : COLOR) {
    O o; o.p = p; o.c = c; o.d = p.x + 0.25 * p.w; return o; }
