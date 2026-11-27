// Direct3D's 32 registers to the pixel stage with clip distances the pixel shader reads: the
// position, one register of them and 30 values (tests/registers.rs).
struct O
{
    float4 p : SV_Position;
    float4 d : SV_ClipDistance0;
    float4 v0 : V0;
    float4 v1 : V1;
    float4 v2 : V2;
    float4 v3 : V3;
    float4 v4 : V4;
    float4 v5 : V5;
    float4 v6 : V6;
    float4 v7 : V7;
    float4 v8 : V8;
    float4 v9 : V9;
    float4 v10 : V10;
    float4 v11 : V11;
    float4 v12 : V12;
    float4 v13 : V13;
    float4 v14 : V14;
    float4 v15 : V15;
    float4 v16 : V16;
    float4 v17 : V17;
    float4 v18 : V18;
    float4 v19 : V19;
    float4 v20 : V20;
    float4 v21 : V21;
    float4 v22 : V22;
    float4 v23 : V23;
    float4 v24 : V24;
    float4 v25 : V25;
    float4 v26 : V26;
    float4 v27 : V27;
    float4 v28 : V28;
    float4 v29 : V29;
};

O main(uint id : SV_VertexID)
{
    O o;
    float f = id;
    float2 p = float2((id << 1) & 2, id & 2) * float2(2, -2) + float2(-1, 1);
    o.p = float4(p, 0, 1);
    o.d = float4(1 + f, 2, 3, 4 + f);
    o.v0 = float4(f + 0, 0 * 0.5 - f, f * 0, 1);
    o.v1 = float4(f + 1, 1 * 0.5 - f, f * 1, 1);
    o.v2 = float4(f + 2, 2 * 0.5 - f, f * 2, 1);
    o.v3 = float4(f + 3, 3 * 0.5 - f, f * 3, 1);
    o.v4 = float4(f + 4, 4 * 0.5 - f, f * 4, 1);
    o.v5 = float4(f + 5, 5 * 0.5 - f, f * 5, 1);
    o.v6 = float4(f + 6, 6 * 0.5 - f, f * 6, 1);
    o.v7 = float4(f + 7, 7 * 0.5 - f, f * 7, 1);
    o.v8 = float4(f + 8, 8 * 0.5 - f, f * 8, 1);
    o.v9 = float4(f + 9, 9 * 0.5 - f, f * 9, 1);
    o.v10 = float4(f + 10, 10 * 0.5 - f, f * 10, 1);
    o.v11 = float4(f + 11, 11 * 0.5 - f, f * 11, 1);
    o.v12 = float4(f + 12, 12 * 0.5 - f, f * 12, 1);
    o.v13 = float4(f + 13, 13 * 0.5 - f, f * 13, 1);
    o.v14 = float4(f + 14, 14 * 0.5 - f, f * 14, 1);
    o.v15 = float4(f + 15, 15 * 0.5 - f, f * 15, 1);
    o.v16 = float4(f + 16, 16 * 0.5 - f, f * 16, 1);
    o.v17 = float4(f + 17, 17 * 0.5 - f, f * 17, 1);
    o.v18 = float4(f + 18, 18 * 0.5 - f, f * 18, 1);
    o.v19 = float4(f + 19, 19 * 0.5 - f, f * 19, 1);
    o.v20 = float4(f + 20, 20 * 0.5 - f, f * 20, 1);
    o.v21 = float4(f + 21, 21 * 0.5 - f, f * 21, 1);
    o.v22 = float4(f + 22, 22 * 0.5 - f, f * 22, 1);
    o.v23 = float4(f + 23, 23 * 0.5 - f, f * 23, 1);
    o.v24 = float4(f + 24, 24 * 0.5 - f, f * 24, 1);
    o.v25 = float4(f + 25, 25 * 0.5 - f, f * 25, 1);
    o.v26 = float4(f + 26, 26 * 0.5 - f, f * 26, 1);
    o.v27 = float4(f + 27, 27 * 0.5 - f, f * 27, 1);
    o.v28 = float4(f + 28, 28 * 0.5 - f, f * 28, 1);
    o.v29 = float4(f + 29, 29 * 0.5 - f, f * 29, 1);
    return o;
}
