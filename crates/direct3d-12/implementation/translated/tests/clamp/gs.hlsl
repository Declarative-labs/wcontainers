// Passes each triangle on.
struct V { float4 p : SV_Position; float4 c : COLOR; };
[maxvertexcount(3)] void main(triangle V i[3], inout TriangleStream<V> s) {
    for (uint k = 0; k < 3; k++) { s.Append(i[k]); } }
