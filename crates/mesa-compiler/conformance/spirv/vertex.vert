#version 450
layout(location = 0) in vec4 at;
layout(location = 1) in vec2 coordinate;
layout(location = 0) out vec2 passed;
layout(std140, binding = 0) uniform Block { mat4 transform; float size; };
void main() {
    gl_Position = transform * at;
    gl_PointSize = size;
    passed = coordinate;
}
