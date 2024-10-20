use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use crate::application::window::CtxAppWindow;

const MAX_DESC_PER_TYPE: u32 = 1024u32;
const MAX_DESC_PER_POOL: u32 = 1024u32;

pub struct DescriptorPool {
    pool: Option<vk::DescriptorPool>,
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
            pool: Some(pool)
        })
    }

    pub fn ptr(&self) -> Result<&vk::DescriptorPool, Error> {
        self.pool.as_ref().ok_or(anyhow!("Descriptor pool is not valid"))
    }

    pub fn allocate(&self, ctx: &CtxAppWindow, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, Error> {
        let descriptor_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool.unwrap())
            .set_layouts(&[layout])
            .build();
        Ok(unsafe { ctx.engine().device()?.ptr().allocate_descriptor_sets(&descriptor_info)?[0] })
    }

    pub fn free(&self, ctx: &CtxAppWindow, set: vk::DescriptorSet) -> Result<(), Error> {
        unsafe { ctx.engine().device()?.ptr().free_descriptor_sets(self.pool.unwrap(), &[set]) }?;
        Ok(())
    }
    
    pub fn destroy(&mut self, ctx: &vulkanalia::Device) -> Result<(), Error> {
        unsafe { ctx.destroy_descriptor_pool(self.pool.take().unwrap(), None); }
        Ok(())
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        if self.pool.is_some() {
            panic!("Descriptor pool have not been destroyed !");
        }
    }
}