use std::ops::Deref;
use crate::application::gfx::device::DeviceSharedData;
use crate::application::window::CtxAppWindow;
use anyhow::{anyhow, Error};
use std::sync::RwLock;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

const MAX_DESC_PER_TYPE: u32 = 1024u32;
const MAX_DESC_PER_POOL: u32 = 1024u32;

pub struct DescriptorPool {
    pool: Option<vk::DescriptorPool>,
    device: RwLock<Option<DeviceSharedData>>,
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

    pub fn init(&self, device_data: DeviceSharedData) {
        *self.device.write().unwrap() = Some(device_data);
    }

    pub fn ptr(&self) -> Result<&vk::DescriptorPool, Error> {
        self.pool.as_ref().ok_or(anyhow!("Descriptor pool is not valid"))
    }

    pub fn allocate(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, Error> {
        let descriptor_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool.unwrap())
            .set_layouts(&[layout])
            .build();
        Ok(unsafe { self.device.read().unwrap().as_ref().unwrap().device().allocate_descriptor_sets(&descriptor_info)?[0] })
    }

    pub fn free(&self, set: vk::DescriptorSet) -> Result<(), Error> {
        unsafe { self.device.read().unwrap().as_ref().unwrap().device().free_descriptor_sets(self.pool.unwrap(), &[set]) }?;
        Ok(())
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe { self.device.read().unwrap().as_ref().unwrap().device().destroy_descriptor_pool(self.pool.unwrap(), None); }
    }
}