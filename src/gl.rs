// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

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
                    std::num::NonZeroU32::new(gl.get_parameter_i32($param_name) as u32)
                        .map($obj_name);

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
                    std::num::NonZeroU32::new(gl.get_parameter_i32($param_name) as u32)
                        .map($obj_name);

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

pub struct Texture {
    pub texture: glow::Texture,
    pub width: u32,
    pub height: u32,
    pub fbo: glow::Framebuffer,
    pub gl: Rc<glow::Context>,
}

impl Texture {
    pub unsafe fn new(gl: &Rc<glow::Context>, width: u32, height: u32) -> Self {
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

    pub unsafe fn with_texture_as_active_fbo<R>(&self, callback: impl FnOnce() -> R) -> R {
        let _saved_fbo = ScopedFrameBufferBinding::new(&self.gl, Some(self.fbo));
        callback()
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.texture);
        }
    }
}
