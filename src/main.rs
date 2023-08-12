use sdl2::{event::Event, keyboard::Keycode};

macro_rules! gl_call {
    ($func:expr) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let res = $func;
            let err = gl::GetError();
            if err != 0 {
                panic!(
                    "[{}] at {}, {}:{}",
                    err,
                    stringify!($func),
                    file!(),
                    line!()
                )
            } else {
                res
            }
        }
    }};
}

const ROTATE_HUE_SHADER_SRC: &[u8] = br#"
#version 460 core
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
layout(rgba32f, binding = 0) uniform image2D screen;

vec4 to_hsv(vec4 rgb) {
    float r = rgb.r;
    float g = rgb.g;
    float b = rgb.b;
    float a = rgb.a;

    float cmax = max(r, max(g, b));
    float cmin = min(r, min(g, b));
    float delta = cmax - cmin;

    float h = 0.0;
    float s = 0.0;
    float v = 0.0;

    if (delta == 0.0) {
        h = 0.0;
    } else if (cmax == r) {
        h = 60.0 * mod((g - b) / delta, 6.0);
    } else if (cmax == g) {
        h = 60.0 * ((b - r) / delta + 2.0);
    } else {
        h = 60.0 * ((r - g) / delta + 4.0);
    }

    if (cmax == 0.0) {
        s = 0.0;
    } else {
        s = delta / cmax;
    }

    v = cmax;

    return vec4(h,s,v,a);
}

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

void main()
{
	ivec2 pixel_coords = ivec2(gl_GlobalInvocationID.xy);
    vec4 start_color = imageLoad(screen, pixel_coords);
    vec4 hsv = to_hsv(start_color);
    hsv.r = mod(hsv.r + 1.0, 360.0);
    vec4 end_color = from_hsv(hsv);

    imageStore(screen, pixel_coords, end_color);
}
"#;

const GEN_IMAGE_SHADER_SRC: &[u8] = br#"
#version 460 core
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
layout(rgba32f, binding = 0) uniform image2D screen;
void main()
{
	vec4 pixel = vec4(0.075, 0.133, 0.173, 1.0);
	ivec2 pixel_coords = ivec2(gl_GlobalInvocationID.xy);
	
	ivec2 dims = imageSize(screen);
	float x = -(float(pixel_coords.x * 2 - dims.x) / dims.x); // transforms to [-1.0, 1.0]
	float y = -(float(pixel_coords.y * 2 - dims.y) / dims.y); // transforms to [-1.0, 1.0]

	float fov = 90.0;
	vec3 cam_o = vec3(0.0, 0.0, -tan(fov / 2.0));
	vec3 ray_o = vec3(x, y, 0.0);
	vec3 ray_d = normalize(ray_o - cam_o);

	vec3 sphere_c = vec3(0.0, 0.0, -5.0);
	float sphere_r = 1.0;

	vec3 o_c = ray_o - sphere_c;
	float b = dot(ray_d, o_c);
	float c = dot(o_c, o_c) - sphere_r * sphere_r;
	float intersectionState = b * b - c;
	vec3 intersection = ray_o + ray_d * (-b + sqrt(b * b - c));

	if (intersectionState >= 0.0)
	{
		pixel = vec4((normalize(intersection - sphere_c) + 1.0) / 2.0, 1.0);
	}

	imageStore(screen, pixel_coords, pixel);
}"#;

const VERTEX_SHADER_SRC: &[u8] = br#"
#version 460 core
layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 uvs;
out vec2 UVs;
void main()
{
	gl_Position = vec4(pos.x, pos.y, 0.0, 1.0);
	UVs = uvs;
}
"#;

const FRAGMENT_SHADER_SOURCE: &[u8] = br#"
#version 460 core
out vec4 FragColor;
uniform sampler2D screen;
in vec2 UVs;
void main()
{
	FragColor = texture(screen, UVs);
}
"#;

const VERTICES: [(f32, f32, f32, f32); 6] = [
    (-1.0, -1.0, 0.0, 0.0),
    (1.0, -1.0, 1.0, 0.0),
    (-1.0, 1.0, 0.0, 1.0),
    (-1.0, 1.0, 0.0, 1.0),
    (1.0, -1.0, 1.0, 0.0),
    (1.0, 1.0, 1.0, 1.0),
];

