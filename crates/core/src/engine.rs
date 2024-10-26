use std::collections::HashMap;
use std::time::Duration;
use anyhow::{Error};
use tracing::{error};
use vulkanalia::vk;
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{WindowAttributes, WindowId};
use types::resource_handle::{Resource, ResourceHandle};
use types::time_delta::TimeDelta;
use crate::application::gfx::frame_graph::frame_graph_definition::{FrameGraph, RenderPass, RenderPassAttachment, RenderTarget};
use crate::application::gfx::instance::{GfxConfig, Instance, InstanceCtx};
use crate::options::{Options, WindowOptions};
use crate::application::window::{AppWindow, WindowCtx};

pub struct Engine {
    windows: HashMap<WindowId, Resource<AppWindow>>,
    instance: Resource<Instance>,
    options: Options,
    self_ref: EngineCtx,

    delta_time: TimeDelta,
    current_frame: usize,
}

pub type EngineCtx = ResourceHandle<Engine>;

impl Engine {
    pub fn new(options: Options) -> Result<Resource<Self>, Error> {
        let mut config = GfxConfig {
            validation_layers: true,
            required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
        };
        let mut data = Resource::new(Self
        {
            windows: Default::default(),
            instance: Resource::default(),
            options,
            self_ref: Default::default(),
            delta_time: Default::default(),
            current_frame: 0,
        });
        data.self_ref = data.handle();
        data.instance = Instance::new(data.handle(), &mut config)?;
        Ok(data)
    }

    pub fn instance(&self) -> InstanceCtx {
        self.instance.handle()
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(event_loop.run_app(self)?)
    }

    pub fn create_window(&mut self, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<WindowCtx, Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();
        let mut window = AppWindow::new(self.self_ref.clone(), event_loop, options)?;
        let device = self.instance.get_or_create_device(window.handle());

        let forward_pass = RenderPass {
            color_attachments: vec![RenderPassAttachment {
                clear_value: Default::default(),
                source: RenderTarget::Internal(vk::Format::R16G16B16A16_SFLOAT)
            }],
            depth_attachment: Some(RenderPassAttachment {
                clear_value: Default::default(),
                source: RenderTarget::Internal(vk::Format::D32_SFLOAT),
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
        self.windows.insert(window.handle().id()?, window);
        Ok(handle)
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    pub fn render_frame(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        self.delta_time.next();
        // Draw all windows (sequentially, no need to parallelize this work for now)
        for window in self.windows.values_mut() {
            if let Err(err) = window.window_event(event_loop, event.clone()) { error!("Event failed : {}", err) }
        }

        // Move to next frame
        self.current_frame = (self.current_frame + 1) % self.options.rendering.image_count;

        // Request redraw for next frame
        if let Some(window) = self.windows.values().next() {
            window.ptr().unwrap().request_redraw();
        }
    }

    pub fn delta_time(&self) -> &Duration {
        self.delta_time.delta_time()
    }
}

impl ApplicationHandler for Engine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_options = self.options.clone();
        match self.create_window(event_loop, &window_options.main_window) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create window : {}", err);
            }
        };

        match self.create_window(event_loop, &window_options.main_window) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create secondary window : {}", err);
            }
        };

        match self.create_window(event_loop, &window_options.main_window) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create secondary window : {}", err);
            }
        };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                
                // Request redraw for next frame
                if let Some(window) = self.windows.values().next() {
                    window.ptr().unwrap().request_redraw();
                } else {
                    // Or exit if all windows have been closed
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                self.render_frame(event_loop, event);
            }
            _ => {
                if let Some(window) = self.windows.get_mut(&id) {
                    match window.window_event(event_loop, event) {
                        Ok(_) => {}
                        Err(err) => { error!("Event failed : {}", err) }
                    };
                }
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.windows.clear();
        self.instance = Resource::default();
    }
}