use std::cell::{RefCell};
use std::collections::{HashMap, HashSet};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock};
use anyhow::{anyhow, Error};
use tracing::{info, warn};
use vulkanalia::{vk};
use vulkanalia::vk::{DeviceV1_0, FenceCreateFlags, Handle, HasBuilder, InstanceV1_0, KhrSurfaceExtension, KhrSwapchainExtension};
use types::resource_handle::{Resource, ResourceHandle, ResourceHandleMut};
use crate::application::gfx::command_buffer::CommandPool;
use crate::application::gfx::descriptor_pool::DescriptorPool;
use crate::application::gfx::frame_graph::frame_graph::{RenderPassObject};
use crate::application::gfx::frame_graph::frame_graph_definition::RenderPass;
use crate::application::gfx::instance::{GfxConfig, InstanceCtx};
use crate::application::gfx::surface::{Surface, SurfaceCtx};

pub struct PhysicalDevice {
    physical_device: vk::PhysicalDevice,
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum QueueFlag {
    Graphic,
    Transfer,
    Compute,
    AsyncCompute,
    Present,
}

pub struct Queues {
    preferred: HashMap<QueueFlag, Arc<SingleQueue>>,
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
            queue_map.insert(index, queue.clone());
            queues.push(queue);
        }


        let stored_queue_map = queue_map.clone();

        // Find fallback queues for transfer and compute
        let compute_queue = Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::COMPUTE, false, &[]);
        let mut transfer_queue = Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::TRANSFER, false, &[]);

        // Find graphic queue (ideally with present capability which should always be the case)
        let graphic_queue = Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::GRAPHICS, false, &[vk::QueueFlags::COMPUTE | vk::QueueFlags::TRANSFER]);

        // Find present queue that is not a dedicated compute queue ideally
        let present_queue = if let Some(graphic_queue) = graphic_queue {
            let mut no_graphic_queue_map = queue_map.clone();
            no_graphic_queue_map.remove(&graphic_queue);
            //if (prefer_dedicated_present_over_compute_queue) { @TODO
            if let Some(compute_queue) = compute_queue {
                no_graphic_queue_map.remove(&compute_queue);
            }
            //}
            if let Some(present_queue) = Self::find_best_suited_queue(&no_graphic_queue_map, &vk::QueueFlags::empty(), true, &[]) {
                Some(present_queue)
            } else {
                let mut no_graphic_queue_map = queue_map.clone();
                no_graphic_queue_map.remove(&graphic_queue);
                Self::find_best_suited_queue(&no_graphic_queue_map, &vk::QueueFlags::empty(), true, &[])
            }
        } else {
            Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::empty(), true, &[])
        };

        if let Some(graphic) = graphic_queue { queue_map.remove(&graphic); }
        if let Some(present) = present_queue { queue_map.remove(&present); }

        // Search dedicated async compute queue
        let async_compute_queue = Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::COMPUTE, false, &[]);
        if let Some(compute) = async_compute_queue { queue_map.remove(&compute); }

        // Search dedicated transfer queue
        if let Some(dedicated_transfer_queue) = Self::find_best_suited_queue(&queue_map, &vk::QueueFlags::TRANSFER, false, &[]) {
            queue_map.remove(&dedicated_transfer_queue);
            transfer_queue = Some(dedicated_transfer_queue);
        }

        let mut preferred = HashMap::new();

        if let Some(graphic) = graphic_queue { preferred.insert(QueueFlag::Graphic, stored_queue_map.get(&graphic).unwrap().clone()); }
        if let Some(compute) = compute_queue { preferred.insert(QueueFlag::Compute, stored_queue_map.get(&compute).unwrap().clone()); }
        if let Some(async_compute) = async_compute_queue { preferred.insert(QueueFlag::AsyncCompute, stored_queue_map.get(&async_compute).unwrap().clone()); }
        if let Some(transfer) = transfer_queue { preferred.insert(QueueFlag::Transfer, stored_queue_map.get(&transfer).unwrap().clone()); }
        if let Some(present) = present_queue { preferred.insert(QueueFlag::Present, stored_queue_map.get(&present).unwrap().clone()); }


        Self {
            preferred,
            ctx: RefCell::default(),
        }
    }

    fn find_best_suited_queue(queues: &HashMap<usize, Arc<SingleQueue>>, required: &vk::QueueFlags, require_present: bool, desired: &[vk::QueueFlags]) -> Option<usize> {
        let mut high_score = 0;
        let mut best_queue = None;
        for (index, queue) in queues {
            if require_present && !queue.support_present { continue; }
            if !queue.flags.contains(*required) { continue; }
            let mut score = 0;
            best_queue = Some(*index);
            let max_value = desired.len();
            for (power, flag) in desired.iter().enumerate() {
                if queue.flags.contains(*flag) {
                    score += max_value - power;
                }
            }
            if score > high_score {
                high_score = score;
                best_queue = Some(*index);
            }
        }
        best_queue
    }

    pub fn initialize_for_device(&self, ctx: DeviceCtx) {
        for (_, queue) in &self.preferred {
            unsafe { *queue.queue.lock().unwrap() = ctx.device.get_device_queue(queue.family_index as u32, 0); }
        }
        self.ctx.replace(Some(ctx));
    }

    pub fn submit(&self, family: &QueueFlag, submit_infos: &[vk::SubmitInfo], fence: Option<&Fence>) {
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            let queue = self.preferred.get(family).unwrap_or_else(|| panic!("There is no {:?} queue available on this device !", family));
            let queue = queue.queue.lock().unwrap();
            unsafe {
                ctx.device.queue_submit(*queue, submit_infos, if let Some(fence) = fence {
                    fence.reset();
                    fence.fence
                } else { vk::Fence::null() }).expect("Failed to submit queue");
            }
        } else {
            panic!("Queue have not been initialized for current device");
        }
    }

    pub fn present(&self, present_infos: &vk::PresentInfoKHR) -> Result<vk::SuccessCode, vk::ErrorCode> {
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            let queue = self.preferred.get(&QueueFlag::Present).unwrap_or_else(|| panic!("There is no present queue available on this device !"));
            let queue = queue.queue.lock().unwrap();
            unsafe { ctx.device.queue_present_khr(*queue, present_infos) }
        } else {
            panic!("Queue have not been initialized for current device");
        }
    }

    pub fn find_queue(&self, flag: &QueueFlag) -> Option<&Arc<SingleQueue>> {
        self.preferred.get(&flag)
    }

    pub fn require_family_in_queues(requirements: Vec<QueueFlag>, queues: &[Arc<SingleQueue>]) -> Option<&Arc<SingleQueue>> {
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

        queues.iter().find(|&queue| queue.flags & flags != vk::QueueFlags::empty() && (!require_present || queue.support_present))
    }
}

