// One small triangle drawn many times, placed and coloured from a constant buffer by instance.
// fxc: vs_5_0 vs, ps_5_0 ps
cbuffer Scene : register(b0) {
    float4 placement[16]; // offset x, y; cosine and sine of the turn
    float4 colors[16];
    float4 scale;
};
struct VsIn { float2 pos : POSITION; uint id : SV_InstanceID; };
struct VsOut { float4 pos : SV_Position; float4 color : COLOR; };
VsOut vs(VsIn i) {
    float4 p = placement[i.id];
    float2 turned = float2(i.pos.x * p.z - i.pos.y * p.w, i.pos.x * p.w + i.pos.y * p.z);
    VsOut o;
    o.pos = float4(turned * scale.xy + p.xy, 0.0, 1.0);
    o.color = colors[i.id];
    return o;
}
float4 ps(VsOut i) : SV_Target { return i.color; }
