use std::collections::HashMap;
use std::ptr::{null};
use std::time::Duration;
use anyhow::{Error};
use tracing::{error};
use vulkanalia::vk;
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{WindowAttributes, WindowId};
use types::measure;
use types::profiler::Profiler;
use types::resource_handle::{Resource, ResourceHandle, ResourceHandleMut};
use types::time_delta::TimeDelta;
use crate::application::Application;
use crate::core::gfx::instance::{GfxConfig, Instance, InstanceCtx};
use crate::options::{Options, WindowOptions};
use crate::core::window::{AppWindow, WindowCtxMut};

static mut ENGINE: Option<ResourceHandleMut<Engine>> = None;


pub struct Engine {
    windows: HashMap<WindowId, Resource<AppWindow>>,
    instance: Resource<Instance>,
    options: Options,
    self_ref: EngineCtx,

    delta_time: TimeDelta,
    current_frame: usize,
    current_rendering_window: WindowId,
    event_loop: *const ActiveEventLoop,

    application: Box<dyn Application>,
}

pub type EngineCtx = ResourceHandle<Engine>;
pub type EngineCtxMut = ResourceHandleMut<Engine>;

impl Engine {
    pub fn new<T: 'static + Application + Default>(options: Options) -> Result<Resource<Self>, Error> {
        Profiler::init();
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
            current_rendering_window: WindowId::dummy(),
            event_loop: null(),
            application: Box::new(T::default()),
        });
        data.self_ref = data.handle();
        data.instance = Instance::new(data.handle(), &mut config)?;
        unsafe { ENGINE = Some(data.handle_mut()); }
        Ok(data)
    }

    pub fn get() -> &'static Self {
        unsafe { ENGINE.as_ref().unwrap() }
    }

    pub fn get_mut() -> &'static mut Self {
        unsafe { ENGINE.as_mut().unwrap() }
    }

    pub fn instance(&self) -> InstanceCtx {
        self.instance.handle()
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        Ok(event_loop.run_app(self)?)
    }

    pub fn params(&self) -> &Options {
        &self.options
    }

    pub fn create_window(&mut self, options: &WindowOptions) -> Result<WindowCtxMut, Error> {
        let record = Profiler::get().record(format!("Create window {}", options.name).as_str());
        let mut attributes = WindowAttributes::default();
        attributes.title = options.name.to_string();
        let mut window = AppWindow::new(self.self_ref.clone(), unsafe { self.event_loop.as_ref().unwrap() }, options)?;

        let mut created_device = false;
        if !self.instance.get_device().is_valid() {
            self.instance.create_device(window.handle());
            created_device = true;
        }
        window.init_swapchain();

        let mut handle = window.handle_mut();

        self.windows.insert(window.handle().id()?, window);

        if created_device {
            self.application.instantiate(&mut handle);
        }
        self.application.create_window(&mut handle);
        record.end();
        Ok(handle)
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    pub fn current_rendering_window(&self) -> WindowId {
        self.current_rendering_window
    }

    pub fn render_frame(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        Profiler::get().new_frame();
        let record = Profiler::get().record("Render frame");
        self.delta_time.next();
        // Draw all windows (sequentially, no need to parallelize this work for now)
        for (id, window) in &mut self.windows {
            self.current_rendering_window = *id;
            if let Err(err) = window.window_event(event_loop, event.clone()) { error!("Event failed : {}", err) }
        }

        // Move to next frame
        self.current_frame = (self.current_frame + 1) % self.options.rendering.image_count;

        // Request redraw for next frame
        if let Some(window) = self.windows.values().next() {
            let record = Profiler::get().record("Request redraw");
            window.ptr().unwrap().request_redraw();
            record.end();
        }
        record.end();
    }

    pub fn delta_time(&self) -> &Duration {
        self.delta_time.delta_time()
    }
}

impl ApplicationHandler for Engine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.event_loop = event_loop as *const ActiveEventLoop;
        let window_options = self.options.clone();
        match self.create_window(&window_options.main_window) {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to create window : {}", err);
            }
        };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let record = Profiler::get().record("Window event");
        self.event_loop = event_loop as *const ActiveEventLoop;
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
        record.end();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.windows.clear();
        self.instance = Resource::default();
        unsafe { ENGINE = None; }
    }
}