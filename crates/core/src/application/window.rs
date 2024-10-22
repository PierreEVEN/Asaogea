use std::sync::{Arc, Weak};
use std::time::Instant;
use anyhow::{anyhow, Error};
use tracing::{error};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use types::rwarc::{RwArc, RwArcReadOnly, RwWeakReadOnly};
use types::rwslock::RwSLock;
use crate::application::gfx::device::DeviceCtx;
use crate::application::gfx::instance::Instance;
use crate::application::gfx::surface::Surface;
use crate::application::gfx::swapchain::Swapchain;
use crate::application::input_manager::InputManager;
use crate::engine::{Engine, EngineCtx};
use crate::options::{WindowOptions};

pub struct AppWindow {
    data: RwArc<WindowData>,
    minimized: bool,
    last_frame_time: Instant,
}
pub struct WindowCtx(RwWeakReadOnly<WindowData>);
impl WindowCtx {
    pub fn get(&self) -> RwArcReadOnly<WindowData> {
        self.0.upgrade()
    }
}
pub struct WindowData {
    swapchain: RwSLock<Option<Swapchain>>,
    surface: Option<Surface>,
    window: Option<Window>,
    engine: EngineCtx,
    input_manager: InputManager,
    pub delta_time: f64,
}

impl WindowData {
    pub fn surface(&self) -> &Surface {
        self.surface.as_ref().unwrap()
    }
    pub fn ptr(&self) -> Result<&Window, Error> {
        self.window.as_ref().ok_or(anyhow!("Window have been destroyed"))
    }

    pub fn id(&self) -> Result<WindowId, Error> {
        Ok(self.ptr()?.id())
    }

    pub fn width(&self) -> Result<u32, Error> {
        Ok(self.window.as_ref().ok_or(anyhow!("Window is null"))?.inner_size().width)
    }

    pub fn height(&self) -> Result<u32, Error> {
        Ok(self.window.as_ref().ok_or(anyhow!("Window is null"))?.inner_size().height)
    }
    
    pub fn input_manager(&self) -> &InputManager {
        &self.input_manager
    }
}

impl AppWindow {}

impl AppWindow {
    pub fn new(ctx: EngineCtx, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<Self, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        let window = event_loop.create_window(attributes)?;
        let surface = Surface::new(ctx.get().read().instance(), &window)?;
        Ok(Self {
            data: RwArc::new(WindowData {
                window: Some(window),
                surface: Some(surface),
                swapchain: RwSLock::new(None),
                engine: ctx,
                input_manager: InputManager::default(),
                delta_time: 0.0,
            }),
            minimized: false,
            last_frame_time: Instant::now(),
        })
    }

    pub fn init_swapchain(&mut self, ctx: DeviceCtx) -> Result<(), Error> {
        *self.data.read().swapchain.write()? = Some(Swapchain::new(ctx, self.ctx())?);
        Ok(())
    }

    pub fn ctx(&self) -> WindowCtx {
        WindowCtx(self.data.downgrade_read_only())
    }

    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<(), Error> {
        self.data.write().input_manager.consume_event(&event);
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let elapsed = self.last_frame_time.elapsed().as_secs_f64();
                self.data.write().delta_time = elapsed;
                self.last_frame_time = Instant::now();
                if !self.minimized {
                    self.data.write().input_manager.begin_frame();
                    let should_recreate = match self.data.read().swapchain.write()?.as_mut().unwrap().render() {
                        Ok(should_recreate) => { should_recreate }
                        Err(err) => {
                            error!("Failed to render frame : {}", err);
                            false
                        }
                    };
                    if should_recreate {
                        match self.data.read().swapchain.write()?.as_mut().unwrap().create_or_recreate_swapchain() {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Failed to recreate swapchain : {}", err);
                            }
                        };
                    }
                }
                self.data.read().window.as_ref().unwrap().request_redraw();
            }
            WindowEvent::Resized(size) => {
                self.minimized = size.width == 0 || size.height == 0;
            }
            _ => (),
        }
        Ok(())
    }
}