use std::collections::HashMap;
use anyhow::{Error};
use tracing::{error, warn};
use vulkanalia::vk;
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{WindowAttributes, WindowId};
use types::resource_handle::{Resource, ResourceHandle};
use crate::application::gfx::frame_graph::frame_graph_definition::{FrameGraph, RenderPass, RenderPassAttachment, RenderTarget};
use crate::application::gfx::instance::{GfxConfig, Instance, InstanceCtx};
use crate::options::WindowOptions;
use crate::application::window::{AppWindow, WindowCtx};

pub struct Engine {
    data: Resource<EngineData>,
    default_window_settings: WindowOptions,
}

pub type EngineCtx = ResourceHandle<EngineData>;

pub struct EngineData {
    windows: HashMap<WindowId, Resource<AppWindow>>,
    instance: Resource<Instance>,
}
impl EngineData {
    pub fn instance(&self) -> InstanceCtx {
        self.instance.handle()
    }
}

impl Engine {
    pub fn new(default_window_settings: WindowOptions) -> Result<Self, Error> {
        let mut config = GfxConfig {
            validation_layers: true,
            required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
        };
        let mut data = Resource::new(EngineData
        {
            windows: Default::default(),
            instance: Resource::default(),
        });
        data.instance = Instance::new(data.handle(), &mut config)?;

        Ok(Self {
            data,
            default_window_settings,
        })
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(event_loop.run_app(self)?)
    }

    pub fn create_window(&mut self, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<WindowCtx, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();
        let mut window = AppWindow::new(self.ctx(), event_loop, options)?;
        let device = self.data.instance.get_or_create_device(window.handle());

        let forward_pass = RenderPass {
            color_attachments: vec![RenderPassAttachment {
                clear_value: Default::default(),
                source: RenderTarget::Internal(vk::Format::R16G16B16A16_SFLOAT)
            }],
            depth_attachment: Some(RenderPassAttachment {
                clear_value: Default::default(),
                source: RenderTarget::Internal(vk::Format::D32_SFLOAT)
            }),
            children: vec![],
        };

        let present_pass = RenderPass {
            color_attachments: vec![RenderPassAttachment {
                clear_value: Default::default(),
                source: RenderTarget::Window(window.handle())
            }],
            depth_attachment: None,
            children: vec![forward_pass],
        };

        let frame_graph = FrameGraph {
            present_pass,
        };

        window.init_swapchain(device, frame_graph)?;
        let handle = window.handle();
        self.data.windows.insert(window.handle().id()?, window);
        Ok(handle)
    }

    pub fn ctx(&self) -> EngineCtx {
        self.data.handle()
    }
}

impl ApplicationHandler for Engine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_options = self.default_window_settings.clone();
        match self.create_window(event_loop, &window_options) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create window : {}", err);
            }
        };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match self.data.windows.get_mut(&id) {
            None => { warn!("Failed to find corresponding windows with id {:?}", id) }
            Some(window) => {
                match window.window_event(event_loop, event) {
                    Ok(_) => {}
                    Err(err) => { error!("Event failed : {}", err) }
                };
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.data.windows.clear();
        self.data.instance = Resource::default();
    }
}