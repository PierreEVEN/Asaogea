use crate::core::gfx::device::DeviceCtx;
use anyhow::Error;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

pub struct Sampler {
    sampler: vk::Sampler,
    ctx: DeviceCtx,
}

impl Sampler {
    pub fn new(ctx: DeviceCtx) -> Result<Self, Error> {
        let create_infos = vk::SamplerCreateInfo::builder()
            .build();

        let sampler = unsafe { ctx.device().create_sampler(&create_infos, None) }?;

        Ok(Self { sampler, ctx })
    }

    pub fn ptr(&self) -> &vk::Sampler {
        &self.sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe { self.ctx.device().destroy_sampler(self.sampler, None); }
    }
}