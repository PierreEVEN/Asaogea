use std::collections::HashSet;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use anyhow::{anyhow, Error};
use tracing::{info, warn};
use vulkanalia::{vk};
use vulkanalia::vk::{DeviceV1_0, HasBuilder, InstanceV1_0, KhrSurfaceExtension, Queue};
use crate::application::gfx::command_buffer::CommandPool;
use crate::application::gfx::descriptor_pool::DescriptorPool;
use crate::application::gfx::instance::{GfxConfig, InstanceCtx};

pub struct PhysicalDevice {
    physical_device: vk::PhysicalDevice,
    queue_family_indices: QueueFamilyIndices,
}

#[derive(Copy, Clone, Debug)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
    pub transfer: u32,
    pub compute: Option<u32>,
}

impl QueueFamilyIndices {
    fn get(ctx: &InstanceCtx, surface: &vk::SurfaceKHR,physical_device: vk::PhysicalDevice) -> Result<Self, Error> {
        let properties = unsafe {
            ctx.get().instance().get_physical_device_queue_family_properties(physical_device)
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
                if ctx.get().instance().get_physical_device_surface_support_khr(
                    physical_device,
                    index as u32,
                    *surface,
                )? {
                    present = Some(index as u32);
                    break;
                }
            }
        }

        let present = present.ok_or(anyhow!("Failed to find present queue family."))?;
        let transfer = transfer.ok_or(anyhow!("Failed to find transfer queue family."))?;

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
    pub fn new(ctx: &InstanceCtx, surface: &vk::SurfaceKHR, config: &GfxConfig) -> Result<Self, Error> {
        unsafe {
            for physical_device in ctx.get().instance().enumerate_physical_devices()? {
                let properties = ctx.get().instance().get_physical_device_properties(physical_device);
                match Self::check_physical_device(ctx, surface, physical_device, config) {
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

    unsafe fn check_physical_device(ctx: &InstanceCtx, surface: &vk::SurfaceKHR, physical_device: vk::PhysicalDevice, config: &GfxConfig) -> Result<QueueFamilyIndices, Error> {
        let properties = ctx.get().instance().get_physical_device_properties(physical_device);
        if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
            return Err(anyhow!("Only discrete GPUs are supported."));
        }
        let _features = ctx.get().instance().get_physical_device_features(physical_device);

        let extensions = ctx.get().instance()
            .enumerate_device_extension_properties(physical_device, None)?
            .iter()
            .map(|e| e.extension_name)
            .collect::<HashSet<_>>();

        let queue_family = QueueFamilyIndices::get(ctx, surface, physical_device)?;
        let swapchain_support = SwapchainSupport::get(ctx.get().instance(), &surface, physical_device)?;
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
    data: Arc<DeviceData>,
}

pub struct DeviceQueues {
    pub graphic: Queue,
    pub present_queue: Queue,
    pub transfer: Queue,
}

#[derive(Clone)]
pub struct DeviceCtx(Weak<DeviceData>);

pub struct DeviceData {
    allocator: MaybeUninit<vulkanalia_vma::Allocator>,
    descriptor_pool: MaybeUninit<DescriptorPool>,
    command_pool: MaybeUninit<CommandPool>,
    device: vulkanalia::Device,
    queues: DeviceQueues,
    instance: InstanceCtx,
    physical_device: PhysicalDevice
}

impl DeviceCtx {
    pub fn get(&self) -> Arc<DeviceData> {
        self.0.upgrade().unwrap()
    }
}

impl DeviceData {
    pub fn device(&self) -> &vulkanalia::Device {
        &self.device
    }

    pub fn physical_device(&self) -> &PhysicalDevice {
        &self.physical_device
    }

    pub fn allocator(&self) -> &vulkanalia_vma::Allocator {
        unsafe { self.allocator.assume_init_ref() }
    }

    pub fn descriptor_pool(&self) -> &DescriptorPool {
        unsafe { self.descriptor_pool.assume_init_ref() }
    }

    pub fn command_pool(&self) -> &CommandPool {
        unsafe { self.command_pool.assume_init_ref() }
    }

    pub fn queues(&self) -> &DeviceQueues {
        &self.queues
    }
}

impl Deref for DeviceData {
    type Target = InstanceCtx;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl Device {
    pub fn new(ctx: InstanceCtx, surface: &vk::SurfaceKHR, config: &GfxConfig) -> Result<Self, Error> {
        let physical_device = PhysicalDevice::new(&ctx, surface, config)?;

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

        let device = unsafe { ctx.get().instance().create_device(physical_device.physical_device, &info, None)? };


        let graphics_queue = unsafe {
            device.get_device_queue(physical_device.queue_family_indices.graphics, 0)
        };
        let present_queue = unsafe {
            device.get_device_queue(physical_device.queue_family_indices.present, 0)
        };
        let transfer_queue = unsafe {
            device.get_device_queue(physical_device.queue_family_indices.transfer, 0)
        };
        let command_pool = CommandPool::new(&device, physical_device.queue_families_indices())?;

        let instance_data = ctx.get();
        let infos = vulkanalia_vma::AllocatorOptions::new(instance_data.instance(), &device, *physical_device.ptr());
        let allocator = unsafe { vulkanalia_vma::Allocator::new(&infos) }?;
        let descriptor_pool = DescriptorPool::new(&device)?;

        let shared_data = Arc::new(DeviceData {
            physical_device,
            allocator: MaybeUninit::new(allocator),
            descriptor_pool: MaybeUninit::new(descriptor_pool),
            command_pool: MaybeUninit::new(command_pool),
            device,
            queues: DeviceQueues { graphic: graphics_queue, present_queue, transfer: transfer_queue },
            instance: ctx.clone(),
        });
        shared_data.descriptor_pool().init(DeviceCtx(Arc::downgrade(&shared_data)));
        shared_data.command_pool().init(DeviceCtx(Arc::downgrade(&shared_data)));

        Ok(Self { data: shared_data })
    }

    pub fn ptr(&self) -> &vulkanalia::Device {
        &self.data.device
    }

    pub fn ctx(&self) -> DeviceCtx {
        DeviceCtx(Arc::downgrade(&self.data))
    }
    
    pub fn destroy(&mut self) -> Result<(), Error> {
        unsafe {
            self.data.command_pool.assume_init_read();
            self.data.descriptor_pool.assume_init_read();
            self.data.allocator.assume_init_read();
            self.data.device.destroy_device(None);
        };
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {}
}
