#version 450
// A geometry program of three invocations writing two vertex streams, for the flat form's invocations and streams:
// glslangValidator -V --target-env vulkan1.1 streams.geom -o streams.spv (the bridge's GLSL compiler allows one
// stream, so only SPIR-V brings a second).
layout(points, invocations = 3) in;
layout(points, max_vertices = 2) out;
layout(location = 0) in vec4 passed[];
layout(location = 0, stream = 0) out vec4 shaded;
layout(location = 1, stream = 1) out vec4 streamed;
void main() {
    gl_Position = gl_in[0].gl_Position;
    shaded = passed[0];
    EmitStreamVertex(0);
    streamed = passed[0];
    EmitStreamVertex(1);
}
