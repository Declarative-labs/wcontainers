#version 450
// A fragment program writing a sample mask, for route.rs. Written to masked.spv by glslang 16.4.0
// (`glslangValidator -V masked.frag -o masked.spv`); naga writes the sample mask as a scalar, which Vulkan's
// SPIR-V refuses (it is an array of 32-bit integers), so the bridge's validator would refuse naga's form.
layout(location = 0) in vec4 colour;
layout(location = 1) flat in uint mask;
layout(location = 0) out vec4 result;
void main() {
    result = colour;
    gl_SampleMask[0] = int(mask);
}
