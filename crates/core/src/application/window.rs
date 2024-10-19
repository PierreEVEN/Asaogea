use anyhow::{anyhow, Error};
use tracing::{error};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::engine::CtxEngine;
use crate::options::{WindowOptions};

pub struct AppWindow {
    window: Option<Window>,
    minimized: bool,
}

pub struct CtxAppWindow<'a> {
    pub engine: &'a CtxEngine<'a>,
    pub window: &'a AppWindow,
}

impl AppWindow {}

impl AppWindow {
    pub fn new(event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<Self, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        Ok(Self {
            window: Some(event_loop.create_window(attributes)?),
            minimized: false,
        })
    }

    pub fn id(&self) -> Result<WindowId, Error> {
        Ok(self.ptr()?.id())
    }

    pub fn ptr(&self) -> Result<&Window, Error> {
        self.window.as_ref().ok_or(anyhow!("Window have been destroyed"))
    }

    pub fn window_event(&mut self, ctx: &CtxEngine, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<(), Error> {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if !self.minimized {
                    let instance = ctx.engine.instance_mut()?;
                    let window = self.window.as_ref().expect("Invalid window");
                    let should_recreate = match instance.surface().write().render(instance.device()) {
                        Ok(should_recreate) => { should_recreate }
                        Err(err) => {
                            error!("Failed to render frame : {}", err);
                            false
                        }
                    };
                    if should_recreate {
                        match instance.surface().write().create_or_recreate_swapchain(&instance, window) {
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
}