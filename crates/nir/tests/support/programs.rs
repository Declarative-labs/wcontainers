//! GLSL programs both route.rs and arguments.rs draw.

/// A vertex program drawing points of its own size, before a geometry or tessellation stage.
pub const POINT_VERTEX: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 passed;
void main() {
    gl_Position = at;
    gl_PointSize = at.w;
    passed = at;
}
";

/// A geometry program taking a point and making two, each with its point size and a layer it names.
pub const POINT_GEOMETRY: &str = "#version 450
layout(points) in;
layout(points, max_vertices = 2) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 shaded;
void main() {
    for (int copy = 0; copy < 2; copy++) {
        gl_Position = gl_in[0].gl_Position + vec4(float(copy));
        gl_PointSize = gl_in[0].gl_PointSize;
        gl_Layer = copy;
        shaded = passed[0];
        EmitVertex();
    }
}
";

/// A tessellation control program of three control points, passing each on.
pub const CONTROL: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 handed[];
void main() {
    handed[gl_InvocationID] = passed[gl_InvocationID];
    gl_out[gl_InvocationID].gl_Position = gl_in[gl_InvocationID].gl_Position;
    gl_TessLevelOuter[0] = 2.0;
    gl_TessLevelOuter[1] = 2.0;
    gl_TessLevelOuter[2] = 2.0;
    gl_TessLevelInner[0] = 2.0;
}
";

/// A fragment program after the geometry or evaluation program.
pub const SHADED_FRAGMENT: &str = "#version 450
layout(location = 0) in vec4 shaded;
layout(location = 0) out vec4 colour;
void main() { colour = shaded; }
";
