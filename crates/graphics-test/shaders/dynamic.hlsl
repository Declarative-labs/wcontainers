// Coloured vertices tinted by a constant buffer rewritten for every draw.
// fxc: ps_4_0 ps_tint
cbuffer Tint : register(b0) { float4 tint; };
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
float4 ps_tint(ColorOut i) : SV_Target { return i.color * tint; }
