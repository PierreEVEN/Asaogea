use std::collections::HashSet;
use std::ops::Deref;
use anyhow::{anyhow, Error};
use tracing::{info, warn};
use vulkanalia::{vk};
use vulkanalia::vk::{DeviceV1_0, HasBuilder, InstanceV1_0, KhrSurfaceExtension, Queue};
use crate::application::gfx::command_buffer::CommandPool;
use crate::application::gfx::instance::{GfxConfig};
use crate::application::gfx::surface::Surface;
use crate::application::window::CtxAppWindow;

pub struct PhysicalDevice {
    physical_device: vk::PhysicalDevice,
    queue_family_indices: QueueFamilyIndices,
}

#[derive(Copy, Clone, Debug)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
    pub transfer: Option<u32>,
    pub compute: Option<u32>,
}

impl QueueFamilyIndices {
    fn get(ctx: &CtxAppWindow, physical_device: vk::PhysicalDevice) -> Result<Self, Error> {
        let properties = unsafe {
            ctx.engine().instance()?.ptr()?.get_physical_device_queue_family_properties(physical_device)
        };

        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        let transfer = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::TRANSFER))
            .map(|i| i as u32);

        let compute = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::COMPUTE))
            .map(|i| i as u32);

        let mut present = None;
        for (index, _) in properties.iter().enumerate() {
            unsafe {
                if ctx.engine().instance()?.ptr()?.get_physical_device_surface_support_khr(
                    physical_device,
                    index as u32,
                    *ctx.window.surface()?.read()?.ptr()?,
                )? {
                    present = Some(index as u32);
                    break;
                }
            }
        }

        let present = match present {
            None => { return Err(anyhow!("Failed to find present queue family.")) }
            Some(present) => { present }
        };


        if let Some(graphics) = graphics {
            Ok(Self { graphics, transfer, compute, present })
        } else {
            Err(anyhow!("Failed to find graphic queue family."))
        }
    }
}

#[derive(Clone, Debug)]
pub struct SwapchainSupport {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}
impl SwapchainSupport {
    pub fn get(ctx: &CtxAppWindow, surface: &Surface, physical_device: vk::PhysicalDevice) -> Result<Self, Error> {
        let surface = *surface.ptr()?;
        let instance = ctx.engine().instance()?;
        unsafe {
            
            Ok(Self {
                capabilities: instance.ptr()?.get_physical_device_surface_capabilities_khr(physical_device, surface)?,
                formats: instance.ptr()?.get_physical_device_surface_formats_khr(physical_device, surface)?,
                present_modes: instance.ptr()?.get_physical_device_surface_present_modes_khr(physical_device, surface)?,
            })
        }
    }
}

impl PhysicalDevice {
    pub fn new(ctx: &CtxAppWindow, config: &GfxConfig) -> Result<Self, Error> {
        unsafe {
            for physical_device in ctx.engine().instance()?.ptr()?.enumerate_physical_devices()? {
                let properties = ctx.engine().instance()?.ptr()?.get_physical_device_properties(physical_device);
                match Self::check_physical_device(ctx, physical_device, config) {
                    Ok(queue_family_indices) => {
                        info!("Selected physical device (`{}`).", properties.device_name);
                        return Ok(Self {
                            physical_device,
                            queue_family_indices,
                        });
                    }
                    Err(err) => {
                        warn!("Skipping physical device (`{}`): {}", properties.device_name, err);
                    }
                }
            }
        }
        Err(anyhow!("Failed to find suitable physical device."))
    }

    pub fn ptr(&self) -> &vk::PhysicalDevice {
        &self.physical_device
    }

    unsafe fn check_physical_device(ctx: &CtxAppWindow, physical_device: vk::PhysicalDevice, config: &GfxConfig) -> Result<QueueFamilyIndices, Error> {
        let properties = ctx.engine().instance()?.ptr()?.get_physical_device_properties(physical_device);
        if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
            return Err(anyhow!("Only discrete GPUs are supported."));
        }
        let _features = ctx.engine().instance()?.ptr()?.get_physical_device_features(physical_device);

        let extensions = ctx.engine().instance()?.ptr()?
            .enumerate_device_extension_properties(physical_device, None)?
            .iter()
            .map(|e| e.extension_name)
            .collect::<HashSet<_>>();

        let queue_family = QueueFamilyIndices::get(ctx, physical_device)?;
        let swapchain_support = SwapchainSupport::get(ctx, &*ctx.window.surface()?.read()?, physical_device)?;
        if swapchain_support.formats.is_empty() || swapchain_support.present_modes.is_empty() {
            return Err(anyhow!("Insufficient swapchain support."));
        }

        if config.required_extensions.iter().all(|e| extensions.contains(e)) {
            Ok(queue_family)
        } else {
            Err(anyhow!("Missing required device extensions."))
        }
    }

    pub fn queue_families_indices(&self) -> &QueueFamilyIndices {
        &self.queue_family_indices
    }
}

pub struct Device {
    physical_device: PhysicalDevice,
    device: Option<vulkanalia::Device>,
    graphics_queue: Queue,
    present_queue: Queue,
    command_pool: Option<CommandPool>,
}

impl Device {
    pub fn new(ctx: &CtxAppWindow, config: &GfxConfig) -> Result<Self, Error> {
        let physical_device = PhysicalDevice::new(ctx, config)?;

        let queue_priorities = &[1.0];
        let queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(physical_device.queue_family_indices.graphics)
            .queue_priorities(queue_priorities);

        let extensions = config.required_extensions
            .iter()
            .map(|n| n.as_ptr())
            .collect::<Vec<_>>();

        let features = vk::PhysicalDeviceFeatures::builder();

        let layers = if config.validation_layers {
            vec![crate::application::gfx::instance::VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };
        let queue_infos = &[queue_info];
        let info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(queue_infos)
            .enabled_layer_names(layers.as_slice())
            .enabled_extension_names(&extensions)
            .enabled_features(&features);
        let device = unsafe { ctx.engine().instance()?.ptr()?.create_device(physical_device.physical_device, &info, None)? };




        let graphics_queue = unsafe {
            device.get_device_queue(physical_device.queue_family_indices.graphics, 0)
        };
        let present_queue = unsafe {
            device.get_device_queue(physical_device.queue_family_indices.present, 0)
        };
        let command_pool = CommandPool::new(&device, physical_device.queue_families_indices())?;

        Ok(Self { physical_device, device: Some(device), graphics_queue, present_queue, command_pool: Some(command_pool) })
    }

    pub fn command_pool(&self) -> &CommandPool {
        self.command_pool.as_ref().expect("Command pool have been destroyed")
    }

    pub fn physical_device(&self) -> &PhysicalDevice {
        &self.physical_device
    }


    pub fn graphic_queue(&self) -> &Queue {
        &self.graphics_queue
    }

    pub fn present_queue(&self) -> &Queue {
        &self.present_queue
    }

    pub fn ptr(&self) -> &vulkanalia::Device {
        self.device.as_ref().expect("Device have not been initialized yet")
    }

    pub fn destroy(&mut self) -> Result<(), Error> {
        unsafe {
            if let Some(device) = self.device.take() {
                if let Some(command_pool) = &mut self.command_pool {
                    command_pool.destroy(&device);
                }
                self.command_pool = None;
                device.destroy_device(None)
            };
            Ok(())
        }
    }
}

impl Deref for Device {
    type Target = PhysicalDevice;

    fn deref(&self) -> &Self::Target {
        &self.physical_device
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if self.device.is_some() {
            panic!("Logical device have not been destroyed using Device::destroy()");
        }
    }
}
