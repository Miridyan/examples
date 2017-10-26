extern crate gdk;
extern crate gtk;

extern crate gl;
extern crate glutin;


#[cfg(feature = "gtk_3_16")]
use gdk::{GLContextExt,
          WindowExt as GdkWindowExt};

#[cfg(feature = "gtk_3_16")]
use gtk::{ContainerExt,
          GLArea, GLAreaExt,
          WidgetExt,
          Window, GtkWindowExt,
          WindowType};

use gl::types::*;
use glutin::{Api, GlContext, GlRequest};

use std::mem;
use std::ptr;
use std::str;
use std::ffi::CString;
use std::os::raw::c_void;
use std::time::SystemTime;
use std::sync::{Arc, Mutex};

#[cfg(feature = "gtk_3_16")]
pub struct GlWindow {
    pub window: Box<Window>,
    pub gl: Box<GLArea>,
}

#[cfg(feature = "gtk_3_16")]
impl GlWindow {
    pub fn init() -> GlWindow {
        if gtk::init().is_err() {
            panic!("Failed to initialize gtk");
        }

        let _window = Window::new(WindowType::Toplevel);
        let _gl = GLArea::new();

        _window.add(&_gl);
        _window.set_title("OpenGL Demo");
        _window.set_default_size(1200, 800);
        _window.connect_delete_event(|_, _| {
                gtk::main_quit();
                gtk::Inhibit(false)
            });

        GlWindow {
            window: Box::new(_window),
            gl: Box::new(_gl),
        }
    }

    pub fn get_gl(&self) -> &Box<GLArea> {
        &self.gl
    }

    pub fn show(&self) {
        self.window.show_all();
    }

    pub fn exec(&self) {
        self.show();
        gtk::main();
    }
}


#[cfg(feature = "gtk_3_16")]
pub fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    unsafe {
        let shader = gl::CreateShader(ty);
        let c_str = CString::new(src.as_bytes()).unwrap();

        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        if status != (gl::TRUE as GLint) {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

            let mut buf = Vec::with_capacity(len as usize);
            gl::GetShaderInfoLog(
                shader,
                len,
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );

            panic!(
                "{}",
                str::from_utf8(&buf).ok().expect(
                    "ShaderLogInfo not valid UTF-8",
                )
            );
        }
        shader
    }
}

#[cfg(feature = "gtk_3_16")]
pub fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    unsafe {
        let program = gl::CreateProgram();
        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);
        gl::LinkProgram(program);

        let mut status = gl::FALSE as GLint;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        if status != (gl::TRUE as GLint) {
            let mut len: GLint = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);

            let mut buf = Vec::with_capacity(len as usize);
            gl::GetProgramInfoLog(
                program,
                len,
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );

            panic!(
                "{}",
                str::from_utf8(&buf).ok().expect(
                    "ProgramLogInfo not valid UTF-8",
                )
            );
        }
        program
    }
}


