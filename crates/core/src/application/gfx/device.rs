use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use anyhow::{anyhow, Error};
use imgui::sys::igDebugNodeInputTextState;
use tracing::{info, warn};
use tracing::dispatcher::get_default;
use vulkanalia::{vk, VkResult};
use vulkanalia::vk::{DeviceV1_0, FenceCreateFlags, FenceCreateInfo, Handle, HasBuilder, InstanceV1_0, KhrSurfaceExtension, KhrSwapchainExtension, PresentInfoKHR, Queue, SubmitInfo, SuccessCode};
use crate::application::gfx::command_buffer::CommandPool;
use crate::application::gfx::descriptor_pool::DescriptorPool;
use crate::application::gfx::instance::{GfxConfig, InstanceCtx};
use crate::application::gfx::surface::Surface;

pub struct PhysicalDevice {
    physical_device: vk::PhysicalDevice,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum QueueFlag {
    Graphic,
    Transfer,
    Compute,
    AsyncCompute,
    Present,
}

pub struct Queues {
    queues: Vec<Arc<SingleQueue>>,
    preferred: HashMap<QueueFlag, Arc<SingleQueue>>,
    queue_map: HashMap<QueueFlag, Vec<Arc<SingleQueue>>>,
    ctx: RefCell<Option<DeviceCtx>>,
}

impl Queues {
    pub fn search(instance: InstanceCtx, physical_device: &vk::PhysicalDevice, surface: &Surface) -> Self {
        let properties = unsafe {
            instance.get().instance().get_physical_device_queue_family_properties(*physical_device)
        };

        let mut queues = vec![];
        let mut queue_map = HashMap::new();

        for (index, prop) in properties.iter().enumerate() {
            let mut support_present = false;
            unsafe {
                if instance.get().instance().get_physical_device_surface_support_khr(
                    *physical_device,
                    index as u32,
                    *surface.ptr()).unwrap() {
                    support_present = true;
                }
            }
            let queue = Arc::new(SingleQueue {
                family_index: index,
                flags: prop.queue_flags,
                queue: Mutex::new(Default::default()),
                support_present,
            });
            if support_present {
                queue_map.entry(QueueFlag::Present).or_insert(vec![]).push(queue.clone());
            }

            if prop.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                queue_map.entry(QueueFlag::Graphic).or_insert(vec![]).push(queue.clone());
            }
            if prop.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                queue_map.entry(QueueFlag::Transfer).or_insert(vec![]).push(queue.clone());
            }
            if prop.queue_flags.contains(vk::QueueFlags::COMPUTE) {
                queue_map.entry(QueueFlag::Compute).or_insert(vec![]).push(queue.clone());
            }

            queues.push(queue);
        }

        let mut queues_object = Self {
            preferred: HashMap::new(),
            queues: queues.clone(),
            queue_map: Default::default(),
            ctx: RefCell::default(),
        };

        fn remove_queue(queue: &Arc<SingleQueue>, mut queues: Vec<Arc<SingleQueue>>) -> Vec<Arc<SingleQueue>> {
            for (i, q) in queues.iter().enumerate() {
                if q.family_index == queue.family_index {
                    queues.remove(i);
                    break;
                }
            }
            queues
        }

        todo!();
        // Pick graphic queue
        if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic, QueueFlag::Present, QueueFlag::Compute], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        } else if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic, QueueFlag::Present], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        } else if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        };


        // Pick async compute queue
        if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic, QueueFlag::Present, QueueFlag::Compute], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        } else if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic, QueueFlag::Present], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        } else if let Some(graphic) = Self::require_family_in_queues(vec![QueueFlag::Graphic], &queues) {
            queues_object.preferred.insert(QueueFlag::Graphic, graphic.clone());
        };

        queues_object
    }

    pub fn fetch_from_device(&self, ctx: DeviceCtx) {
        self.ctx.replace(Some(ctx));
    }

    pub fn submit(&self, family: &QueueFlag, submit_infos: &[vk::SubmitInfo], fence: Option<&Fence>) {
        let res = self.queue_map.get(family).unwrap_or_else(|| panic!("There is no {:?} queue available on this device !", family));
        let queue = res[0].queue.lock().unwrap();
        if let Some(fence) = fence { fence.reset() };
        unsafe { self.ctx.borrow().as_ref().unwrap().get().device.queue_submit(*queue, submit_infos, if let Some(fence) = fence { fence.fence } else { vk::Fence::null() }).expect("Failed to submit queue"); }
    }

    pub fn present(&self, present_infos: &vk::PresentInfoKHR) -> Result<vk::SuccessCode, vk::ErrorCode> {
        let res = self.queue_map.get(&QueueFlag::Present).unwrap_or_else(|| panic!("There is no present queue available on this device !"));
        let queue = res[0].queue.lock().unwrap();
        unsafe { Ok(self.ctx.borrow().as_ref().unwrap().get().device.queue_present_khr(*queue, present_infos)?) }
    }

    pub fn count(&self, family: &QueueFlag) -> usize {
        match self.queue_map.get(family) {
            None => { 0 }
            Some(f) => { f.len() }
        }
    }


    fn require_family(&self, requirements: Vec<QueueFlag>) -> Option<&Arc<SingleQueue>> {
        Self::require_family_in_queues(requirements, &self.queues)
    }

    pub fn require_family_in_queues(requirements: Vec<QueueFlag>, queues: &Vec<Arc<SingleQueue>>) -> Option<&Arc<SingleQueue>> {
        let mut flags = vk::QueueFlags::empty();
        let mut require_present = false;
        for requirement in requirements {
            match requirement {
                QueueFlag::Graphic => { flags |= vk::QueueFlags::GRAPHICS }
                QueueFlag::Transfer => { flags |= vk::QueueFlags::TRANSFER }
                QueueFlag::Compute => { flags |= vk::QueueFlags::COMPUTE }
                QueueFlag::Present => { require_present = true }
                QueueFlag::AsyncCompute => { flags |= vk::QueueFlags::COMPUTE }
            }
        }

        for queue in queues {
            if queue.flags & flags != vk::QueueFlags::empty() && (!require_present || queue.support_present) {
                return Some(queue);
            }
        }
        None
    }
}

