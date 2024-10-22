use std::slice;
use std::sync::Weak;
use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{CopyDescriptorSet, DescriptorImageInfo, DescriptorSetLayout, DeviceV1_0, HasBuilder};
use crate::application::gfx::descriptor_pool::DescriptorPool;
use crate::application::gfx::device::DeviceCtx;

pub struct DescriptorSets {
    desc_set: Option<vk::DescriptorSet>,
    ctx: DeviceCtx
}
pub enum ShaderInstanceBinding {
    Sampler(vk::Sampler),
    SampledImage(vk::ImageView, vk::ImageLayout),
}

impl DescriptorSets {
    pub fn new(ctx: DeviceCtx, layout: &DescriptorSetLayout) -> Result<Self, Error> {
        let desc_set = ctx.get().descriptor_pool().allocate(*layout)?;
        Ok(Self {
            desc_set: Some(desc_set),
            ctx,
        })
    }

    pub fn update(&mut self, bindings: Vec<(ShaderInstanceBinding, u32)>) -> Result<(), Error> {
        let mut desc_images = Vec::new();

        let mut write_desc_set = Vec::new();
        for (desc_set, binding) in &bindings {
            let write_set = match &desc_set {
                ShaderInstanceBinding::Sampler(sampler) => {
                    desc_images.push(
                        DescriptorImageInfo::builder()
                            .sampler(*sampler).build());
                    vk::WriteDescriptorSet::builder()
                        .descriptor_type(vk::DescriptorType::SAMPLER)
                        .image_info(slice::from_ref(&desc_images[desc_images.len() - 1]))
                }
                ShaderInstanceBinding::SampledImage(sampled_image, layout) => {
                    desc_images.push(
                        DescriptorImageInfo::builder()
                            .image_view(*sampled_image)
                            .image_layout(*layout).build());
                    vk::WriteDescriptorSet::builder()
                        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                        .image_info(slice::from_ref(&desc_images[desc_images.len() - 1]))
                }
            }
                .dst_set(self.desc_set.unwrap())
                .dst_binding(*binding)
                .build();
            write_desc_set.push(write_set);
        }

        let copies = Vec::<CopyDescriptorSet>::new();
        
        unsafe { self.ctx.get().device().update_descriptor_sets(write_desc_set.as_slice(), copies.as_slice()); }

        Ok(())
    }

    pub fn ptr(&self) -> Result<&vk::DescriptorSet, Error> {
        self.desc_set.as_ref().ok_or(anyhow!("Descriptor set is not valid"))
    }
}

impl Drop for DescriptorSets {
    fn drop(&mut self) {
        self.ctx.get().descriptor_pool().free(self.desc_set.take().unwrap()).unwrap();
    }
}