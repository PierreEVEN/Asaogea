use std::ops::Deref;
use anyhow::Error;
use vulkanalia::bytecode::Bytecode;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

pub struct ShaderModule {
    shader_module: Option<vk::ShaderModule>,
}

impl ShaderModule {
    pub fn new(device: &vulkanalia::Device, bytecode: &Vec<u8>) -> Result<Self, Error> {
        let bytecode = Bytecode::new(bytecode)?;
        let info = vk::ShaderModuleCreateInfo::builder()
            .code_size(bytecode.code_size())
            .code(bytecode.code());
        let shader_module = unsafe { device.create_shader_module(&info, None)? };
        Ok(Self {
            shader_module: Some(shader_module)
        })
    }

    pub fn destroy(&mut self, device: &vulkanalia::Device) {
        unsafe { device.destroy_shader_module(self.shader_module.take().expect("Shader module have already been destroyed"), None); }
    }
}

impl Deref for ShaderModule {
    type Target = vk::ShaderModule;

    fn deref(&self) -> &Self::Target {
        self.shader_module.as_ref().expect("Shader module have been destroyed")
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        if self.shader_module.is_some() {
            panic!("Shader module have not been destroyed using ShaderModule::destroy()");
        }
    }
}