fn compile_shader(src: &[u8], kind: gl::types::GLenum) -> Result<gl::types::GLuint, String> {
    let s = gl_call!(gl::CreateShader(kind));
    gl_call!(gl::ShaderSource(
        s,
        1,
        [src.as_ptr() as *const i8].as_ptr(),
        [src.len() as i32].as_ptr()
    ));
    gl_call!(gl::CompileShader(s));

    let mut has_compiled = 0;
    gl_call!(gl::GetShaderiv(s, gl::COMPILE_STATUS, &mut has_compiled));
    if has_compiled != gl::TRUE as gl::types::GLint {
        let mut info_log_length = 0;
        gl_call!(gl::GetShaderiv(
            s,
            gl::INFO_LOG_LENGTH,
            &mut info_log_length
        ));

        if info_log_length > 0 {
            let mut buffer = vec![0; info_log_length as usize];
            gl_call!(gl::GetShaderInfoLog(
                s,
                info_log_length,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut gl::types::GLchar
            ));
            return Err(std::str::from_utf8(&buffer).unwrap().to_string());
        }
    }

    Ok(s)
}

fn compile_program(shaders: &[gl::types::GLuint]) -> Result<gl::types::GLuint, String> {
    let p = gl_call!(gl::CreateProgram());

    for s in shaders {
        gl_call!(gl::AttachShader(p, *s));
        gl_call!(gl::DeleteShader(*s));
    }

    gl_call!(gl::LinkProgram(p));
    gl_call!(gl::ValidateProgram(p));
    let mut status = 0;
    gl_call!(gl::GetProgramiv(p, gl::LINK_STATUS, &mut status));
    if status != (gl::TRUE as gl::types::GLint) {
        let mut log_length = 0;
        gl_call!(gl::GetProgramiv(p, gl::INFO_LOG_LENGTH, &mut log_length));
        let mut buffer: Vec<_> = vec![0; log_length as usize];
        gl_call!(gl::GetProgramInfoLog(
            p,
            log_length,
            std::ptr::null_mut(),
            buffer.as_mut_ptr() as *mut gl::types::GLchar
        ));

        return Err(std::str::from_utf8(&buffer[..]).unwrap().to_string());
    }

    Ok(p)
}

