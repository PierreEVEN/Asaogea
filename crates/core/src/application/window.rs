use anyhow::{anyhow, Error};
use tracing::{error};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use types::rwslock::RwSLock;
use crate::application::gfx::surface::Surface;
use crate::engine::{CtxEngine, Engine};
use crate::options::{WindowOptions};

pub struct AppWindow {
    window: Option<Window>,
    surface: Option<RwSLock<Surface>>,
    minimized: bool,
}

pub struct CtxAppWindow<'a> {
    engine: &'a CtxEngine<'a>,
    pub window: &'a AppWindow,
}

impl<'a> CtxAppWindow<'a> {
    pub fn new(engine: &'a CtxEngine, window: &'a AppWindow) -> Self {
        Self { engine, window }
    }
}

impl<'a> CtxAppWindow<'a> {
    pub fn engine(&self) -> &Engine {
        &self.engine.engine
    }
    pub fn ctx_engine(&self) -> &CtxEngine {
        &self.engine
    }
}

impl AppWindow {}

impl AppWindow {
    pub fn new(event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<Self, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        let window = event_loop.create_window(attributes)?;

        Ok(Self {
            window: Some(window),
            surface: None,
            minimized: false,
        })
    }

    pub fn init(&mut self, ctx: &CtxEngine) -> Result<(), Error> {
        let ctx = CtxAppWindow { engine: ctx, window: self };
        self.surface = Some(RwSLock::new(Surface::new(&ctx)?));
        Ok(())
    }

    pub fn id(&self) -> Result<WindowId, Error> {
        Ok(self.ptr()?.id())
    }

    pub fn ptr(&self) -> Result<&Window, Error> {
        self.window.as_ref().ok_or(anyhow!("Window have been destroyed"))
    }
    
    pub fn surface(&self) -> Result<&RwSLock<Surface>, Error> {
        self.surface.as_ref().ok_or(anyhow!("Surface is not valid. Window::init() have not been called or the windows have been destroyed"))
    }

    pub fn width(&self) -> Result<u32, Error> {
        Ok(self.window.as_ref().ok_or(anyhow!("Window is null"))?.inner_size().width)
    }

    pub fn height(&self) -> Result<u32, Error> {
        Ok(self.window.as_ref().ok_or(anyhow!("Window is null"))?.inner_size().height)
    }

    pub fn window_event(&mut self, ctx: &CtxEngine, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<(), Error> {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let ctx = CtxAppWindow { engine: ctx, window: self };
                if !self.minimized {
                    let should_recreate = match self.surface()?.write()?.render(&ctx) {
                        Ok(should_recreate) => { should_recreate }
                        Err(err) => {
                            error!("Failed to render frame : {}", err);
                            false
                        }
                    };
                    if should_recreate {
                        match self.surface()?.write()?.create_or_recreate_swapchain(&ctx) {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Failed to recreate swapchain : {}", err);
                            }
                        };
                    }
                }
                self.window.as_ref().unwrap().request_redraw();
            }
            WindowEvent::Resized(size) => {
                self.minimized = size.width == 0 || size.height == 0;
            }
            _ => (),
        }
        Ok(())
    }
    
    pub fn destroy(&mut self, ctx: &CtxEngine) -> Result<(), Error> {
        if let Some(surface) = &self.surface {
            let ctx = CtxAppWindow { engine: ctx, window: self };
            surface.write()?.destroy(&ctx)?;
        }
        self.surface = None;
        Ok(())
    }
}