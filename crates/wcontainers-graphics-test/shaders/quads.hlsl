// Textured, coloured quads.
// fxc: vs_5_0 vs, ps_5_0 ps
Texture2D tex : register(t0);
SamplerState smp : register(s0);
struct VsIn { float3 pos : POSITION; float2 uv : TEXCOORD; float4 color : COLOR; };
struct VsOut { float4 pos : SV_Position; float2 uv : TEXCOORD; float4 color : COLOR; };
VsOut vs(VsIn i) { VsOut o; o.pos = float4(i.pos, 1.0); o.uv = i.uv; o.color = i.color; return o; }
float4 ps(VsOut i) : SV_Target { return tex.Sample(smp, i.uv) * i.color; }
