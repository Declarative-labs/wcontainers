// Passes position and colour and writes a cull distance that culls nothing.
struct O { float4 p : SV_Position; float4 c : COLOR; float d : SV_CullDistance; };
O main(float4 p : POSITION, float4 c : COLOR) { O o; o.p = p; o.c = c; o.d = 1.0; return o; }
