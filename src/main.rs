// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod mpv;
mod mpv_sys;

use std::ffi::c_void;
use std::num::NonZeroU32;
use std::rc::Rc;

slint::include_modules!();

use glow::HasContext;

macro_rules! define_scoped_binding {
    (struct $binding_ty_name:ident => $obj_name:path, $param_name:path, $binding_fn:ident, $target_name:path) => {
        struct $binding_ty_name {
            saved_value: Option<$obj_name>,
            gl: Rc<glow::Context>,
        }

        impl $binding_ty_name {
            unsafe fn new(gl: &Rc<glow::Context>, new_binding: Option<$obj_name>) -> Self {
                let saved_value =
                    NonZeroU32::new(gl.get_parameter_i32($param_name) as u32).map($obj_name);

                gl.$binding_fn($target_name, new_binding);
                Self {
                    saved_value,
                    gl: gl.clone(),
                }
            }
        }

        impl Drop for $binding_ty_name {
            fn drop(&mut self) {
                unsafe {
                    self.gl.$binding_fn($target_name, self.saved_value);
                }
            }
        }
    };
    (struct $binding_ty_name:ident => $obj_name:path, $param_name:path, $binding_fn:ident) => {
        struct $binding_ty_name {
            saved_value: Option<$obj_name>,
            gl: Rc<glow::Context>,
        }

        impl $binding_ty_name {
            unsafe fn new(gl: &Rc<glow::Context>, new_binding: Option<$obj_name>) -> Self {
                let saved_value =
                    NonZeroU32::new(gl.get_parameter_i32($param_name) as u32).map($obj_name);

                gl.$binding_fn(new_binding);
                Self {
                    saved_value,
                    gl: gl.clone(),
                }
            }
        }

        impl Drop for $binding_ty_name {
            fn drop(&mut self) {
                unsafe {
                    self.gl.$binding_fn(self.saved_value);
                }
            }
        }
    };
}

define_scoped_binding!(struct ScopedTextureBinding => glow::NativeTexture, glow::TEXTURE_BINDING_2D, bind_texture, glow::TEXTURE_2D);
define_scoped_binding!(struct ScopedFrameBufferBinding => glow::NativeFramebuffer, glow::DRAW_FRAMEBUFFER_BINDING, bind_framebuffer, glow::DRAW_FRAMEBUFFER);

struct DemoTexture {
    texture: glow::Texture,
    width: u32,
    height: u32,
    fbo: glow::Framebuffer,
    gl: Rc<glow::Context>,
}

impl DemoTexture {
    unsafe fn new(gl: &Rc<glow::Context>, width: u32, height: u32) -> Self {
        let fbo = gl
            .create_framebuffer()
            .expect("Unable to create framebuffer");

        let texture = gl.create_texture().expect("Unable to allocate texture");

        let _saved_texture_binding = ScopedTextureBinding::new(gl, Some(texture));

        let old_unpack_alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
        let old_unpack_row_length = gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH);
        let old_unpack_skip_pixels = gl.get_parameter_i32(glow::UNPACK_SKIP_PIXELS);
        let old_unpack_skip_rows = gl.get_parameter_i32(glow::UNPACK_SKIP_ROWS);

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, width as i32);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as _,
            width as _,
            height as _,
            0,
            glow::RGBA as _,
            glow::UNSIGNED_BYTE as _,
            None,
        );

        let _saved_fbo_binding = ScopedFrameBufferBinding::new(gl, Some(fbo));

        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );

        debug_assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, old_unpack_alignment);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, old_unpack_row_length);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, old_unpack_skip_pixels);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, old_unpack_skip_rows);

        Self {
            texture,
            width,
            height,
            fbo,
            gl: gl.clone(),
        }
    }

    unsafe fn with_texture_as_active_fbo<R>(&self, callback: impl FnOnce() -> R) -> R {
        let _saved_fbo = ScopedFrameBufferBinding::new(&self.gl, Some(self.fbo));
        callback()
    }
}

impl Drop for DemoTexture {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.texture);
        }
    }
}

struct DemoRenderer {
    gl: Rc<glow::Context>,
    texture: DemoTexture,
    mpv_gl: mpv::MpvRenderContext,
}

impl DemoRenderer {
    fn new(
        mpv: mpv::Mpv,
        gl: glow::Context,
        get_proc_addr: Box<&dyn Fn(&std::ffi::CStr) -> *const c_void>,
    ) -> Self {
        let gl = Rc::new(gl);
        let texture = unsafe { DemoTexture::new(&gl, 320, 200) };

        let mpv_gl = mpv::MpvRenderContext::new(mpv, get_proc_addr).unwrap();
        Self {
            gl,
            mpv_gl,
            texture,
        }
    }
}

impl DemoRenderer {
    /// Returns `Some` when a texture has changed, and None if previous one has
    /// been rendered to
    fn render(&mut self, width: u32, height: u32) -> Option<slint::Image> {
        let recreated = unsafe {
            let gl = &self.gl;

            let recreated = if self.texture.width != width || self.texture.height != height {
                self.texture = DemoTexture::new(gl, width, height);
                true
            } else {
                false
            };

            self.texture.with_texture_as_active_fbo(|| {
                let mut saved_viewport: [i32; 4] = [0, 0, 0, 0];
                gl.get_parameter_i32_slice(glow::VIEWPORT, &mut saved_viewport);

                gl.viewport(0, 0, self.texture.width as _, self.texture.height as _);

                self.mpv_gl.render(
                    self.texture.texture.0.get(),
                    self.texture.width as _,
                    self.texture.height as _,
                ).unwrap();

                gl.viewport(
                    saved_viewport[0],
                    saved_viewport[1],
                    saved_viewport[2],
                    saved_viewport[3],
                );
            });

            recreated
        };

        if recreated {
            let result_texture = unsafe {
                slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                    self.texture.texture.0,
                    (self.texture.width, self.texture.height).into(),
                )
                .build()
            };
            Some(result_texture)
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
        .set_rendering_notifier(move |state, graphics_api| {
            // eprintln!("rendering state {:#?}", state);

            match state {
                slint::RenderingState::RenderingSetup => {
                    let mut mpv = mpv::Mpv::new().unwrap();
                    mpv.set_option_string("terminal", "yes");
                    mpv.set_option_string("msg-level", "all=v");
                    mpv.initialize().unwrap();

                    let (context, get_proc_addr) = match graphics_api {
                        slint::GraphicsAPI::NativeOpenGL { get_proc_address } => unsafe {
                            (glow::Context::from_loader_function_cstr(|s| get_proc_address(s)), get_proc_address)
                        },
                        _ => panic!("Non-opengl graphics api"),
                    };
                    let get_proc_addr = Box::new(*get_proc_addr);
                    renderer = Some(DemoRenderer::new(mpv, context, get_proc_addr))
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
            }
        });
    if let Err(error) = r {
        match error {
            slint::SetRenderingNotifierError::Unsupported => eprintln!("This example requires the use of the GL backend. Please run with the environment variable SLINT_BACKEND=GL set."),
            _ => unreachable!()
        }
        std::process::exit(1);
    }

    app.run().unwrap();
}
