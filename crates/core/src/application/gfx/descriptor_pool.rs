use crate::application::gfx::device::DeviceCtx;
use anyhow::{anyhow, Error};
use std::sync::RwLock;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

const MAX_DESC_PER_TYPE: u32 = 1024u32;
const MAX_DESC_PER_POOL: u32 = 1024u32;

pub struct DescriptorPool {
    pool: Option<vk::DescriptorPool>,
    device: RwLock<Option<DeviceCtx>>,
}

impl DescriptorPool {
    pub fn new(device: &vulkanalia::Device) -> Result<Self, Error> {
        let pool_sizes = vec![
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::SAMPLER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::STORAGE_IMAGE).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::UNIFORM_TEXEL_BUFFER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::STORAGE_TEXEL_BUFFER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::STORAGE_BUFFER_DYNAMIC).descriptor_count(MAX_DESC_PER_TYPE).build(),
            vk::DescriptorPoolSize::builder().type_(vk::DescriptorType::INPUT_ATTACHMENT).descriptor_count(MAX_DESC_PER_TYPE).build(),
        ];

        let pool = unsafe {
            device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::builder()
                .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                .max_sets(MAX_DESC_PER_POOL)
                .pool_sizes(pool_sizes.as_slice())
                .build(), None)
        }?;

        Ok(Self {
            pool: Some(pool),
            device: Default::default(),
        })
    }

    pub fn init(&self, device_data: DeviceCtx) {
        *self.device.write().unwrap() = Some(device_data);
    }

    pub fn ptr(&self) -> Result<&vk::DescriptorPool, Error> {
        self.pool.as_ref().ok_or(anyhow!("Descriptor pool is not valid"))
    }

    pub fn allocate(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, Error> {
        let layouts = vec![layout];
        let descriptor_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool.unwrap())
            .set_layouts(layouts.as_slice())
            .build();
        Ok(unsafe { self.device.read().unwrap().as_ref().unwrap().get().device().allocate_descriptor_sets(&descriptor_info)?[0] })
    }

    pub fn free(&self, set: vk::DescriptorSet) -> Result<(), Error> {
        let set = vec![set];
        unsafe { self.device.read().unwrap().as_ref().unwrap().get().device().free_descriptor_sets(self.pool.unwrap(), set.as_slice()) }?;
        Ok(())
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe { self.device.read().unwrap().as_ref().unwrap().get().device().destroy_descriptor_pool(self.pool.unwrap(), None); }
    }
}