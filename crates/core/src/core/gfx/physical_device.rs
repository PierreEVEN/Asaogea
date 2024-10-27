use std::collections::HashSet;
use anyhow::{anyhow, Error};
use tracing::{info, warn};
use vulkanalia::vk;
use vulkanalia::vk::{InstanceV1_0, KhrSurfaceExtension};
use crate::core::gfx::instance::{GfxConfig, InstanceCtx};
use crate::core::gfx::queues::{QueueFlag, Queues};
use crate::core::gfx::surface::Surface;

pub struct PhysicalDevice {
    physical_device: vk::PhysicalDevice,
}

#[derive(Clone, Debug)]
pub struct SwapchainSupport {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}
impl SwapchainSupport {
    pub fn get(instance: &vulkanalia::Instance, surface: &vk::SurfaceKHR, physical_device: vk::PhysicalDevice) -> Result<Self, Error> {
        unsafe {
            Ok(Self {
                capabilities: instance.get_physical_device_surface_capabilities_khr(physical_device, *surface)?,
                formats: instance.get_physical_device_surface_formats_khr(physical_device, *surface)?,
                present_modes: instance.get_physical_device_surface_present_modes_khr(physical_device, *surface)?,
            })
        }
    }
}

impl PhysicalDevice {
    pub fn new(ctx: &InstanceCtx, surface: &Surface, config: &GfxConfig) -> Result<Self, Error> {
        unsafe {
            for physical_device in ctx.ptr().enumerate_physical_devices()? {
                let properties = ctx.ptr().get_physical_device_properties(physical_device);
                match Self::check_physical_device(ctx, surface, physical_device, config) {
                    Ok(_) => {
                        info!("Selected physical device (`{}`).", properties.device_name);
                        return Ok(Self {
                            physical_device
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

    unsafe fn check_physical_device(ctx: &InstanceCtx, surface: &Surface, physical_device: vk::PhysicalDevice, config: &GfxConfig) -> Result<(), Error> {
        let properties = ctx.ptr().get_physical_device_properties(physical_device);
        if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
            return Err(anyhow!("Only discrete GPUs are supported."));
        }
        let _features = ctx.ptr().get_physical_device_features(physical_device);

        let extensions = ctx.ptr()
            .enumerate_device_extension_properties(physical_device, None)?
            .iter()
            .map(|e| e.extension_name)
            .collect::<HashSet<_>>();


        let queues = Queues::search(ctx.clone(), &physical_device, surface);

        if queues.find_queue(&QueueFlag::Graphic).is_none() {
            return Err(anyhow!("There is no available graphic queue on this device"));
        }

        if queues.find_queue(&QueueFlag::Present).is_none() {
            return Err(anyhow!("There is no available present queue on this device"));
        }

        if queues.find_queue(&QueueFlag::Compute).is_none() {
            return Err(anyhow!("There is no available compute queue on this device"));
        }

        let swapchain_support = SwapchainSupport::get(ctx.ptr(), surface.ptr(), physical_device)?;
        if swapchain_support.formats.is_empty() || swapchain_support.present_modes.is_empty() {
            return Err(anyhow!("Insufficient swapchain support."));
        }

        if config.required_extensions.iter().all(|e| extensions.contains(e)) {
            Ok(())
        } else {
            Err(anyhow!("Missing required device extensions."))
        }
    }
}