use std::collections::HashMap;
use std::sync::{Arc, RwLockReadGuard, Weak};
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
    data: Arc<EngineData>,
    default_window_settings: WindowOptions,
}

pub struct EngineCtx(Weak<EngineData>);
impl EngineCtx {
    pub fn get(&self) -> Arc<EngineData> {
        self.0.upgrade().unwrap()
    }
}

pub struct EngineData {
    windows: HashMap<WindowId, AppWindow>,
    gfx_instance: Option<Instance>,
}

impl Engine {
    pub fn new(default_window_settings: WindowOptions) -> Result<Self, Error> {
        let mut config = GfxConfig {
            validation_layers: true,
            required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
        };
        Ok(Self {
            data: Arc::new(EngineData {
                windows: Default::default(),
                gfx_instance: Some(Instance::new(&mut config)?),
            }),
            default_window_settings,
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

        let mut window = AppWindow::new(self.ctx(), event_loop, options)?;
        let id = window.id()?;

        let engine = self.ctx();
        let ctx_window = CtxAppWindow::new(&engine, &window);
        if self.data.gfx_device.is_none() {
            self.data.gfx_device = Some(RwSLock::new(Device::new(&ctx_window, &GfxConfig {
                validation_layers: true,
                required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
            })?));
        }
        window.init_swapchain(self.gfx_device.as_ref().unwrap().read().unwrap().shared_data())?;

        self.windows.insert(id, RwSLock::new(window));

        Ok(())
    }

    pub fn ctx(&self) -> EngineCtx {
        EngineCtx(Arc::downgrade(&self.data))
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
        self.windows.clear();
        if let Some(device) = self.gfx_device.take() {
            if let Err(err) = device.write().unwrap().destroy() {
                panic!("Failed to destroy device : {}", err);
            };
        }
        self.gfx_instance = None;
    }
}