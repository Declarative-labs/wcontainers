// Passes position and colour.
struct O { float4 p : SV_Position; float4 c : COLOR; };
O main(float4 p : POSITION, float4 c : COLOR) { O o; o.p = p; o.c = c; return o; }
