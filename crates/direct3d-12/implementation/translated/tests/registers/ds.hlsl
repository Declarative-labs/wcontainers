// Direct3D's 32 output registers of a domain shader: the position and 31 values (tests/registers.rs).
struct V
{
    float4 p : P;
    float k : K;
};

struct C
{
    float e[3] : SV_TessFactor;
    float i : SV_InsideTessFactor;
};

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

[domain("tri")]
G main(C c, float3 b : SV_DomainLocation, const OutputPatch<V, 3> ip)
{
    G g;
    g.p = ip[0].p * b.x + ip[1].p * b.y + ip[2].p * b.z;
    float k = ip[0].k * b.x + ip[1].k * b.y + ip[2].k * b.z;
    float n = b.y + 2 * b.z;
    g.v0 = float4(k * 1.5 + 0, 0 * 0.25 - k, n, 0);
    g.v1 = float4(k * 1.5 + 1, 1 * 0.25 - k, n, 1);
    g.v2 = float4(k * 1.5 + 2, 2 * 0.25 - k, n, 2);
    g.v3 = float4(k * 1.5 + 3, 3 * 0.25 - k, n, 3);
    g.v4 = float4(k * 1.5 + 4, 4 * 0.25 - k, n, 4);
    g.v5 = float4(k * 1.5 + 5, 5 * 0.25 - k, n, 5);
    g.v6 = float4(k * 1.5 + 6, 6 * 0.25 - k, n, 6);
    g.v7 = float4(k * 1.5 + 7, 7 * 0.25 - k, n, 7);
    g.v8 = float4(k * 1.5 + 8, 8 * 0.25 - k, n, 8);
    g.v9 = float4(k * 1.5 + 9, 9 * 0.25 - k, n, 9);
    g.v10 = float4(k * 1.5 + 10, 10 * 0.25 - k, n, 10);
    g.v11 = float4(k * 1.5 + 11, 11 * 0.25 - k, n, 11);
    g.v12 = float4(k * 1.5 + 12, 12 * 0.25 - k, n, 12);
    g.v13 = float4(k * 1.5 + 13, 13 * 0.25 - k, n, 13);
    g.v14 = float4(k * 1.5 + 14, 14 * 0.25 - k, n, 14);
    g.v15 = float4(k * 1.5 + 15, 15 * 0.25 - k, n, 15);
    g.v16 = float4(k * 1.5 + 16, 16 * 0.25 - k, n, 16);
    g.v17 = float4(k * 1.5 + 17, 17 * 0.25 - k, n, 17);
    g.v18 = float4(k * 1.5 + 18, 18 * 0.25 - k, n, 18);
    g.v19 = float4(k * 1.5 + 19, 19 * 0.25 - k, n, 19);
    g.v20 = float4(k * 1.5 + 20, 20 * 0.25 - k, n, 20);
    g.v21 = float4(k * 1.5 + 21, 21 * 0.25 - k, n, 21);
    g.v22 = float4(k * 1.5 + 22, 22 * 0.25 - k, n, 22);
    g.v23 = float4(k * 1.5 + 23, 23 * 0.25 - k, n, 23);
    g.v24 = float4(k * 1.5 + 24, 24 * 0.25 - k, n, 24);
    g.v25 = float4(k * 1.5 + 25, 25 * 0.25 - k, n, 25);
    g.v26 = float4(k * 1.5 + 26, 26 * 0.25 - k, n, 26);
    g.v27 = float4(k * 1.5 + 27, 27 * 0.25 - k, n, 27);
    g.v28 = float4(k * 1.5 + 28, 28 * 0.25 - k, n, 28);
    g.v29 = float4(k * 1.5 + 29, 29 * 0.25 - k, n, 29);
    g.v30 = float4(k * 1.5 + 30, 30 * 0.25 - k, n, 30);
    return g;
}
