// A hull shader handing the domain shader no control points, before tessellation.hlsl's ds_tri_none;
// fxc's build alone, as dxc refuses a count of none.

#include "tessellation.hlsl"

[domain("tri")] [partitioning("fractional_odd")] [outputtopology("triangle_cw")]
[outputcontrolpoints(0)] [patchconstantfunc("tri_factors")]
ControlPoint hs_tri_none(InputPatch<ControlPoint, 1> p) { return p[0]; }
