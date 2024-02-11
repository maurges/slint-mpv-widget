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

    /// See https://mpv.io/manual/master/#list-of-input-commands
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

pub mod property {
    use std::ffi::CStr;

    use super::sys;

    #[derive(Debug, Clone)]
    pub enum Property {
        Duration(Duration),
        TimePos(TimePos),
        Pause(Pause),
        AoVolume(AoVolume),
        AoMute(AoMute),
        Filename(Filename),
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Duration(pub f64);
    #[derive(Debug, Clone, Copy)]
    pub struct TimePos(pub f64);
    /// This one is not documented?? But it's mentioned in a lot of examples?
    #[derive(Debug, Clone, Copy)]
    pub struct Pause(pub bool);
    #[derive(Debug, Clone, Copy)]
    /// Until a video with audio track is loaded, volume property is not
    /// accessible
    pub struct AoVolume(pub f64);
    #[derive(Debug, Clone, Copy)]
    pub struct AoMute(pub bool);
    #[derive(Debug, Clone)]
    pub struct Filename(pub String);

    pub trait ReadProperty: Sized {
        const NAME: &'static CStr;
        const FORMAT: sys::mpv_format;
        type MpvRepr: Default + Copy;
        fn from_repr(val: Self::MpvRepr) -> Self;
    }
    pub trait WriteProperty: ReadProperty {
        fn to_repr(&self) -> Self::MpvRepr;
    }

    impl Property {
        pub unsafe fn from_raw(prop: *const sys::mpv_event_property) -> Result<Self, ConvertError> {
            debug_assert!(!prop.is_null());

            unsafe fn read<P: ReadProperty>(p: *const sys::mpv_event_property) -> Result<P, ConvertError> {
                if (*p).format == P::FORMAT {
                    let data = (*p).data as *const P::MpvRepr;
                    debug_assert!(!data.is_null());
                    Ok(P::from_repr(*data))
                } else {
                    Err(ConvertError::TypeError)
                }
            }

            let name = std::ffi::CStr::from_ptr((*prop).name);
            if name == Duration::NAME {
                read(prop).map(Property::Duration)
            } else if name == TimePos::NAME {
                read(prop).map(Property::TimePos)
            } else if name == Pause::NAME {
                read(prop).map(Property::Pause)
            } else if name == AoVolume::NAME {
                read(prop).map(Property::AoVolume)
            } else if name == AoMute::NAME {
                read(prop).map(Property::AoMute)
            } else if name == Filename::NAME {
                read(prop).map(Property::Filename)
            } else {
                Err(ConvertError::Invalid)
            }
        }
    }

    impl ReadProperty for Duration {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"duration\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_DOUBLE;
        type MpvRepr = f64;
        fn from_repr(val: Self::MpvRepr) -> Self {
            Self(val)
        }
    }
    impl WriteProperty for Duration {
        fn to_repr(&self) -> Self::MpvRepr {
            self.0
        }
    }
    impl ReadProperty for TimePos {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"time-pos\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_DOUBLE;
        type MpvRepr = f64;
        fn from_repr(val: Self::MpvRepr) -> Self {
            Self(val)
        }
    }
    impl WriteProperty for TimePos {
        fn to_repr(&self) -> Self::MpvRepr {
            self.0
        }
    }
    impl ReadProperty for Pause {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"pause\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_FLAG;
        type MpvRepr = std::ffi::c_int;
        fn from_repr(val: Self::MpvRepr) -> Self {
            Self(val != 0)
        }
    }
    impl WriteProperty for Pause {
        fn to_repr(&self) -> Self::MpvRepr {
            std::ffi::c_int::from(self.0)
        }
    }
    impl ReadProperty for AoVolume {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"ao-volume\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_DOUBLE;
        type MpvRepr = f64;
        fn from_repr(val: Self::MpvRepr) -> Self {
            Self(val)
        }
    }
    impl WriteProperty for AoVolume {
        fn to_repr(&self) -> Self::MpvRepr {
            self.0
        }
    }
    impl ReadProperty for AoMute {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"ao-mute\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_FLAG;
        type MpvRepr = std::ffi::c_int;
        fn from_repr(val: Self::MpvRepr) -> Self {
            Self(val != 0)
        }
    }
    impl WriteProperty for AoMute {
        fn to_repr(&self) -> Self::MpvRepr {
            std::ffi::c_int::from(self.0)
        }
    }
    impl ReadProperty for Filename {
        const NAME: &'static CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"filename\0") };
        const FORMAT: sys::mpv_format = sys::mpv_format_MPV_FORMAT_STRING;
        type MpvRepr = StrPtr;
        fn from_repr(val: Self::MpvRepr) -> Self {
            let cstr = unsafe { std::ffi::CStr::from_ptr(val.0) };
            // Or should I do bytestring and convert to bytes instead of lossy?
            // Or should I assume unicode and unwrap?
            Self(cstr.to_string_lossy().into_owned())
        }
    }

