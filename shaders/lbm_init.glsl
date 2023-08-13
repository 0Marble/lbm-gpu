#version 460 core
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
layout(rgba32f, binding = 0) uniform writeonly image2D Screen;
layout(rgba32f, binding = 1) uniform writeonly image2DArray Fin;
layout(rgba32f, binding = 2) uniform writeonly image2DArray Fout;
layout(rg32f, binding = 3) uniform writeonly image2D Vel;
layout(rg32f, binding = 4) uniform writeonly image2D Initial_vel;
layout(r8ui, binding = 5) uniform writeonly uimage2D Obstacle;

float T_WEIGHTS[9] = {
    1.0/36.0, 1.0/9.0, 1.0/36.0,
    1.0/9.0, 4.0/9.0, 1.0/9.0,
    1.0/36.0, 1.0/9.0, 1.0/36.0,
};
vec2 DIRS[9] = { 
    vec2(1.0, 1.0), vec2(1.0, 0.0), vec2(1.0, -1.0), 
    vec2(0.0, 1.0), vec2(0.0, 0.0), vec2(0.0, -1.0), 
    vec2(-1.0, 1.0), vec2(-1.0, 0.0), vec2(-1.0, -1.0), 
};

vec4 from_hsv(vec4 hsv) {
    float h = hsv.r;
    float s = hsv.g;
    float v = hsv.b;
    float a = hsv.a;

    float c = v * s;
    float x = c * (1.0 - abs(mod(h / 60.0,2.0) - 1.0));
    float m = v - c;

    float r = 0.0;
    float g = 0.0;
    float b = 0.0;

    if (h >= 0 && h < 60.0) {
        r = c; g = x; b = 0.0;
    } else if (h >= 60.0 && h < 120.0) {
        r = x; g = c; b = 0.0;
    } else if (h >= 120.0 && h < 180.0) {
        r = 0.0; g = c; b = x;
    } else if (h >= 180.0 && h < 240.0) {
        r = 0.0; g = x; b = c;
    } else if (h >= 240.0 && h < 300.0) {
        r = x; g = 0.0; b = c;
    } else if (h >= 300.0 && h < 360.0) {
        r = c; g = 0.0; b = x;
    }

    return vec4(r + m, g + m, b + m, a);
}

void main() {
    int clm = int(gl_GlobalInvocationID.x);
    int row = int(gl_GlobalInvocationID.y);
    ivec3 size = imageSize(Fin);
    
    float reynolds_number = REYNOLDS_NUMBER;
    float ulb = 0.04;
    float r = float(size.y / 9);
    float omega = 1.0 / (3.0 * ulb * r / reynolds_number + 0.5);

    imageStore(Fout, ivec3(clm, row, 0), vec4(0.0,0.0,0.0,0.0));
    imageStore(Fout, ivec3(clm, row, 1), vec4(0.0,0.0,0.0,0.0));
    imageStore(Fout, ivec3(clm, row, 2), vec4(0.0,0.0,0.0,0.0));

    vec2 vel = vec2(ulb * (1.0 + 0.0001 * sin(float(clm) * 2.0 * 3.141 / float(size.y - 1))), 0.00001);
    imageStore(Initial_vel, ivec2(clm, row), vec4(vel.x, vel.y, 0.0, 0.0));
    imageStore(Vel, ivec2(clm, row), vec4(vel.x, vel.y, 0.0, 0.0));
    float vel_len = length(vel);

    float fin[9];
    for (int i = 0; i < 9; i++) {
        float d = dot(DIRS[i], vel);
        fin[i] = T_WEIGHTS[i] * (1.0 + d * 3.0 + 4.5 * d * d + 1.5 * vel_len * vel_len);
    }
    imageStore(Fin, ivec3(clm, row, 0), vec4(fin[0], fin[1], fin[2], 0.0));
    imageStore(Fin, ivec3(clm, row, 1), vec4(fin[3], fin[4], fin[5], 0.0));
    imageStore(Fin, ivec3(clm, row, 2), vec4(fin[6], fin[7], fin[8], 0.0));

    float x = float(clm) - float(size.x) * 0.25;
    float y = float(row) - float(size.y) * 0.5;
    if (x * x + y * y - 1.5 * x * y < r * r) {
        imageStore(Obstacle, ivec2(clm, row), uvec4(1));
        imageStore(Screen, ivec2(clm, row), vec4(0.0, 1.0, 0.0, 1.0));
    } else {
        imageStore(Obstacle, ivec2(clm, row), uvec4(0));
        imageStore(Screen, ivec2(clm, row), from_hsv(vec4( 180.0 / 3.141 * acos(vel.x / vel_len), 0.7, vel_len / 0.09, 1.0)));
    }    

}
