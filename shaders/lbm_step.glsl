#version 460 core
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in; 

layout(rgba32f, binding = 0) uniform writeonly image2D Screen;
layout(rgba32f, binding = 1) uniform image2DArray Fin;
layout(rgba32f, binding = 2) uniform image2DArray Fout;
layout(rg32f, binding = 3) uniform image2D Vel;
layout(rg32f, binding = 4) uniform readonly image2D Initial_vel;
layout(r8ui, binding = 5) uniform readonly uimage2D Obstacle;

vec2 DIRS[9] = { 
    vec2(1.0, 1.0), vec2(1.0, 0.0), vec2(1.0, -1.0), 
    vec2(0.0, 1.0), vec2(0.0, 0.0), vec2(0.0, -1.0), 
    vec2(-1.0, 1.0), vec2(-1.0, 0.0), vec2(-1.0, -1.0), 
};

ivec2 DISCRETE_DIRS[9] = { 
    ivec2(1, 1), ivec2(1, 0), ivec2(1, -1), 
    ivec2(0, 1), ivec2(0, 0), ivec2(0, -1), 
    ivec2(-1, 1), ivec2(-1, 0), ivec2(-1, -1), 
};

float T_WEIGHTS[9] = {
    1.0/36.0, 1.0/9.0, 1.0/36.0,
    1.0/9.0, 4.0/9.0, 1.0/9.0,
    1.0/36.0, 1.0/9.0, 1.0/36.0,
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

    float reynolds_number = 1000.0;
    float ulb = 0.04;
    float r = float(size.y / 9);
    float omega = 1.0 / (3.0 * ulb * r / reynolds_number + 0.5);


    float fin[9];
    if (clm + 1 == size.x) {
    // outflow from the right
        for (int i = 0; i < 2; i++) {
            vec3 f = imageLoad(Fin, ivec3(clm, row, i)).xyz;
            for (int j = 0; j < 3; j++) {
                fin[i * 3 + j] = f[j];
            }
        }
        vec4 from = imageLoad(Fin, ivec3(clm - 1, row, 2));
        fin[6] = from.x;
        fin[7] = from.y;
        fin[8] = from.z;
    } else {    
        for (int i = 0; i < 3; i++) {
            vec3 f = imageLoad(Fin, ivec3(clm, row, i)).xyz;
            for (int j = 0; j < 3; j++) {
                fin[i * 3 + j] = f[j];
            }
        }
    }

    // density and velocity
    float density = 0.0;
    vec2 velocity = vec2(0.0, 0.0);
    if (clm != 0) {
        for (int i = 0; i < 9; i++) {
            density += fin[i];
            velocity += fin[i] * DIRS[i];
        }
        velocity /= density;
    } else {
        velocity = imageLoad(Initial_vel, ivec2(clm, row)).xy;
        density = (fin[3] + fin[4] + fin[5] + 2.0 * (fin[6] + fin[7] + fin[8])) / (1.0 - velocity.x);
    }
    
    // equilibrium
    float vel_len = length(velocity);
    float equilibrium[9];
    for (int i = 0; i < 9; i++) {
        float d = dot(DIRS[i], velocity);
        equilibrium[i] = density * T_WEIGHTS[i] * (1.0 + d * 3.0 + 4.5 * d * d - 1.5 * vel_len * vel_len);
    }
    if (clm == 0) {
        for (int i = 0; i < 3; i++) {
            fin[i] = equilibrium[i] + fin[8 - i] - equilibrium[8 - i];
        }
    }

    // collide
    float fout[9];
    if (imageLoad(Obstacle, ivec2(clm, row)).x == 0) {
        for (int i = 0; i < 9; i++) {
            fout[i] = fin[i] - omega * (fin[i] - equilibrium[i]);
        }
    } else {
        for (int i = 0; i < 9; i++) {
            fout[i] = fin[8 - i];
        }
    }

    imageStore(Fout, ivec3(clm, row, 0), vec4(fout[0], fout[1], fout[2], 0.0));
    imageStore(Fout, ivec3(clm, row, 1), vec4(fout[3], fout[4], fout[5], 0.0));
    imageStore(Fout, ivec3(clm, row, 2), vec4(fout[6], fout[7], fout[8], 0.0));
    imageStore(Vel, ivec2(clm, row), vec4(velocity.x, velocity.y, 0.0, 0.0));

    if (imageLoad(Obstacle, ivec2(clm, row)).x != 0) {
        imageStore(Screen, ivec2(clm, row), vec4(0.0, 1.0, 0.0, 1.0));
    } else if (vel_len < 0.1) {
        imageStore(Screen, ivec2(clm, row), from_hsv(vec4( 180.0 / 3.141 * acos(velocity.x / vel_len), 0.7, vel_len / 0.09, 1.0)));
    } else {
        imageStore(Screen, ivec2(clm, row), vec4(0.0, 0.0, 1.0, 1.0));
    }

    memoryBarrier(); // wait for all to finish

    if (clm != 0 && clm + 1 != size.x) {
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                int from_x = clm - DISCRETE_DIRS[i * 3 + j].x;
                int from_y = row - DISCRETE_DIRS[i * 3 + j].y;
                if (from_y == -1) {
                    from_y = size.y - 1;
                } else if (from_y == size.y) {
                    from_y = 0;
                }

                fin[i * 3 + j] = imageLoad(Fout, ivec3(from_x, from_y, i))[j];
            }
        }
    }
    imageStore(Fin, ivec3(clm, row, 0), vec4(fin[0], fin[1], fin[2], 0.0));
    imageStore(Fin, ivec3(clm, row, 1), vec4(fin[3], fin[4], fin[5], 0.0));
    imageStore(Fin, ivec3(clm, row, 2), vec4(fin[6], fin[7], fin[8], 0.0)); 
}