#[cfg(feature = "gtk_3_16")]
fn main() {
    let app = GlWindow::init();
    let gl = app.get_gl();

    let start = SystemTime::now();
    /**
     * We need to enclose our gl objects in Arc<Mutex<T>> in order to pass them around to
     * multiple closures and so each closure can access and mutate the gl objects. 
     *
     * Box<T> will NOT work in this situation.
     */
    let vbo = Arc::new(Mutex::new(0));
    let ebo = Arc::new(Mutex::new(0));
    let vao = Arc::new(Mutex::new(0));
    let prog = Arc::new(Mutex::new(0));

    gl.connect_create_context(|gl_area| {
            /**
             * Here i get window parent of the `gl_area` and create a gl context for it.
             * This step is actually not necessary unless you want to request a specific
             * gl version for the context or if you want to enable debug.
             */
            let gl_context = match gl_area.get_window().unwrap().create_gl_context() {
                Ok(context) => context,
                Err(error) => panic!("{:?}", error),
            };
            gl_context.set_required_version(3, 0);
            gl_context
        });

    gl.connect_resize(|_gl_area, width, height| {
            unsafe {
                gl::Viewport(0, 0, width, height);
            }
        });

    {
        let (vbo, ebo, vao, prog) = (vbo.clone(), ebo.clone(), vao.clone(), prog.clone());
        gl.connect_realize(move |gl_area| {
                gl_area.get_context().unwrap().make_current();

                /**
                 * This is a dummy context that we're using to load opengl functions. There
                 * are more elegant solutions than this one, but you must use this method if
                 * you want your software to compile and run on Windows. 
                 *
                 * Windows OpenGL function loading works differently than on linux. On linux,
                 * I can query the system at anytime with a crate like `static_library` and
                 * it will return all of the OpenGL functions that I request with 
                 * `gl::load_with()`. Windows OpenGL loading is context based, so you need to
                 * have a valid context that the system's OpenGL provider will recognize (WGL
                 * in this case). The `static_library` approach won't work on Windows for this
                 * reason.
                 *
                 * A context must be defined and must also declare which version of OpenGL it
                 * would like to use. Once this is done, you can query functions from the system
                 * and load them with `gl::load_with()`. Fortunately, we don't have to have that
                 * valid WGL context on windows in order to render, we just need some OpenGL
                 * context to be made current and OpenGL will draw to that. Some once we use
                 * the Headless Context from glutin to load the OpenGL functions, we can just drop
                 * it and not have to worry about it.
                 *
                 * The downside of this approach is really just the pulling in of a bunch of
                 * extra dependencies which increases compile time. 
                 */
                let context = glutin::HeadlessRendererBuilder::new(0, 0)
                    .with_gl(GlRequest::Specific(Api::OpenGl, (3, 0)))
                    .build_strict()
                    .unwrap();

                gl::load_with(|s| {
                    (context.get_proc_address(s) as *const c_void)
                });

                drop(context);

                unsafe {
                    let verts: [f32; 8] = [
                         0.5,  0.5,
                        -0.5,  0.5,
                        -0.5, -0.5,
                         0.5, -0.5
                    ];
                    let indices: [u16; 6] = [
                        0, 1, 2,
                        2, 3, 0
                    ];

                    let vert_shader_source = r"
                        #version 330

                        layout (location = 0) in vec2 position;

                        void main() {
                            gl_Position = vec4(position, 0.0, 1.0);
                        }
                    ";
                    let frag_shader_source = r"
                        #version 330

                        out vec4 color;

                        void main() {
                            color = vec4(1.0f, 1.0f, 1.0f, 1.0f);
                        }
                    ";

                    let mut vbo = vbo.lock().unwrap();
                    let mut ebo = ebo.lock().unwrap();
                    let mut vao = vao.lock().unwrap();
                    let mut prog = prog.lock().unwrap();

                    let vert_shader = compile_shader(
                        vert_shader_source,
                        gl::VERTEX_SHADER,
                    );
                    let frag_shader = compile_shader(
                        frag_shader_source,
                        gl::FRAGMENT_SHADER,
                    );
                    *prog = link_program(vert_shader, frag_shader);

                    gl::GenBuffers(1, &mut *vbo);
                    gl::GenBuffers(1, &mut *ebo);
                    gl::GenVertexArrays(1, &mut *vao);

                    gl::BindBuffer(gl::ARRAY_BUFFER, *vbo);
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        8 * 4 /*8x f32*/,
                        mem::transmute(&verts),
                        gl::STATIC_DRAW,
                    );
                    gl::BindBuffer(gl::ARRAY_BUFFER, 0);

                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, *ebo);
                    gl::BufferData(
                        gl::ELEMENT_ARRAY_BUFFER,
                        6 * 2 /*6x i16*/,
                        mem::transmute(&indices),
                        gl::STATIC_DRAW,
                    );
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
                }
            });
    }

    {
        let (vbo, ebo, vao, prog) = (vbo.clone(), ebo.clone(), vao.clone(), prog.clone());
        gl.connect_render(move |gl_area, _context| {
                unsafe {
                    let now = SystemTime::now();
                    let dur = now.duration_since(start).expect("RIP");
                    let millis = dur.as_secs() * 1_000 + (dur.subsec_nanos() as u64) / 1_000_000;

                    let t = ((millis % 2000) as f32) / 1000.0;

                    gl::ClearColor(t / 2.0, 1.0 - (t / 2.0), 1.0, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);

                    let vbo = vbo.lock().unwrap();
                    let vao = vao.lock().unwrap();
                    let ebo = ebo.lock().unwrap();
                    let prog = prog.lock().unwrap();

                    gl::BindVertexArray(*vao);
                    gl::BindBuffer(gl::ARRAY_BUFFER, *vbo);
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, *ebo);
                    gl::UseProgram(*prog);
                    gl::EnableVertexAttribArray(0);
                    gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 0, 0 as *mut c_void);
                    gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_SHORT, 0 as *mut c_void);
                    gl::DisableVertexAttribArray(0);

                    gl::UseProgram(0);
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
                    gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                    gl::BindVertexArray(0);
                }

                gl_area.queue_render();
                gtk::Inhibit(false)
            });
    }

    app.exec();
}

#[cfg(not(feature = "gtk_3_16"))]
fn main() {
    println!("You must compile with `--features gtk_3_16`!");
}
