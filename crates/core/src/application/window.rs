use std::sync::Weak;
use std::time::Instant;
use anyhow::{anyhow, Error};
use tracing::{error};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use types::rwslock::RwSLock;
use crate::application::gfx::device::DeviceSharedData;
use crate::application::gfx::instance::Instance;
use crate::application::gfx::surface::Surface;
use crate::application::gfx::swapchain::Swapchain;
use crate::engine::{CtxEngine, Engine};
use crate::options::{WindowOptions};

pub struct AppWindow {
    window: Option<Window>,
    surface: Option<Surface>,
    swapchain: Option<RwSLock<Swapchain>>,
    minimized: bool,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub left_pressed: bool,
    pub right_pressed: bool,
    last_frame_time: Instant,
    pub delta_time: f64,
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
    pub fn new( ctx: Weak<vulkanalia::Instance>, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<Self, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        let window = event_loop.create_window(attributes)?;
        let surface = Surface::new(ctx, &window)?;
        Ok(Self {
            window: Some(window),
            surface: Some(surface),
            swapchain: None,
            minimized: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            left_pressed: false,
            right_pressed: false,
            last_frame_time: Instant::now(),
            delta_time: 0.0,
        })
    }

    pub fn init_swapchain(&mut self, ctx: DeviceSharedData) -> Result<(), Error> {
        self.swapchain = Some(RwSLock::new(Swapchain::new(ctx)?));
        Ok(())
    }
    
    pub fn id(&self) -> Result<WindowId, Error> {
        Ok(self.ptr()?.id())
    }

    pub fn ptr(&self) -> Result<&Window, Error> {
        self.window.as_ref().ok_or(anyhow!("Window have been destroyed"))
    }

    pub fn surface(&self) -> Result<&Surface, Error> {
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
            WindowEvent::CursorMoved { device_id: _device_id, position } => {
                self.mouse_x = position.x;
                self.mouse_y = position.y;
            }
            WindowEvent::MouseInput { device_id: _device_id, state: element_state, button: mouse_button } => {
                match element_state {
                    ElementState::Pressed => {
                        match mouse_button {
                            MouseButton::Left => { self.left_pressed = true }
                            MouseButton::Right => { self.right_pressed = true }
                            MouseButton::Middle => {}
                            MouseButton::Back => {}
                            MouseButton::Forward => {}
                            MouseButton::Other(_) => {}
                        }
                    }
                    ElementState::Released => {
                        match mouse_button {
                            MouseButton::Left => { self.left_pressed = false }
                            MouseButton::Right => { self.right_pressed = false }
                            MouseButton::Middle => {}
                            MouseButton::Back => {}
                            MouseButton::Forward => {}
                            MouseButton::Other(_) => {}
                        }
                    }
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let elapsed = self.last_frame_time.elapsed().as_secs_f64();
                self.delta_time = elapsed;
                self.last_frame_time = Instant::now();
                let ctx = CtxAppWindow { engine: ctx, window: self };
                if !self.minimized {
                    let should_recreate = match self.swapchain.as_ref().unwrap().write()?.render(&ctx) {
                        Ok(should_recreate) => { should_recreate }
                        Err(err) => {
                            error!("Failed to render frame : {}", err);
                            false
                        }
                    };
                    if should_recreate {
                        match self.swapchain.as_ref().unwrap().write()?.create_or_recreate_swapchain(&ctx, self.surface.as_ref().unwrap()) {
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