use anyhow::Error;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use crate::shader_module::ShaderModule;

pub struct Pipeline {
    pipeline_layout: Option<vk::PipelineLayout>,
}

impl Pipeline {
    pub fn new(device: &vulkanalia::Device, mut vertex_module: ShaderModule, mut fragment_module: ShaderModule) -> Result<Self, Error> {
        let layout_info = vk::PipelineLayoutCreateInfo::builder();
        let pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }?;

        vertex_module.destroy(device);
        fragment_module.destroy(device);

        Ok(Self {
            pipeline_layout: Some(pipeline_layout),
        })
    }

    pub fn destroy(&mut self, device: &vulkanalia::Device) {
        unsafe { device.destroy_pipeline_layout(self.pipeline_layout.take().expect("Shader module have already been destroyed"), None); }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        if self.pipeline_layout.is_some() {
            panic!("Pipeline have not been destroyed using Pipeline::destroy()");
        }
    }
}