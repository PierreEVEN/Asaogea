use anyhow::{anyhow, Error};
use tracing::{error};
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use types::resource_handle::{Resource, ResourceHandle, ResourceHandleMut};
use crate::core::gfx::frame_graph::frame_graph_definition::{Renderer};
use crate::core::gfx::surface::{Surface, SurfaceCtx};
use crate::core::gfx::swapchain::{Swapchain, SwapchainCtx};
use crate::core::input_manager::InputManager;
use crate::engine::{EngineCtx};
use crate::options::{WindowOptions};

pub type WindowCtx = ResourceHandle<AppWindow>;
pub type WindowCtxMut = ResourceHandleMut<AppWindow>;
pub struct AppWindow {
    minimized: bool,

    swapchain: Resource<Swapchain>,
    surface: Resource<Surface>,
    window: Option<Window>,
    engine: EngineCtx,
    input_manager: InputManager,
    self_ctx: ResourceHandle<AppWindow>,
}

impl AppWindow {}

impl AppWindow {
    pub fn new(ctx: EngineCtx, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<Resource<Self>, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        let window = event_loop.create_window(attributes)?;
        let surface = Surface::new(ctx.instance(), &window)?;
        let mut window = Resource::new(Self {
            window: Some(window),
            surface,
            swapchain: Resource::default(),
            engine: ctx,
            input_manager: InputManager::default(),
            minimized: false,
            self_ctx: Default::default(),
        });
        window.self_ctx = window.handle();
        Ok(window)
    }

    pub fn init_swapchain(&mut self) {
        self.swapchain = Swapchain::new(self.engine.instance().device(), self.self_ctx.clone()).unwrap();
    }

    pub fn set_renderer(&mut self, renderer: Renderer) -> Result<(), Error> {
        self.swapchain.set_renderer(renderer);
        Ok(())
    }

    pub fn engine(&self) -> &EngineCtx {
        &self.engine
    }

    pub fn window_event(&mut self, _: &ActiveEventLoop, event: WindowEvent) -> Result<(), Error> {
        self.input_manager.consume_event(&event);
        match event {
            WindowEvent::RedrawRequested => {
                if !self.minimized {
                    self.input_manager.begin_frame();
                    if self.swapchain.is_valid() {
                        if let Err(err) = self.swapchain.render() {
                            error!("Failed to render frame : {}", err);
                        };
                    }
                }
            }
            WindowEvent::Resized(size) => {
                self.minimized = size.width == 0 || size.height == 0;
            }
            _ => (),
        }
        Ok(())
    }

    pub fn surface(&self) -> SurfaceCtx {
        self.surface.handle()
    }
    pub fn swapchain(&self) -> SwapchainCtx {
        self.swapchain.handle()
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