use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use crate::application::window::CtxAppWindow;

pub struct Sampler {
    sampler: Option<vk::Sampler>,
}

impl Sampler {
    pub fn new(ctx: &CtxAppWindow) -> Result<Self, Error> {
        let create_infos = vk::SamplerCreateInfo::builder()
            .build();

        let sampler = unsafe { ctx.engine().device()?.ptr().create_sampler(&create_infos, None) }?;

        Ok(Self { sampler: Some(sampler) })
    }

    pub fn ptr(&self) -> Result<&vk::Sampler, Error> {
        self.sampler.as_ref().ok_or(anyhow!("Sampler is not valid"))
    }

    pub fn destroy(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        if let Some(sampler) = self.sampler {
            unsafe { ctx.engine().device()?.ptr().destroy_sampler(sampler, None); }
        }
        self.sampler = None;
        Ok(())
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if self.sampler.is_some() {
            panic!("Sampler::destroy() haven not been called");
        }
    }
}