pub struct Fence {
    fence: vk::Fence,
    ctx: DeviceCtx,
}

impl Fence {
    pub fn new(ctx: DeviceCtx) -> Self {
        let create_infos = vk::FenceCreateInfo::builder().build();
        unsafe {
            Self {
                fence: ctx.get().device.create_fence(&create_infos, None).unwrap(),
                ctx,
            }
        }
    }

    pub fn new_signaled(ctx: DeviceCtx) -> Self {
        let create_infos = vk::FenceCreateInfo::builder().flags(FenceCreateFlags::SIGNALED).build();
        unsafe {
            Self {
                fence: ctx.get().device.create_fence(&create_infos, None).unwrap(),
                ctx,
            }
        }
    }

    pub fn ptr(&self) -> &vk::Fence {
        &self.fence
    }

    pub fn reset(&self) {
        unsafe { self.ctx.get().device.reset_fences(&[self.fence]).unwrap() }
    }

    pub fn wait(&self) {
        unsafe { self.ctx.get().device.wait_for_fences(&[self.fence], true, u64::MAX).unwrap(); }
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe { self.ctx.get().device.destroy_fence(self.fence, None) }
    }
}

pub struct SingleQueue {
    family_index: usize,
    flags: vk::QueueFlags,
    queue: Mutex<vk::Queue>,
    support_present: bool,
}

impl SingleQueue {
    pub fn index(&self) -> usize {
        self.family_index
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
    pub fn new(ctx: &InstanceCtx, surface: &Surface, config: &GfxConfig) -> Result<Self, Error> {
        unsafe {
            for physical_device in ctx.get().instance().enumerate_physical_devices()? {
                let properties = ctx.get().instance().get_physical_device_properties(physical_device);
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


        let queues = Queues::search(ctx.clone(), &physical_device, surface);

        if queues.count(&QueueFlag::Graphic) == 0 {
            return Err(anyhow!("There is no available graphic queue on this device"));
        }

        if queues.count(&QueueFlag::Present) == 0 {
            return Err(anyhow!("There is no available present queue on this device"));
        }

        if queues.count(&QueueFlag::Compute) == 0 {
            return Err(anyhow!("There is no available compute queue on this device"));
        }

        let swapchain_support = SwapchainSupport::get(ctx.get().instance(), surface.ptr(), physical_device)?;
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

pub struct Device {
    data: Arc<DeviceData>,
}

#[derive(Clone)]
pub struct DeviceCtx(Weak<DeviceData>);

pub struct DeviceData {
    allocator: MaybeUninit<vulkanalia_vma::Allocator>,
    descriptor_pool: MaybeUninit<DescriptorPool>,
    command_pool: MaybeUninit<CommandPool>,
    device: vulkanalia::Device,
    queues: Queues,
    instance: InstanceCtx,
    physical_device: PhysicalDevice,
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

    pub fn queues(&self) -> &Queues {
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
    pub fn new(ctx: InstanceCtx, surface: &Surface, config: &GfxConfig) -> Result<Self, Error> {
        let physical_device = PhysicalDevice::new(&ctx, surface, config)?;

        let queues = Queues::search(ctx.clone(), &physical_device.physical_device, surface);
        let graphic_queue_family = queues.require_family(vec![QueueFlag::Compute, QueueFlag::Graphic, QueueFlag::Present]).expect("failed to find required queue");

        let queue_priorities = &[1.0];
        let queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(graphic_queue_family.family_index as u32)
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

        let command_pool = CommandPool::new(&device, graphic_queue_family.family_index)?;

        let instance_data = ctx.get();
        let infos = vulkanalia_vma::AllocatorOptions::new(instance_data.instance(), &device, *physical_device.ptr());
        let allocator = unsafe { vulkanalia_vma::Allocator::new(&infos) }?;
        let descriptor_pool = DescriptorPool::new(&device)?;

        let shared_data = Arc::new(DeviceData {
            physical_device,
            allocator: MaybeUninit::new(allocator),
            descriptor_pool: MaybeUninit::new(descriptor_pool),
            command_pool: MaybeUninit::new(command_pool),
            queues,
            device,
            instance: ctx.clone(),
        });
        shared_data.descriptor_pool().init(DeviceCtx(Arc::downgrade(&shared_data)));
        shared_data.command_pool().init(DeviceCtx(Arc::downgrade(&shared_data)));

        let device = Self { data: shared_data };
        device.data.queues.fetch_from_device(device.ctx());
        Ok(device)
    }

    pub fn ptr(&self) -> &vulkanalia::Device {
        &self.data.device
    }

    pub fn ctx(&self) -> DeviceCtx {
        DeviceCtx(Arc::downgrade(&self.data))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.data.command_pool.assume_init_read();
            self.data.descriptor_pool.assume_init_read();
            self.data.allocator.assume_init_read();
            self.data.device.destroy_device(None);
        }
    }
}
