use std::time::Instant;
use anyhow::{anyhow, Error};
use glam::Vec4;
use tracing::{error};
use vulkanalia::vk;
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use types::resource_handle::{Resource, ResourceHandle};
use crate::application::gfx::device::DeviceCtx;
use crate::application::gfx::frame_graph::frame_graph::{AttachmentSource, ClearValues, FrameGraphInstance, RenderPassAttachment, RenderPassCreateInfos};
use crate::application::gfx::surface::{Surface, SurfaceCtx};
use crate::application::gfx::swapchain::Swapchain;
use crate::application::input_manager::InputManager;
use crate::engine::{EngineCtx};
use crate::options::{WindowOptions};

pub struct AppWindow {
    minimized: bool,
    last_frame_time: Instant,

    swapchain: Option<Resource<Swapchain>>,
    surface: Resource<Surface>,
    window: Option<Window>,
    _engine: EngineCtx,
    input_manager: InputManager,
    pub delta_time: f64,
    
    self_ctx: ResourceHandle<AppWindow>
}
pub type WindowCtx = ResourceHandle<AppWindow>;

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
            swapchain: None,
            _engine: ctx,
            input_manager: InputManager::default(),
            delta_time: 0.0,
            minimized: false,
            last_frame_time: Instant::now(),
            self_ctx: Default::default(),
        });
        window.self_ctx = window.handle();
        Ok(window)
    }

    pub fn init_swapchain(&mut self, ctx: DeviceCtx) -> Result<(), Error> {








        let mut present_pass = ctx.find_or_create_render_pass(RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: ClearValues::Color(Vec4::new(1f32, 0f32, 0f32, 1f32)),
                source: AttachmentSource::Surface(self.self_ctx.clone()),
            }],
            depth_attachment: None,
        });

        present_pass.attach(ctx.find_or_create_render_pass(RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: ClearValues::Color(Vec4::new(1f32, 1f32, 0f32, 1f32)),
                source: AttachmentSource::Image(vk::Format::R16G16B16A16_SFLOAT),
            }],
            depth_attachment: Some(RenderPassAttachment {
                clear_value: ClearValues::DontClear,
                source: AttachmentSource::Image(vk::Format::D32_SFLOAT),
            }),
        }).as_ref());

        let framegraph = FrameGraphInstance::new(swapchain.device.clone(), present_pass, crate::application::gfx::swapchain::MAX_FRAMES_IN_FLIGHT + 1);
        
        
        
        
        self.swapchain = Some(Swapchain::new(ctx, self.self_ctx.clone())?);
        Ok(())
    }

    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<(), Error> {
        self.input_manager.consume_event(&event);
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let elapsed = self.last_frame_time.elapsed().as_secs_f64();
                self.delta_time = elapsed;
                self.last_frame_time = Instant::now();
                if !self.minimized {
                    self.input_manager.begin_frame();
                    let should_recreate = match self.swapchain.as_mut().unwrap().render() {
                        Ok(should_recreate) => { should_recreate }
                        Err(err) => {
                            error!("Failed to render frame : {}", err);
                            false
                        }
                    };
                    if should_recreate {
                        match self.swapchain.as_mut().unwrap().create_or_recreate_swapchain() {
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

    pub fn surface(&self) -> SurfaceCtx {
        self.surface.handle()
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