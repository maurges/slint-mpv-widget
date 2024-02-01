use std::ffi::c_void;

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
mod sys;

pub struct Mpv {
    ptr: *mut sys::mpv_handle,
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // Safety: TODO
        unsafe { sys::mpv_terminate_destroy(self.ptr) }
    }
}

type Result<T> = std::result::Result<T, Error>;

// Safety: mpv docs say: "concurrent calls to different mpv_handles are always safe"
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

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

    pub fn set_option_string(&self, name: &str, value: &str) {
        // Safety: TODO
        let _ = unsafe {
            let name = std::ffi::CString::new(name).unwrap();
            let value = std::ffi::CString::new(value).unwrap();
            sys::mpv_set_option_string(self.ptr, name.as_ptr(), value.as_ptr())
        };
    }

    pub fn initialize(&self) -> Result<()> {
        // Safety: TODO
        let e = unsafe { sys::mpv_initialize(self.ptr) };
        Error::raise(e)
    }

    pub fn command(&self, args: &[&str]) -> Result<()> {
        let args_buf = args
            .iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let mut args = args_buf.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        args.push(std::ptr::null());
        // Safety: TODO
        let e = unsafe { sys::mpv_command(self.ptr, args.as_mut_ptr()) };
        Error::raise(e)
    }

    #[allow(dead_code)]
    pub fn set_wakeup_callback<F>(&self, cb: F)
    where
        F: FnMut(),
    {
        let closure = Box::new(cb);
        unsafe {
            let closure_ptr = Box::into_raw(closure);
            let closure_ptr = closure_ptr as *mut c_void;
            sys::mpv_set_wakeup_callback(self.ptr, Some(call_closure_0::<F>), closure_ptr)
        }
    }
}

pub mod property {
    use std::ffi::c_void;

    use super::sys;

    pub(super) mod name {
        use std::ffi::CStr;

