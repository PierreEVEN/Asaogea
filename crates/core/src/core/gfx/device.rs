use std::any::Any;
use std::collections::{HashMap};
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::{RwLock};
use anyhow::{anyhow, Error};
use tracing::{info, warn};
use vulkanalia::{vk};
use vulkanalia::vk::{DeviceV1_0, FenceCreateFlags, HasBuilder};
use winit::window::WindowId;
use types::resource_handle::{Resource, ResourceHandle};
use crate::core::gfx::command_buffer::CommandPool;
use crate::core::gfx::descriptor_pool::DescriptorPool;
use crate::core::gfx::frame_graph::frame_graph_instance::{RenderPassObject};
use crate::core::gfx::frame_graph::frame_graph_definition::{RenderPass, RenderPassName};
use crate::core::gfx::instance::{GfxConfig, InstanceCtx};
use crate::core::gfx::physical_device::PhysicalDevice;
use crate::core::gfx::queues::{QueueFlag, Queues};
use crate::core::gfx::surface::{SurfaceCtx};

#[derive(Default)]
pub struct Fence {
    fence: vk::Fence,
    ctx: Option<DeviceCtx>,
}

impl Fence {
    pub fn new(ctx: DeviceCtx) -> Resource<Self> {
        let create_infos = vk::FenceCreateInfo::builder().build();
        unsafe {
            Resource::new(Self {
                fence: ctx.device.create_fence(&create_infos, None).unwrap(),
                ctx: Some(ctx),
            })
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

pub type DeviceCtx = ResourceHandle<Device>;
pub struct Device {
    instance: InstanceCtx,
    physical_device: PhysicalDevice,
    device: vulkanalia::Device,
    allocator: MaybeUninit<vulkanalia_vma::Allocator>,
    descriptor_pool: MaybeUninit<DescriptorPool>,
    command_pool: HashMap<QueueFlag, Rc<CommandPool>>,
    queues: Queues,
    present_passes: RwLock<HashMap<WindowId, Resource<RenderPassObject>>>,
    render_passes: RwLock<HashMap<String, Resource<RenderPassObject>>>,
    self_ref: DeviceCtx,

    pending_kill_resources: RwLock<Vec<HashMap<WindowId, Vec<Box<dyn Any>>>>>,
}

impl Device {
    pub fn new(ctx: InstanceCtx, surface: &SurfaceCtx, config: &GfxConfig) -> Result<Resource<Self>, Error> {
        let physical_device = PhysicalDevice::new(&ctx, surface, config)?;
        let queues = Queues::search(ctx.clone(), physical_device.ptr(), surface);

        for (flag, queue) in queues.preferred() {
            info!("{:?} queue : index = {} : {:?}", flag, queue.index(), queue.flags());
        }


        let mut unique_queue_indices = HashMap::<usize, Vec<QueueFlag>>::new();
        for (flag, queue) in queues.preferred() {
            unique_queue_indices.entry(queue.index()).or_default().push(*flag);
        }

        let queue_priorities = &[1.0];
        let mut queue_info = vec![];
        for family_index in unique_queue_indices.keys() {
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
            vec![crate::core::gfx::instance::VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };
        let info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(queue_info.as_slice())
            .enabled_layer_names(layers.as_slice())
            .enabled_extension_names(&extensions)
            .enabled_features(&features);

        let device = unsafe { ctx.ptr().create_device(*physical_device.ptr(), &info, None)? };

        let infos = vulkanalia_vma::AllocatorOptions::new(ctx.ptr(), &device, *physical_device.ptr());
        let allocator = unsafe { vulkanalia_vma::Allocator::new(&infos) }?;
        let descriptor_pool = DescriptorPool::new(&device)?;

        let mut device = Resource::new(Self {
            physical_device,
            allocator: MaybeUninit::new(allocator),
            descriptor_pool: MaybeUninit::new(descriptor_pool),
            command_pool: HashMap::new(),
            queues,
            device,
            instance: ctx.clone(),
            present_passes: RwLock::new(HashMap::new()),
            render_passes: RwLock::new(HashMap::new()),
            self_ref: Default::default(),
            pending_kill_resources: Default::default(),
        });

        {
            let mut pending_kill_resources = device.pending_kill_resources.write().unwrap();
            pending_kill_resources.clear();
            for _ in 0..ctx.engine().params().rendering.image_count {
                pending_kill_resources.push(HashMap::default());
            }
        }
        device.self_ref = device.handle();
        device.descriptor_pool().init(device.handle());

        for (index, flags) in unique_queue_indices {
            let pool = Rc::new(CommandPool::new(device.handle(), index)?);
            for flag in flags {
                assert!(device.command_pool.insert(flag, pool.clone()).is_none())
            }
        }
        device.queues.initialize_for_device(device.handle());
        Ok(device)
    }

    pub fn instance(&self) -> InstanceCtx {
        self.instance.clone()
    }

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
        self.command_pool.get(flags).expect("Command pool is not available").as_ref()
    }

    pub fn queues(&self) -> &Queues {
        &self.queues
    }

    pub fn wait_idle(&self) {
        let mut unique_queues = HashMap::new();
        let mut locks = vec![];
        for queue in self.queues.preferred().values() {
            unique_queues.insert(queue.index(), queue.clone());
        }
        for queue in unique_queues.values() {
            locks.push(queue.ptr().lock().unwrap())
        }

        unsafe { self.device().device_wait_idle().unwrap(); }
        for image in &mut *self.pending_kill_resources.write().unwrap() {
            image.clear()
        }
        locks.clear();
    }

    pub fn declare_render_pass(&self, render_pass: RenderPass) -> Result<RenderPassName, Error> {
        assert!(!(render_pass.color_attachments.is_empty() && render_pass.depth_attachment.is_none()));
        match &render_pass.name {
            RenderPassName::Present(window) => {
                if self.present_passes.read().unwrap().contains_key(&window.id()?) {
                    return Err(anyhow!("Present pass for window {:?} already exists", window.id()));
                }
                self.present_passes.write().unwrap().insert(window.id()?, RenderPassObject::new(self.self_ref.clone(), &render_pass));
            }
            RenderPassName::Named(name) => {
                if self.render_passes.read().unwrap().contains_key(name) {
                    warn!("Render pass {name} already exists");
                    return Ok(render_pass.name);
                }
                self.render_passes.write().unwrap().insert(name.clone(), RenderPassObject::new(self.self_ref.clone(), &render_pass));
            }
        }
        Ok(render_pass.name)
    }


    pub fn find_render_pass(&self, render_pass: &RenderPassName) -> Result<ResourceHandle<RenderPassObject>, Error> {
        Ok(match &render_pass {
            RenderPassName::Present(window) => {
                self.present_passes.read().unwrap().get(&window.id()?).ok_or(anyhow!("Cannot find present pass for window {:?}", window.id()?))?.handle()
            }
            RenderPassName::Named(name) => {
                self.render_passes.read().unwrap().get(name).ok_or(anyhow!("Cannot find render pass for window {name}"))?.handle()
            }
        })
    }


    pub fn ptr(&self) -> &vulkanalia::Device {
        &self.device
    }

    pub fn queue_resource_cleanup<T: 'static>(&self, resource: Box<T>) {
        let mut per_image = self.pending_kill_resources.write().unwrap();
        let engine = self.instance.engine();
        per_image[engine.current_frame()].entry(engine.current_rendering_window()).or_default().push(resource);
    }

    pub fn free_resources_for_window(&self, window: WindowId, frame: usize) {
        let mut per_image = self.pending_kill_resources.write().unwrap();
        per_image[frame].entry(window).or_default().clear();
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.render_passes.write().unwrap().clear();
            self.present_passes.write().unwrap().clear();
            self.pending_kill_resources.write().unwrap().clear();
            self.command_pool.clear();
            self.descriptor_pool.assume_init_read();
            self.allocator.assume_init_read();
            self.device.destroy_device(None);
        }
    }
}