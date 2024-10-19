use std::ops::Deref;
use anyhow::Error;
use vulkanalia::bytecode::Bytecode;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

#[derive(Clone)]
pub struct ShaderStageBindings {
    pub binding: u32,
    pub descriptor_type: vk::DescriptorType,
}

pub struct ShaderStageInputs {
    pub location: i32,
    pub offset: u32,
    pub input_size: u32,
    pub property_type: vk::Format,
}

pub struct ShaderStageInfos {
    pub descriptor_bindings: Vec<ShaderStageBindings>,
    pub push_constant_size: Option<u32>,
    pub stage_input: Vec<ShaderStageInputs>,
    pub stage: vk::ShaderStageFlags,
    pub entry_point: String
}


pub struct ShaderStage {
    shader_module: Option<vk::ShaderModule>,
    infos: ShaderStageInfos
}

impl ShaderStage {
    pub fn new(device: &vulkanalia::Device, bytecode: &Vec<u8>, infos: ShaderStageInfos) -> Result<Self, Error> {
        let bytecode = Bytecode::new(bytecode)?;
        let info = vk::ShaderModuleCreateInfo::builder()
            .code_size(bytecode.code_size())
            .code(bytecode.code());
        let shader_module = unsafe { device.create_shader_module(&info, None)? };
        Ok(Self {
            shader_module: Some(shader_module),
            infos,
        })
    }

    pub fn shader_module(&self) -> &vk::ShaderModule {
        &self.shader_module.as_ref().expect("Shader module have been destroyed")
    }

    pub fn destroy(&mut self, device: &vulkanalia::Device) {
        unsafe { device.destroy_shader_module(self.shader_module.take().expect("Shader module have already been destroyed"), None); }
    }

    pub fn infos(&self) -> &ShaderStageInfos {
        &self.infos
    }
}

impl Deref for ShaderStage {
    type Target = vk::ShaderModule;

    fn deref(&self) -> &Self::Target {
        self.shader_module.as_ref().expect("Shader module have been destroyed")
    }
}

impl Drop for ShaderStage {
    fn drop(&mut self) {
        if self.shader_module.is_some() {
            panic!("Shader module have not been destroyed using ShaderModule::destroy()");
        }
    }
}