/*
Copyright (c) 2024 maurges <contact@morj.men>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
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
        mpv: std::sync::Arc<mpv::Mpv>,
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
            let texture = unsafe {
                slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                    self.texture.texture.0,
                    (self.texture.width, self.texture.height).into(),
                )
                .build()
            };
            Some(texture)
        } else {
            None
        }
    }
}

fn main() {
    let mpv = mpv::Mpv::new().unwrap();
    mpv.set_option_string("terminal", "no").unwrap();
    mpv.initialize().unwrap();
    let mpv = std::sync::Arc::new(mpv);

    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    let app_weak_ = app_weak.clone();
    let mpv_ = mpv.clone();
    let _binding = std::thread::spawn(move || {
        mpv_.observe_property::<mpv::property::Duration>().unwrap();
        mpv_.observe_property::<mpv::property::TimePos>().unwrap();
        mpv_.observe_property::<mpv::property::AoVolume>().unwrap();
        mpv_.observe_property::<mpv::property::AoMute>().unwrap();
        mpv_.observe_property::<mpv::property::Filename>().unwrap();
        loop {
            if let Some(event) = mpv_.wait_event(1.0) {
                use mpv::event::MpvEvent;
                use mpv::property::Property;
                match event {
                    MpvEvent::PropertyChange(Property::Duration(t)) => {
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_duration(t.0 as f32);
                        });
                    }
                    MpvEvent::PropertyChange(Property::TimePos(t)) => {
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_position(t.0 as f32);
                        });
                    }
                    MpvEvent::PropertyChange(Property::AoVolume(t)) => {
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_volume(t.0 as f32);
                        });
                    }
                    MpvEvent::PropertyChange(Property::AoMute(_)) => {
                        // update audio value just in case (see below)
                        let mb_volume = mpv_.get_property::<mpv::property::AoVolume>();
                        let value = mb_volume.map(|t| t.0).unwrap_or(0.0);
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_volume(value as f32);
                        });
                    }
                    MpvEvent::PropertyChange(Property::Filename(t)) => {
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_title(t.0.into());
                        });
                    }
                    // Volume event is not emitted when changing from undefined
                    // to some number, so we workaround. But this still doesn't
                    // work in some cases, sooooooooooo
                    MpvEvent::AudioReconfig
                    | MpvEvent::VideoReconfig
                    | MpvEvent::PlaybackRestart => {
                        let mb_volume = mpv_.get_property::<mpv::property::AoVolume>();
                        // if not available, set to zero
                        let value = mb_volume.map(|t| t.0).unwrap_or(0.0);
                        eprintln!("audio reconfig: {}", value);
                        let _ = app_weak_.upgrade_in_event_loop(move |app| {
                            app.set_video_volume(value as f32);
                        });
                    }
                    _ => {}
                }
            }
            // check if event loop is still alive
            if app_weak_.upgrade_in_event_loop(|_| {}).is_err() {
                break;
            }
        }
    });

    let mpv_ = mpv.clone();
    app.on_toggle_pause(move || {
        let mpv::property::Pause(state) = mpv_.get_property().unwrap();
        mpv_.set_property(&mpv::property::Pause(!state)).unwrap();
    });
    let mpv_ = mpv.clone();
    app.on_toggle_mute(move || {
        let mpv::property::AoMute(state) = mpv_.get_property().unwrap();
        mpv_.set_property(&mpv::property::AoMute(!state)).unwrap();
    });
    let mpv_ = mpv.clone();
    app.on_seek(move |val| {
        mpv_.set_property(&mpv::property::TimePos(val as f64))
            .unwrap();
    });
    let mpv_ = mpv.clone();
    app.on_set_volume(move |val| {
        mpv_.set_property(&mpv::property::AoVolume(val as f64))
            .unwrap();
    });
    let mpv_ = mpv.clone();
    app.on_open_file(move || {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            mpv_.command(&["loadfile", path.to_str().unwrap()]).unwrap();
        }
    });

    let mut renderer = None;

    let r = app
        .window()
        .set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {

                let get_proc_address = match graphics_api {
                    slint::GraphicsAPI::NativeOpenGL { get_proc_address } => get_proc_address,
                    _ => panic!("Non-opengl graphics api"),
                };

                let context =
                    unsafe { glow::Context::from_loader_function_cstr(|s| get_proc_address(s)) };
                let mut mpv = DemoRenderer::new(mpv.clone(), context, get_proc_address);

                mpv.mpv_gl.set_update_callback(|| {
                    let _ = app_weak.upgrade_in_event_loop(|app| app.window().request_redraw());
                });

                mpv.mpv_gl
                    .command(&[
                        "loadfile",
                        "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/TearsOfSteel.mp4",
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