        pub const DURATION: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"duration\0") };
        pub const TIME_POS: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"time-pos\0") };
        pub const PAUSE: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"pause\0") };
    }

    #[derive(Debug, Clone)]
    pub enum Property {
        Duration(f64),
        TimePos(f64),
        Pause(bool),
    }

    impl Property {
        pub unsafe fn from_raw(prop: *const sys::mpv_event_property) -> Result<Self, ConvertError> {
            debug_assert!(!prop.is_null());

            unsafe fn read_double(
                prop: *const sys::mpv_event_property,
            ) -> Result<f64, ConvertError> {
                if (*prop).format == sys::mpv_format_MPV_FORMAT_DOUBLE {
                    let data = (*prop).data as *const f64;
                    debug_assert!(!data.is_null());
                    Ok(*data)
                } else {
                    Err(ConvertError::TypeError)
                }
            }
            unsafe fn read_bool(
                prop: *const sys::mpv_event_property,
            ) -> Result<bool, ConvertError> {
                if (*prop).format == sys::mpv_format_MPV_FORMAT_FLAG {
                    let data = (*prop).data as *const std::ffi::c_int;
                    debug_assert!(!data.is_null());
                    Ok(*data != 0)
                } else {
                    Err(ConvertError::TypeError)
                }
            }

            let name = std::ffi::CStr::from_ptr((*prop).name);
            if name == name::DURATION {
                read_double(prop).map(Property::Duration)
            } else if name == name::TIME_POS {
                read_double(prop).map(Property::TimePos)
            } else if name == name::PAUSE {
                read_bool(prop).map(Property::Pause)
            } else {
                Err(ConvertError::Invalid)
            }
        }

        pub fn into_raw(self) -> (&'static std::ffi::CStr, u32, Box<c_void>) {
            fn make_ptr<T>(x: T) -> Box<c_void> {
                let boxed = Box::new(x);
                let ptr = Box::into_raw(boxed);
                let ptr = ptr as *mut c_void;
                // Safety: safe as it was constructed from boxed value
                unsafe { Box::from_raw(ptr) }
            }
            match self {
                Property::Duration(val) => (
                    name::DURATION,
                    sys::mpv_format_MPV_FORMAT_DOUBLE,
                    make_ptr(val),
                ),
                Property::TimePos(val) => (
                    name::TIME_POS,
                    sys::mpv_format_MPV_FORMAT_DOUBLE,
                    make_ptr(val),
                ),
                Property::Pause(val) => (
                    name::PAUSE,
                    sys::mpv_format_MPV_FORMAT_FLAG,
                    make_ptr(std::ffi::c_int::from(val)),
                ),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum ConvertError {
        TypeError,
        Invalid,
    }
}

/// GL render context
pub struct MpvRenderContext {
    ptr: *mut sys::mpv_render_context,
    parent: std::sync::Arc<Mpv>,
}

impl Drop for MpvRenderContext {
    fn drop(&mut self) {
        // Safety: TODO
        unsafe { sys::mpv_render_context_free(self.ptr) };
    }
}

impl std::ops::Deref for MpvRenderContext {
    type Target = Mpv;

    fn deref(&self) -> &Mpv {
        &self.parent
    }
}

/// Mirrors slint's create context fn. Useful for me to not get lost in pointer
/// casts
pub type CreateContextFn<'a> = dyn Fn(&std::ffi::CStr) -> *const c_void + 'a;

impl MpvRenderContext {
    pub fn new<'a>(
        parent: std::sync::Arc<Mpv>,
        get_proc_addr: &'a &CreateContextFn<'a>,
    ) -> Result<Self> {
        // this is monomorphic because it's only ever used for slint's function
        // type, and doing otherwise would require too many plumbing, not worth
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
        let e =
            unsafe { sys::mpv_render_context_create(&mut ptr, parent.ptr, params.as_mut_ptr()) };
        Error::raises(Self { ptr, parent }, e)
    }

    #[allow(dead_code)]
    pub fn unset_update_callback(&mut self) {
        unsafe { sys::mpv_render_context_set_update_callback(self.ptr, None, std::ptr::null_mut()) }
    }

    pub fn set_update_callback<F>(&mut self, cb: F)
    where
        F: FnMut(),
    {
        let closure = Box::new(cb);
        unsafe {
            let closure_ptr = Box::into_raw(closure);
            let closure_ptr = closure_ptr as *mut c_void;
            sys::mpv_render_context_set_update_callback(
                self.ptr,
                Some(call_closure_0::<F>),
                closure_ptr,
            )
        }
    }

    pub fn render(&mut self, fbo: u32, width: i32, height: i32) -> Result<()> {
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
        let e = unsafe { sys::mpv_render_context_render(self.ptr, params.as_mut_ptr()) };
        Error::raise(e)
    }
}

pub mod event {
    //! https://mpv.io/manual/master/#properties

    use super::{property::Property, sys};

    #[derive(Debug, Clone)]
    pub enum MpvEvent {
        PropertyChange(Property),
        Unsupported,
        /// Could not parse event. More events might be available
        Error,
    }

    pub(super) fn convert_event(e: *mut sys::mpv_event) -> Option<MpvEvent> {
        debug_assert!(!e.is_null());
        unsafe {
            if (*e).event_id == sys::mpv_event_id_MPV_EVENT_NONE {
                None
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE {
                let prop = (*e).data as *const sys::mpv_event_property;
                match Property::from_raw(prop) {
                    Ok(p) => Some(MpvEvent::PropertyChange(p)),
                    Err(_) => Some(MpvEvent::Error),
                }
            } else {
                Some(MpvEvent::Unsupported)
            }
        }
    }
}

#[allow(dead_code)]
impl Mpv {
    pub fn wait_event(&self, timeout: f64) -> Option<event::MpvEvent> {
        let event_ptr = unsafe { sys::mpv_wait_event(self.ptr, timeout) };
        event::convert_event(event_ptr)
    }

    pub fn set_property(&self, p: property::Property) -> Result<()> {
        let (name, format, data_box) = p.into_raw();
        let data_ptr = Box::into_raw(data_box);
        // Safety: safe as ptr is valid, and data from property is also valid
        let e = unsafe { sys::mpv_set_property(self.ptr, name.as_ptr(), format, data_ptr) };
        drop(unsafe { Box::from_raw(data_ptr) });
        Error::raise(e)
    }

    /// Notify mpv that we want to observe `duration` events
    pub fn observe_duration(&self) -> Result<()> {
        unsafe {
            let e = sys::mpv_observe_property(
                self.ptr,
                0,
                property::name::DURATION.as_ptr(),
                sys::mpv_format_MPV_FORMAT_DOUBLE,
            );
            Error::raise(e)
        }
    }

    pub fn get_pause(&self) -> Result<bool> {
        let mut buffer: std::ffi::c_int = 0;
        let e = unsafe {
            let ptr = &mut buffer as *mut std::ffi::c_int;
            sys::mpv_get_property(
                self.ptr,
                property::name::PAUSE.as_ptr(),
                sys::mpv_format_MPV_FORMAT_FLAG,
                ptr as *mut c_void,
            )
        };
        Error::raises(buffer != 0, e)
    }

    /// Notify mpv that we want to observe `time-pos` events
    pub fn observe_time_pos(&self) -> Result<()> {
        unsafe {
            let e = sys::mpv_observe_property(
                self.ptr,
                0,
                property::name::TIME_POS.as_ptr(),
                sys::mpv_format_MPV_FORMAT_DOUBLE,
            );
            Error::raise(e)
        }
    }
}

/// Safety: `closure_ptr` must be Box<F>
unsafe extern "C" fn call_closure_0<F>(closure_ptr: *mut c_void)
where
    F: FnMut(),
{
    let closure_ptr = closure_ptr as *mut F;
    let mut closure: Box<F> = Box::from_raw(closure_ptr);
    closure();
    Box::leak(closure);
}

#[derive(Debug)]
pub struct Error(i32);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Safety: perfectly fine function
        let desc = unsafe { sys::mpv_error_string(self.0) };
        // Safety: guaranteed c string
        let desc = unsafe { std::ffi::CStr::from_ptr(desc) };
        let desc = desc.to_string_lossy();
        write!(f, "{}", desc)
    }
}

impl std::error::Error for Error {}

impl Error {
    fn raise(e: i32) -> Result<()> {
        Self::raises((), e)
    }
    fn raises<T>(x: T, e: i32) -> Result<T> {
        if e != sys::mpv_error_MPV_ERROR_SUCCESS {
            Err(Error(e))
        } else {
            Ok(x)
        }
    }
}
