// One triangle with a colour at each corner.
// fxc: vs_4_0 vs, ps_4_0 ps
// dxc: vs_6_0 vs, ps_6_0 ps
struct VsIn { float2 pos : POSITION; float3 color : COLOR; };
struct VsOut { float4 pos : SV_Position; float3 color : COLOR; };
VsOut vs(VsIn i) { VsOut o; o.pos = float4(i.pos, 0.0, 1.0); o.color = i.color; return o; }
float4 ps(VsOut i) : SV_Target { return float4(i.color, 1.0); }
