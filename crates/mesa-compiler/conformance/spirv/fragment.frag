#version 450
layout(location = 0) in vec2 passed;
layout(location = 0) out vec4 colour;
layout(binding = 0) uniform sampler2D picture;
layout(binding = 1) uniform isampler2D counts;
layout(binding = 2) uniform sampler1D line;
layout(binding = 3) uniform samplerCube sky;
void main() {
    vec2 moved = interpolateAtOffset(passed, vec2(0.25));
    ivec4 count = texture(counts, passed);
    colour = texture(picture, moved) + vec4(count) + texture(line, passed.x) + texture(sky, vec3(passed, 1.0));
    colour.x += gl_SamplePosition.x;
    gl_SampleMask[0] = 1;
}
