// Sums the 31 values a geometry or domain shader passes beside the position, each weighted
// (tests/registers.rs).
struct G
{
    float4 p : SV_Position;
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
    float4 v30 : V30;
};

float4 main(G g) : SV_Target
{
    float4 r = 0;
    r += g.v0 * 1.0;
    r += g.v1 * 2.0;
    r += g.v2 * 3.0;
    r += g.v3 * 4.0;
    r += g.v4 * 5.0;
    r += g.v5 * 1.0;
    r += g.v6 * 2.0;
    r += g.v7 * 3.0;
    r += g.v8 * 4.0;
    r += g.v9 * 5.0;
    r += g.v10 * 1.0;
    r += g.v11 * 2.0;
    r += g.v12 * 3.0;
    r += g.v13 * 4.0;
    r += g.v14 * 5.0;
    r += g.v15 * 1.0;
    r += g.v16 * 2.0;
    r += g.v17 * 3.0;
    r += g.v18 * 4.0;
    r += g.v19 * 5.0;
    r += g.v20 * 1.0;
    r += g.v21 * 2.0;
    r += g.v22 * 3.0;
    r += g.v23 * 4.0;
    r += g.v24 * 5.0;
    r += g.v25 * 1.0;
    r += g.v26 * 2.0;
    r += g.v27 * 3.0;
    r += g.v28 * 4.0;
    r += g.v29 * 5.0;
    r += g.v30 * 1.0;
    return r;
}
