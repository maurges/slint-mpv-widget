use std::ffi::c_void;

use crate::mpv_sys as sys;

pub struct Mpv {
    ptr: *mut sys::mpv_handle,
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // Safety: TODO
        unsafe { sys::mpv_terminate_destroy(self.ptr) };
    }
}

impl Mpv {
    #[must_use]
    pub fn new() -> Option<Self> {
        // Safety: TODO
        let ptr = unsafe { sys::mpv_create() };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub fn set_option_string(&mut self, name: &str, value: &str) {
        let _ = unsafe {
            let name = std::ffi::CString::new(name).unwrap();
            let value = std::ffi::CString::new(value).unwrap();
            sys::mpv_set_option_string(
                self.ptr,
                name.as_ptr(),
                value.as_ptr(),
            )
        };
    }

    #[must_use]
    pub fn initialize(&mut self) -> Option<()> {
        if unsafe { sys::mpv_initialize(self.ptr) } < 0 {
            None
        } else {
            Some(())
        }
    }

    #[must_use]
    pub fn command(&mut self, args: &[&str]) -> Option<()> {
        let args_buf = args.iter().map(|s| std::ffi::CString::new(*s).unwrap()).collect::<Vec<_>>();
        let mut args = args_buf.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        args.push(std::ptr::null());
        let r = unsafe {
            sys::mpv_command(self.ptr, args.as_mut_ptr())
        };
        if r < 0 {
            None
        } else {
            Some(())
        }
    }
}

pub struct MpvRenderContext {
    ptr: *mut sys::mpv_render_context,
    parent: Mpv,
}

impl Drop for MpvRenderContext {
    fn drop(&mut self) {
        unsafe { sys::mpv_render_context_free(self.ptr) };
    }
}

impl std::ops::Deref for MpvRenderContext {
    type Target = Mpv;

    fn deref(&self) -> &Mpv {
        &self.parent
    }
}
impl std::ops::DerefMut for MpvRenderContext {
    fn deref_mut(&mut self) -> &mut Mpv {
        &mut self.parent
    }
}

pub type CreateContextFn<'a> = dyn Fn(&std::ffi::CStr) -> *const c_void + 'a;

impl MpvRenderContext {
    #[must_use]
    pub fn new<'a>(
        parent: Mpv,
        get_proc_addr: &'a &CreateContextFn<'a>,
    ) -> Option<Self> {

        unsafe extern "C" fn call_closure(closure_ptr: *mut c_void, arg: *const i8) -> *mut c_void {
            let arg = std::ffi::CStr::from_ptr(arg);
            let closure_ptr = closure_ptr as *const &CreateContextFn;
            let closure: &&CreateContextFn = &*closure_ptr;
            let r = closure(arg).cast_mut();
            r
        }
        let closure_ptr = get_proc_addr as *const &CreateContextFn;
        let mut init_params = sys::mpv_opengl_init_params {
            get_proc_address: Some(call_closure),
            get_proc_address_ctx: closure_ptr as *mut c_void,
        };
        let mut params = [
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                data: sys::MPV_RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&mut init_params as *mut sys::mpv_opengl_init_params).cast(),
            },
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];

        let mut ptr = std::ptr::null_mut();
        let r =
            unsafe { sys::mpv_render_context_create(&mut ptr, parent.ptr, params.as_mut_ptr()) };
        if r < 0 {
            None
        } else {
            Some(Self { ptr, parent })
        }
    }

    pub fn unset_update_callback(&mut self) {
        unsafe {
            sys::mpv_render_context_set_update_callback(self.ptr, None, std::ptr::null_mut())
        }
    }

    pub fn set_update_callback(&mut self, cb: unsafe extern "C" fn(*mut c_void)) {
        unsafe {
            sys::mpv_render_context_set_update_callback(self.ptr, Some(cb), std::ptr::null_mut())
        }
    }

    #[must_use]
    pub fn render(&mut self, fbo: u32, width: i32, height: i32) -> Option<()> {
        let mut mpfbo = sys::mpv_opengl_fbo {
            fbo: fbo as i32,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip_y: i32 = 0;
        let mut params = [
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&mut mpfbo as *mut sys::mpv_opengl_fbo).cast(),
            },
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y,
                data: (&mut flip_y as *mut i32).cast(),
            },
            sys::mpv_render_param {
                type_: sys::mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let r = unsafe { sys::mpv_render_context_render(self.ptr, params.as_mut_ptr()) };
        if r < 0 {
            None
        } else {
            Some(())
        }
    }
}
