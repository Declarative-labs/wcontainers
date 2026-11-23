// Sums the 31 values the vertex shader passes, each weighted (tests/registers.rs).
struct Values
{
    float4 position : SV_Position;
    nointerpolation float4 v0 : V0;
    nointerpolation float4 v1 : V1;
    nointerpolation float4 v2 : V2;
    nointerpolation float4 v3 : V3;
    nointerpolation float4 v4 : V4;
    nointerpolation float4 v5 : V5;
    nointerpolation float4 v6 : V6;
    nointerpolation float4 v7 : V7;
    nointerpolation float4 v8 : V8;
    nointerpolation float4 v9 : V9;
    nointerpolation float4 v10 : V10;
    nointerpolation float4 v11 : V11;
    nointerpolation float4 v12 : V12;
    nointerpolation float4 v13 : V13;
    nointerpolation float4 v14 : V14;
    nointerpolation float4 v15 : V15;
    nointerpolation float4 v16 : V16;
    nointerpolation float4 v17 : V17;
    nointerpolation float4 v18 : V18;
    nointerpolation float4 v19 : V19;
    nointerpolation float4 v20 : V20;
    nointerpolation float4 v21 : V21;
    nointerpolation float4 v22 : V22;
    nointerpolation float4 v23 : V23;
    nointerpolation float4 v24 : V24;
    nointerpolation float4 v25 : V25;
    nointerpolation float4 v26 : V26;
    nointerpolation float4 v27 : V27;
    nointerpolation float4 v28 : V28;
    nointerpolation float4 v29 : V29;
    nointerpolation float4 v30 : V30;
};

float4 main(Values o) : SV_Target
{
    float4 r = 0;
    r += o.v0 * 1.0;
    r += o.v1 * 2.0;
    r += o.v2 * 3.0;
    r += o.v3 * 4.0;
    r += o.v4 * 5.0;
    r += o.v5 * 1.0;
    r += o.v6 * 2.0;
    r += o.v7 * 3.0;
    r += o.v8 * 4.0;
    r += o.v9 * 5.0;
    r += o.v10 * 1.0;
    r += o.v11 * 2.0;
    r += o.v12 * 3.0;
    r += o.v13 * 4.0;
    r += o.v14 * 5.0;
    r += o.v15 * 1.0;
    r += o.v16 * 2.0;
    r += o.v17 * 3.0;
    r += o.v18 * 4.0;
    r += o.v19 * 5.0;
    r += o.v20 * 1.0;
    r += o.v21 * 2.0;
    r += o.v22 * 3.0;
    r += o.v23 * 4.0;
    r += o.v24 * 5.0;
    r += o.v25 * 1.0;
    r += o.v26 * 2.0;
    r += o.v27 * 3.0;
    r += o.v28 * 4.0;
    r += o.v29 * 5.0;
    r += o.v30 * 1.0;
    return r;
}
