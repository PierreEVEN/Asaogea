use crate::application::gfx::device::{DeviceCtx, Fence};
use crate::application::gfx::instance::InstanceCtx;
use crate::application::gfx::surface::Surface;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use types::resource_handle::ResourceHandle;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Handle, InstanceV1_0, KhrSurfaceExtension, KhrSwapchainExtension};

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum QueueFlag {
    Graphic,
    Transfer,
    Compute,
    AsyncCompute,
    Present,
}

pub struct Queues {
    preferred: HashMap<QueueFlag, Arc<Queue>>,
    ctx: RefCell<Option<DeviceCtx>>,
}

impl Queues {
    pub fn preferred(&self) -> &HashMap<QueueFlag, Arc<Queue>> {
        &self.preferred
    }

    pub fn search(instance: InstanceCtx, physical_device: &vk::PhysicalDevice, surface: &Surface) -> Self {
        let properties = unsafe {
            instance.ptr().get_physical_device_queue_family_properties(*physical_device)
        };

        let mut queues = vec![];
        let mut queue_map = HashMap::new();

        for (index, prop) in properties.iter().enumerate() {
            let mut support_present = false;
            unsafe {
                if instance.ptr().get_physical_device_surface_support_khr(
                    *physical_device,
                    index as u32,
                    *surface.ptr()).unwrap() {
                    support_present = true;
                }
            }
            let queue = Arc::new(Queue {
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

    fn find_best_suited_queue(queues: &HashMap<usize, Arc<Queue>>, required: &vk::QueueFlags, require_present: bool, desired: &[vk::QueueFlags]) -> Option<usize> {
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
        for queue in self.preferred.values() {
            unsafe { *queue.queue.lock().unwrap() = ctx.device().get_device_queue(queue.family_index as u32, 0); }
        }
        self.ctx.replace(Some(ctx));
    }

    pub fn submit(&self, family: &QueueFlag, submit_infos: &[vk::SubmitInfo], fence: Option<ResourceHandle<Fence>>) {
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            let queue = self.preferred.get(family).unwrap_or_else(|| panic!("There is no {:?} queue available on this device !", family));
            let queue = queue.queue.lock().unwrap();
            unsafe {
                ctx.device().queue_submit(*queue, submit_infos, if let Some(fence) = fence {
                    fence.reset();
                    *fence.ptr()
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
            unsafe { ctx.device().queue_present_khr(*queue, present_infos) }
        } else {
            panic!("Queue have not been initialized for current device");
        }
    }

    pub fn find_queue(&self, flag: &QueueFlag) -> Option<&Arc<Queue>> {
        self.preferred.get(flag)
    }

    pub fn require_family_in_queues(requirements: Vec<QueueFlag>, queues: &[Arc<Queue>]) -> Option<&Arc<Queue>> {
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

pub struct Queue {
    family_index: usize,
    flags: vk::QueueFlags,
    queue: Mutex<vk::Queue>,
    support_present: bool,
}

impl Queue {
    pub fn index(&self) -> usize {
        self.family_index
    }


    pub fn flags(&self) -> &vk::QueueFlags {
        &self.flags
    }
    
    pub fn ptr(&self) -> &Mutex<vk::Queue> {
        &self.queue
    }
}
