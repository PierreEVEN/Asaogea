use std::collections::HashMap;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use anyhow::{anyhow, Error};
use tracing::{error, warn};
use vulkanalia::vk;
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{WindowAttributes, WindowId};
use types::rwslock::RwSLock;
use crate::application::gfx::device::Device;
use crate::application::gfx::instance::{GfxConfig, Instance};
use crate::options::WindowOptions;
use crate::application::window::{AppWindow, CtxAppWindow};

pub struct Engine {
    windows: HashMap<WindowId, RwSLock<AppWindow>>,
    gfx_instance: Option<RwSLock<Instance>>,
    gfx_device: Option<RwSLock<Device>>,
    default_window_settings: WindowOptions
}

pub struct CtxEngine<'a> {
    pub engine: &'a Engine,
}

impl Engine {
    pub fn new(default_window_settings: WindowOptions) -> Result<Self, Error> {
        Ok(Self {
            windows: Default::default(),
            gfx_instance: None,
            gfx_device: None,
            default_window_settings
        })
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(event_loop.run_app(self)?)
    }

    pub fn create_window(&mut self, event_loop: &ActiveEventLoop, options: &WindowOptions) -> Result<(), Error> {
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();

        let mut window = AppWindow::new(event_loop, options)?;
        let id = window.id()?;

        let mut config = GfxConfig {
            validation_layers: true,
            required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
        };

        let engine = self.ctx();
        let ctx_window = CtxAppWindow::new(&engine, &window);
        if self.gfx_instance.is_none() {
            self.gfx_instance = Some(RwSLock::new(
                Instance::new(&ctx_window, &mut config)?));
        }
        window.init(&self.ctx())?;

        let engine = self.ctx();
        let ctx_window = CtxAppWindow::new(&engine, &window);
        if self.gfx_device.is_none() {
            self.gfx_device = Some(RwSLock::new(Device::new(&ctx_window, &GfxConfig {
                validation_layers: true,
                required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
            })?));
        }

        let engine = self.ctx();
        let ctx_window = CtxAppWindow::new(&engine, &window);
        window.surface()?.write()?.create_or_recreate_swapchain(&ctx_window)?;

        self.windows.insert(id, RwSLock::new(window));

        Ok(())
    }

    pub fn device(&self) -> Result<RwLockReadGuard<Device>, Error> {
        self.gfx_device.as_ref()
            .ok_or(anyhow!("Device is not valid. Please create a window first"))?
            .read()
    }

    pub fn instance(&self) -> Result<RwLockReadGuard<Instance>, Error> {
        self.gfx_instance.as_ref()
            .ok_or(anyhow!("Instance is not valid. Please create a window first"))?
            .read()
    }

    pub fn instance_mut(&self) -> Result<RwLockWriteGuard<Instance>, Error> {
        self.gfx_instance.as_ref()
            .ok_or(anyhow!("Instance is not valid. Please create a window first"))?
            .write()
    }

    pub fn ctx(&self) -> CtxEngine {
        CtxEngine {
            engine: self
        }
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
        match self.windows.get(&id) {
            None => { warn!("Failed to find corresponding windows with id {:?}", id) }
            Some(window) => {
                let mut window = window.write().unwrap();
                match window.window_event(&self.ctx(), event_loop, event) {
                    Ok(_) => {}
                    Err(err) => { error!("Event failed : {}", err) }
                };
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        for window in self.windows.values() {
            if let Err(err) = window.write().unwrap().destroy(&CtxEngine { engine: self }) {
                panic!("Failed to destroy window : {}", err);
            }
        }
        self.windows.clear();
        if let Some(device) = self.gfx_device.take() {
            if let Err(err) = device.write().unwrap().destroy() {
                panic!("Failed to destroy device : {}", err);
            };
        }
        if let Some(instance) = self.gfx_instance.take() {
            if let Err(err) = instance.write().unwrap().destroy() {
                panic!("Failed to destroy instance : {}", err);
            }
        }
    }
}