fn main() {
    let title = "LBM";
    let width = 800;
    let height = 800;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let gl_attributes = video_subsystem.gl_attr();
    gl_attributes.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attributes.set_double_buffer(true);
    gl_attributes.set_multisample_samples(4);
    gl_attributes.set_framebuffer_srgb_compatible(true);
    gl_attributes.set_context_flags().debug().set();
    gl_attributes.set_context_version(4, 6);

    let window = video_subsystem
        .window(title, width, height)
        .opengl()
        .resizable()
        .build()
        .unwrap();

    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);
    window.gl_make_current(&gl_context).unwrap();
    window
        .subsystem()
        .gl_set_swap_interval(sdl2::video::SwapInterval::VSync)
        .unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    gl_call!(gl::ClearColor(1.0, 1.0, 1.0, 1.0));
    gl_call!(gl::Enable(gl::BLEND));
    gl_call!(gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA));

    let vertex_shader = compile_shader(VERTEX_SHADER_SRC, gl::VERTEX_SHADER).unwrap();
    let fragment_shader = compile_shader(FRAGMENT_SHADER_SOURCE, gl::FRAGMENT_SHADER).unwrap();
    let draw_program = compile_program(&[vertex_shader, fragment_shader]).unwrap();
    let gen_image_shader = compile_shader(GEN_IMAGE_SHADER_SRC, gl::COMPUTE_SHADER).unwrap();
    let gen_image_program = compile_program(&[gen_image_shader]).unwrap();
    let rotate_hue_shader = compile_shader(ROTATE_HUE_SHADER_SRC, gl::COMPUTE_SHADER).unwrap();
    let rotate_hue_program = compile_program(&[rotate_hue_shader]).unwrap();

    let mut texture = 0;
    gl_call!(gl::CreateTextures(gl::TEXTURE_2D, 1, &mut texture));
    gl_call!(gl::TextureParameteri(
        texture,
        gl::TEXTURE_MIN_FILTER,
        gl::NEAREST as i32
    ));

    gl_call!(gl::TextureParameteri(
        texture,
        gl::TEXTURE_MAG_FILTER,
        gl::NEAREST as i32
    ));
    gl_call!(gl::TextureParameteri(
        texture,
        gl::TEXTURE_WRAP_S,
        gl::CLAMP_TO_EDGE as i32
    ));
    gl_call!(gl::TextureParameteri(
        texture,
        gl::TEXTURE_WRAP_T,
        gl::CLAMP_TO_EDGE as i32
    ));
    gl_call!(gl::TextureStorage2D(
        texture,
        1,
        gl::RGBA32F,
        width as i32,
        height as i32
    ));
    gl_call!(gl::BindImageTexture(
        0,
        texture,
        0,
        gl::FALSE,
        0,
        gl::READ_WRITE,
        gl::RGBA32F
    ));

    let (mut vao, mut vbo) = (0, 0);
    gl_call!(gl::GenVertexArrays(1, &mut vao));
    gl_call!(gl::GenBuffers(1, &mut vbo));
    gl_call!(gl::BindVertexArray(vao));
    gl_call!(gl::BindBuffer(gl::ARRAY_BUFFER, vbo));
    gl_call!(gl::EnableVertexAttribArray(0));
    gl_call!(gl::VertexAttribPointer(
        0,
        2,
        gl::FLOAT,
        gl::FALSE,
        4 * std::mem::size_of::<f32>() as i32,
        std::ptr::null()
    ));
    gl_call!(gl::EnableVertexAttribArray(1));
    gl_call!(gl::VertexAttribPointer(
        1,
        2,
        gl::FLOAT,
        gl::FALSE,
        4 * std::mem::size_of::<f32>() as i32,
        std::ptr::null::<f32>().offset(2) as _
    ));
    gl_call!(gl::BufferData(
        gl::ARRAY_BUFFER,
        std::mem::size_of_val(&VERTICES) as isize,
        VERTICES.as_ptr() as _,
        gl::STATIC_DRAW
    ));

    let texture_uniform = gl_call!(gl::GetUniformLocation(
        draw_program,
        b"screen".as_ptr() as *const i8
    ));

    gl_call!(gl::UseProgram(gen_image_program));
    gl_call!(gl::DispatchCompute(width / 8, width / 8, 1));
    gl_call!(gl::MemoryBarrier(gl::ALL_BARRIER_BITS));

    'running: loop {
        for e in event_pump.poll_iter() {
            match e {
                Event::Quit { .. } => break 'running,
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Escape) => {
                    //
                    break 'running;
                }
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Return) => {
                    //
                }
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Tab) => {
                    //
                }
                _ => {}
            }
        }

        gl_call!(gl::Clear(gl::COLOR_BUFFER_BIT));

        gl_call!(gl::UseProgram(draw_program));
        gl_call!(gl::BindTextureUnit(0, texture));
        gl_call!(gl::Uniform1i(texture_uniform, 0));
        gl_call!(gl::BindVertexArray(vao));
        gl_call!(gl::DrawArrays(gl::TRIANGLES, 0, VERTICES.len() as i32));

        gl_call!(gl::UseProgram(rotate_hue_program));
        gl_call!(gl::DispatchCompute(width / 8, width / 8, 1));
        gl_call!(gl::MemoryBarrier(gl::ALL_BARRIER_BITS));

        window.gl_swap_window();
    }

    gl_call!(gl::DeleteTextures(1, &texture));
    gl_call!(gl::DeleteBuffers(1, &vbo));
    gl_call!(gl::DeleteVertexArrays(1, &vao));
    gl_call!(gl::DeleteProgram(draw_program));
    gl_call!(gl::DeleteProgram(gen_image_program));
    gl_call!(gl::DeleteProgram(rotate_hue_program));
}