#[derive(Default)]
pub struct Fence {
    fence: vk::Fence,
    ctx: Option<DeviceCtx>,
}

impl Fence {
    pub fn new(ctx: DeviceCtx) -> Self {
        let create_infos = vk::FenceCreateInfo::builder().build();
        unsafe {
            Self {
                fence: ctx.device.create_fence(&create_infos, None).unwrap(),
                ctx: Some(ctx),
            }
        }
    }

    pub fn new_signaled(ctx: DeviceCtx) -> Self {
        let create_infos = vk::FenceCreateInfo::builder().flags(FenceCreateFlags::SIGNALED).build();
        unsafe {
            Self {
                fence: ctx.device.create_fence(&create_infos, None).unwrap(),
                ctx: Some(ctx),
            }
        }
    }

    pub fn ptr(&self) -> &vk::Fence {
        &self.fence
    }

    pub fn reset(&self) {
        let fences = vec![self.fence];
        unsafe { self.ctx.as_ref().unwrap().device.reset_fences(fences.as_slice()).unwrap() }
    }

    pub fn is_valid(&self) -> bool {
        self.ctx.is_some()
    }

    pub fn wait(&self) {
        let fences = vec![self.fence];
        unsafe { self.ctx.as_ref().unwrap().device.wait_for_fences(fences.as_slice(), true, u64::MAX).unwrap(); }
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        if let Some(ctx) = self.ctx.as_ref() {
            unsafe { ctx.device.destroy_fence(self.fence, None) }
        }
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

        if queues.find_queue(&QueueFlag::Graphic).is_none() {
            return Err(anyhow!("There is no available graphic queue on this device"));
        }

        if queues.find_queue(&QueueFlag::Present).is_none() {
            return Err(anyhow!("There is no available present queue on this device"));
        }

        if queues.find_queue(&QueueFlag::Compute).is_none() {
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
    data: Resource<DeviceData>,
}

pub type DeviceCtx = ResourceHandle<DeviceData>;

pub struct DeviceData {
    instance: InstanceCtx,
    physical_device: PhysicalDevice,
    device: vulkanalia::Device,
    allocator: MaybeUninit<vulkanalia_vma::Allocator>,
    descriptor_pool: MaybeUninit<DescriptorPool>,
    command_pool: MaybeUninit<HashMap<QueueFlag, Arc<CommandPool>>>,
    queues: Queues,
    render_passes: RwLock<Vec<Resource<RenderPassObject>>>,

    self_ref: DeviceCtx
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

    pub fn command_pool(&self, flags: &QueueFlag) -> &CommandPool {
        unsafe { self.command_pool.assume_init_ref().get(flags).expect("Command pool is not available").as_ref() }
    }

    pub fn queues(&self) -> &Queues {
        &self.queues
    }

    pub fn wait_idle(&self) {
        let mut unique_queues = HashMap::new();
        let mut locks = vec![];
        for (_, queue) in &self.queues.preferred {
            unique_queues.insert(queue.family_index, queue.clone());
        }
        for (_, queue) in &unique_queues {
            locks.push(queue.queue.lock().unwrap())
        }

        unsafe { self.device().device_wait_idle().unwrap(); }
        locks.clear();
    }

    pub fn find_or_create_render_pass(&self, base: &RenderPass) -> ResourceHandleMut<RenderPassObject> {
        let render_pass = RenderPassObject::new(self.self_ref.clone(), base);
        let handle = render_pass.handle_mut();
        self.render_passes.write().unwrap().push(render_pass);
        handle
    }
}

impl Deref for DeviceData {
    type Target = InstanceCtx;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl Device {
    pub fn new(ctx: InstanceCtx, surface: &SurfaceCtx, config: &GfxConfig) -> Result<Self, Error> {
        let physical_device = PhysicalDevice::new(&ctx, surface, config)?;
        let queues = Queues::search(ctx.clone(), &physical_device.physical_device, surface);

        for (flag, queue) in &queues.preferred {
            info!("{:?} queue : index = {} : {:?}", flag, queue.index(), queue.flags);
        }


        let mut unique_queue_indices = HashMap::<usize, Vec<QueueFlag>>::new();
        for (flag, queue) in &queues.preferred {
            unique_queue_indices.entry(queue.family_index).or_default().push(*flag);
        }

        let queue_priorities = &[1.0];
        let mut queue_info = vec![];
        for (family_index, _) in &unique_queue_indices {
            queue_info.push(vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*family_index as u32)
                .queue_priorities(queue_priorities));
        }

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
        let info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(queue_info.as_slice())
            .enabled_layer_names(layers.as_slice())
            .enabled_extension_names(&extensions)
            .enabled_features(&features);

        let device = unsafe { ctx.get().instance().create_device(physical_device.physical_device, &info, None)? };

        let instance_data = ctx.get();
        let infos = vulkanalia_vma::AllocatorOptions::new(instance_data.instance(), &device, *physical_device.ptr());
        let allocator = unsafe { vulkanalia_vma::Allocator::new(&infos) }?;
        let descriptor_pool = DescriptorPool::new(&device)?;

        let mut shared_data = Resource::new(DeviceData {
            physical_device,
            allocator: MaybeUninit::new(allocator),
            descriptor_pool: MaybeUninit::new(descriptor_pool),
            command_pool: MaybeUninit::new(HashMap::new()),
            queues,
            device,
            instance: ctx.clone(),
            render_passes: RwLock::new(vec![]),
            self_ref: Default::default(),
        });
        shared_data.self_ref = shared_data.handle();
        shared_data.descriptor_pool().init(shared_data.handle());

        let device = Self { data: shared_data };

        let mut command_pool_list = vec![];
        for (index, flags) in unique_queue_indices {
            let pool = Arc::new(CommandPool::new(device.ctx(), index)?);
            unsafe {
                for flag in flags {
                    let pool_mut = device.data.command_pool.assume_init_ref() as *const HashMap<QueueFlag, Arc<CommandPool>> as *mut HashMap<QueueFlag, Arc<CommandPool>>;
                    assert!(pool_mut.as_mut().unwrap().insert(flag, pool.clone()).is_none())
                }
            }
            command_pool_list.push(pool);
        }
        device.data.queues.initialize_for_device(device.ctx());
        Ok(device)
    }

    pub fn ptr(&self) -> &vulkanalia::Device {
        &self.data.device
    }

    pub fn ctx(&self) -> DeviceCtx {
        self.data.handle()
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