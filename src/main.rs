mod gl;
mod mpv;

slint::include_modules!();

struct DemoRenderer {
    gl: std::rc::Rc<glow::Context>,
    texture: gl::Texture,
    mpv_gl: mpv::MpvRenderContext,
}

impl DemoRenderer {
    fn new<'a>(
        mpv: mpv::Mpv,
        gl: glow::Context,
        get_proc_addr: &'a &mpv::CreateContextFn<'a>,
    ) -> Self {
        let gl = std::rc::Rc::new(gl);
        // random size, will be set for real in render
        let texture = unsafe { gl::Texture::new(&gl, 320, 200) };
        let mut mpv_gl = mpv::MpvRenderContext::new(mpv, get_proc_addr).unwrap();
        mpv_gl.unset_update_callback();
        Self {
            gl,
            mpv_gl,
            texture,
        }
    }

    fn texture(&self) -> slint::Image {
        unsafe {
            slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                self.texture.texture.0,
                (self.texture.width, self.texture.height).into(),
            )
            .build()
        }
    }

    /// Returns `Some` when a texture has changed, and None if previous one has
    /// been rendered to
    fn render(&mut self, width: u32, height: u32) -> Option<slint::Image> {
        let recreated = unsafe {
            let gl = &self.gl;

            let recreated = if self.texture.width != width || self.texture.height != height {
                self.texture = gl::Texture::new(gl, width, height);
                true
            } else {
                false
            };

            self.texture.with_texture_as_active_fbo(|| {
                self.mpv_gl
                    .render(
                        self.texture.fbo.0.get(),
                        self.texture.width as _,
                        self.texture.height as _,
                    )
                    .unwrap();
            });

            recreated
        };

        if recreated {
            Some(self.texture())
        } else {
            None
        }
    }
}

fn main() {
    let app = App::new().unwrap();

    let mut renderer = None;

    let app_weak = app.as_weak();

    let r = app
        .window()
        .set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {
                let mut mpv = mpv::Mpv::new().unwrap();
                mpv.set_option_string("terminal", "yes");
                mpv.set_option_string("msg-level", "all=v");
                mpv.initialize().unwrap();

                let get_proc_address = match graphics_api {
                    slint::GraphicsAPI::NativeOpenGL { get_proc_address } => get_proc_address,
                    _ => panic!("Non-opengl graphics api"),
                };

                let context =
                    unsafe { glow::Context::from_loader_function_cstr(|s| get_proc_address(s)) };
                let mut mpv = DemoRenderer::new(mpv, context, get_proc_address);
                mpv.mpv_gl
                    .command(&[
                        "loadfile",
                        "/home/morj/videos/Screencast_20230507_134547.webm",
                    ])
                    .unwrap();

                renderer = Some(mpv);
            }
            slint::RenderingState::BeforeRendering => {
                if let (Some(renderer), Some(app)) = (renderer.as_mut(), app_weak.upgrade()) {
                    let mb_texture = renderer.render(
                        app.get_requested_texture_width() as u32,
                        app.get_requested_texture_height() as u32,
                    );
                    if let Some(texture) = mb_texture {
                        app.set_texture(texture);
                    }
                    app.window().request_redraw();
                }
            }
            slint::RenderingState::AfterRendering => {}
            slint::RenderingState::RenderingTeardown => {
                drop(renderer.take());
            }
            _ => {}
        });
    if let Err(error) = r {
        match error {
            slint::SetRenderingNotifierError::Unsupported =>
                eprintln!("This example requires the use of the GL backend. Please run with the environment variable SLINT_BACKEND=GL set."),
            _ => unreachable!()
        }
        std::process::exit(1);
    }

    app.run().unwrap();
}
