use anyhow::Error;
use tracing::error;
use vulkanalia::vk;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::instance::{GfxConfig, Instance};
use crate::options::{Options, WindowOptions};

#[derive(Default)]
pub struct AppWindow {
    window: Option<Window>,
    instance: Option<Instance>,
    options: Options
}

impl AppWindow {}

impl AppWindow {
    pub fn run(&mut self) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(event_loop.run_app(self)?)
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<(), Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();


        let window = event_loop.create_window(attributes)?;
        self.instance = Some(Instance::new(&mut GfxConfig {
            validation_layers: true,
            required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
        }, &window)?);
        self.window = Some(window);
        Ok(())
    }
}

impl ApplicationHandler for AppWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_options = self.options.windows.clone();
        match self.create_window(event_loop, &window_options) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create window : {}", err);
            }
        };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}