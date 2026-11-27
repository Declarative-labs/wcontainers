// Direct3D's 32 input slots and 32 registers to the pixel stage: a position in slot 0, a value in
// each of slots 1 to 31, passed on in 31 values beside the position (tests/registers.rs).
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

Values main(
    float2 position : P,
    float a1 : A1,
    float a2 : A2,
    float a3 : A3,
    float a4 : A4,
    float a5 : A5,
    float a6 : A6,
    float a7 : A7,
    float a8 : A8,
    float a9 : A9,
    float a10 : A10,
    float a11 : A11,
    float a12 : A12,
    float a13 : A13,
    float a14 : A14,
    float a15 : A15,
    float a16 : A16,
    float a17 : A17,
    float a18 : A18,
    float a19 : A19,
    float a20 : A20,
    float a21 : A21,
    float a22 : A22,
    float a23 : A23,
    float a24 : A24,
    float a25 : A25,
    float a26 : A26,
    float a27 : A27,
    float a28 : A28,
    float a29 : A29,
    float a30 : A30,
    float a31 : A31)
{
    Values o;
    o.position = float4(position, 0, 1);
    o.v0 = float4(a1, a2, 0.0, a8 * 2.0);
    o.v1 = float4(a2, a3, 1.0, a9 * 2.0);
    o.v2 = float4(a3, a4, 2.0, a10 * 2.0);
    o.v3 = float4(a4, a5, 3.0, a11 * 2.0);
    o.v4 = float4(a5, a6, 4.0, a12 * 2.0);
    o.v5 = float4(a6, a7, 5.0, a13 * 2.0);
    o.v6 = float4(a7, a8, 6.0, a14 * 2.0);
    o.v7 = float4(a8, a9, 7.0, a15 * 2.0);
    o.v8 = float4(a9, a10, 8.0, a16 * 2.0);
    o.v9 = float4(a10, a11, 9.0, a17 * 2.0);
    o.v10 = float4(a11, a12, 10.0, a18 * 2.0);
    o.v11 = float4(a12, a13, 11.0, a19 * 2.0);
    o.v12 = float4(a13, a14, 12.0, a20 * 2.0);
    o.v13 = float4(a14, a15, 13.0, a21 * 2.0);
    o.v14 = float4(a15, a16, 14.0, a22 * 2.0);
    o.v15 = float4(a16, a17, 15.0, a23 * 2.0);
    o.v16 = float4(a17, a18, 16.0, a24 * 2.0);
    o.v17 = float4(a18, a19, 17.0, a25 * 2.0);
    o.v18 = float4(a19, a20, 18.0, a26 * 2.0);
    o.v19 = float4(a20, a21, 19.0, a27 * 2.0);
    o.v20 = float4(a21, a22, 20.0, a28 * 2.0);
    o.v21 = float4(a22, a23, 21.0, a29 * 2.0);
    o.v22 = float4(a23, a24, 22.0, a30 * 2.0);
    o.v23 = float4(a24, a25, 23.0, a31 * 2.0);
    o.v24 = float4(a25, a26, 24.0, a1 * 2.0);
    o.v25 = float4(a26, a27, 25.0, a2 * 2.0);
    o.v26 = float4(a27, a28, 26.0, a3 * 2.0);
    o.v27 = float4(a28, a29, 27.0, a4 * 2.0);
    o.v28 = float4(a29, a30, 28.0, a5 * 2.0);
    o.v29 = float4(a30, a31, 29.0, a6 * 2.0);
    o.v30 = float4(a31, a1, 30.0, a7 * 2.0);
    return o;
}