    #[derive(Debug, Clone)]
    pub enum ConvertError {
        TypeError,
        Invalid,
    }

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct StrPtr(*const i8);
    impl Default for StrPtr {
        fn default() -> Self {
            Self(std::ptr::null())
        }
    }
}

pub mod event {
    //! https://mpv.io/manual/master/#properties

    use super::{property::Property, sys};

    #[derive(Debug, Clone)]
    pub enum MpvEvent {
        StartFile{ playlist_entry_id: i64 },
        FileLoaded,
        PlaybackRestart,
        VideoReconfig,
        AudioReconfig,
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
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_START_FILE {
                let prop = (*e).data as *const sys::mpv_event_start_file;
                debug_assert!(!prop.is_null());
                Some(MpvEvent::StartFile { playlist_entry_id: (*prop).playlist_entry_id })
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_FILE_LOADED {
                Some(MpvEvent::FileLoaded)
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART {
                Some(MpvEvent::PlaybackRestart)
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_AUDIO_RECONFIG {
                Some(MpvEvent::AudioReconfig)
            } else if (*e).event_id == sys::mpv_event_id_MPV_EVENT_VIDEO_RECONFIG {
                Some(MpvEvent::VideoReconfig)
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

    /// Notify mpv that we want to observe property events
    pub fn observe_property<P: property::ReadProperty>(&self) -> Result<()> {
        unsafe {
            let e = sys::mpv_observe_property(
                self.ptr,
                0,
                P::NAME.as_ptr(),
                P::FORMAT,
            );
            Error::raise(e)
        }
    }

    pub fn get_property<P: property::ReadProperty>(&self) -> Result<P> {
        let mut buffer = P::MpvRepr::default();
        let e = unsafe {
            let ptr = &mut buffer as *mut P::MpvRepr;
            sys::mpv_get_property(
                self.ptr,
                P::NAME.as_ptr(),
                P::FORMAT,
                ptr as *mut c_void,
            )
        };
        Error::raises(P::from_repr(buffer), e)
    }

    pub fn set_property<P: property::WriteProperty>(&self, p: &P) -> Result<()> {
        let data = p.to_repr();
        let data_ptr = &data as *const P::MpvRepr;
        let data_ptr = data_ptr as *mut c_void;
        // Safety: safe as ptr is valid, and data from property is also valid
        let e = unsafe { sys::mpv_set_property(self.ptr, P::NAME.as_ptr(), P::FORMAT, data_ptr) };
        Error::raise(e)
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

pub struct Error(i32);

// panic uses Debug for showing error, not display? Fucking why?
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Safety: perfectly fine function
        let desc = unsafe { sys::mpv_error_string(self.0) };
        // Safety: guaranteed c string
        let desc = unsafe { std::ffi::CStr::from_ptr(desc) };
        let desc = desc.to_string_lossy();
        write!(f, "\"{}\"", desc)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
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
