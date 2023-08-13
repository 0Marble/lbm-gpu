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

const LBM_STEP_SRC: &[u8] = include_bytes!("../shaders/lbm_step.glsl");
const LBM_INIT_SRC: &[u8] = include_bytes!("../shaders/lbm_init.glsl");
const VERTEX_SHADER_SRC: &[u8] = include_bytes!("../shaders/vertex.glsl");
const FRAGMENT_SHADER_SRC: &[u8] = include_bytes!("../shaders/fragment.glsl");

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

        return Err(std::str::from_utf8(&buffer).unwrap().to_string());
    }

    Ok(p)
}

fn make_2d_texture(
    width: i32,
    height: i32,
    pos: gl::types::GLuint,
    storage: gl::types::GLenum,
) -> gl::types::GLuint {
    let mut texture = 0;
    gl_call!(gl::CreateTextures(gl::TEXTURE_2D, 1, &mut texture));
    gl_call!(gl::TextureStorage2D(texture, 1, storage, width, height));
    gl_call!(gl::BindImageTexture(
        pos,
        texture,
        0,
        gl::FALSE,
        0,
        gl::READ_WRITE,
        storage
    ));

    texture
}

fn make_3d_texture(
    width: i32,
    height: i32,
    pos: gl::types::GLuint,
    storage: gl::types::GLenum,
) -> gl::types::GLuint {
    let mut texture = 0;
    gl_call!(gl::CreateTextures(gl::TEXTURE_2D_ARRAY, 1, &mut texture));
    // gl_call!(gl::TextureStorage2D(texture, 1, storage, width, height));
    gl_call!(gl::TextureStorage3D(texture, 1, storage, width, height, 3));
    gl_call!(gl::BindImageTexture(
        pos,
        texture,
        0,
        gl::TRUE,
        0,
        gl::READ_WRITE,
        storage
    ));

    texture
}

fn main() {
    let width = 8 * 8 * 20;
    let height = 8 * 8 * 10;

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
        .window("LBM", width as u32, height as u32)
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
    let fragment_shader = compile_shader(FRAGMENT_SHADER_SRC, gl::FRAGMENT_SHADER).unwrap();
    let draw_program = compile_program(&[vertex_shader, fragment_shader]).unwrap();

    let lbm_init_shader = compile_shader(LBM_INIT_SRC, gl::COMPUTE_SHADER).unwrap();
    let lbm_init_program = compile_program(&[lbm_init_shader]).unwrap();
    let lbm_step_shader = compile_shader(LBM_STEP_SRC, gl::COMPUTE_SHADER).unwrap();
    let lbm_step_program = compile_program(&[lbm_step_shader]).unwrap();

    let screen_texture = make_2d_texture(width, height, 0, gl::RGBA32F);
    let fin_texture = make_3d_texture(width, height, 1, gl::RGBA32F);
    let fout_texture = make_3d_texture(width, height, 2, gl::RGBA32F);
    let vel_texure = make_2d_texture(width, height, 3, gl::RG32F);
    let initial_vel_texure = make_2d_texture(width, height, 4, gl::RG32F);
    let obstacle_texture = make_2d_texture(width, height, 5, gl::R8UI);

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

    gl_call!(gl::UseProgram(draw_program));
    let screen_uniform = gl_call!(gl::GetUniformLocation(
        draw_program,
        b"screen".as_ptr() as *const i8
    ));
    gl_call!(gl::BindTextureUnit(0, screen_texture));
    gl_call!(gl::Uniform1i(screen_uniform, 0));

    gl_call!(gl::UseProgram(lbm_init_program));
    gl_call!(gl::BindTextureUnit(0, screen_texture));
    gl_call!(gl::BindTextureUnit(1, fin_texture));
    gl_call!(gl::BindTextureUnit(2, fout_texture));
    gl_call!(gl::BindTextureUnit(3, vel_texure));
    gl_call!(gl::BindTextureUnit(4, initial_vel_texure));
    gl_call!(gl::BindTextureUnit(5, obstacle_texture));
    gl_call!(gl::DispatchCompute(width as u32 / 8, height as u32 / 8, 1));
    gl_call!(gl::MemoryBarrier(gl::ALL_BARRIER_BITS));

    gl_call!(gl::UseProgram(lbm_step_program));
    gl_call!(gl::BindTextureUnit(0, screen_texture));
    gl_call!(gl::BindTextureUnit(1, fin_texture));
    gl_call!(gl::BindTextureUnit(2, fout_texture));
    gl_call!(gl::BindTextureUnit(3, vel_texure));
    gl_call!(gl::BindTextureUnit(4, initial_vel_texure));
    gl_call!(gl::BindTextureUnit(5, obstacle_texture));

    'running: for _ in 0.. {
        for e in event_pump.poll_iter() {
            match e {
                Event::Quit { .. } => break 'running,
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Escape) => {
                    //
                    break 'running;
                }
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Return) => {
                    //
                    gl_call!(gl::UseProgram(lbm_step_program));
                    gl_call!(gl::DispatchCompute(width as u32 / 8, height as u32 / 8, 1));
                    gl_call!(gl::MemoryBarrier(gl::ALL_BARRIER_BITS));
                }
                Event::KeyDown { keycode, .. } if keycode == Some(Keycode::Tab) => {
                    //
                }
                _ => {}
            }
        }

        gl_call!(gl::Clear(gl::COLOR_BUFFER_BIT));

        gl_call!(gl::UseProgram(draw_program));
        gl_call!(gl::BindVertexArray(vao));
        gl_call!(gl::DrawArrays(gl::TRIANGLES, 0, VERTICES.len() as i32));

        window.gl_swap_window();
    }

    gl_call!(gl::DeleteTextures(1, &fin_texture));
    gl_call!(gl::DeleteTextures(1, &fout_texture));
    gl_call!(gl::DeleteTextures(1, &vel_texure));
    gl_call!(gl::DeleteTextures(1, &initial_vel_texure));
    gl_call!(gl::DeleteTextures(1, &obstacle_texture));
    gl_call!(gl::DeleteTextures(1, &screen_texture));

    gl_call!(gl::DeleteBuffers(1, &vbo));
    gl_call!(gl::DeleteVertexArrays(1, &vao));
    gl_call!(gl::DeleteProgram(draw_program));
    gl_call!(gl::DeleteProgram(lbm_init_program));
    gl_call!(gl::DeleteProgram(lbm_step_program));
}
