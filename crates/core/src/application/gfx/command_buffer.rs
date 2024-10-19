use anyhow::Error;
use vulkanalia::vk;
use vulkanalia::vk::{CommandBuffer, DeviceV1_0, HasBuilder};
use crate::application::gfx::device::{Device, QueueFamilyIndices};

pub struct CommandPool {
    command_pool: Option<vk::CommandPool>,
}

impl CommandPool {
    pub fn new(device: &vulkanalia::Device, queue_family_indices: &QueueFamilyIndices) -> Result<Self, Error> {
        let info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // Optional.
            .queue_family_index(queue_family_indices.graphics);
        let command_pool = unsafe { device.create_command_pool(&info, None) }?;

        Ok(Self {
            command_pool: Some(command_pool),
        })
    }

    pub fn allocate(&self, device: &Device, num: u32) -> Result<Vec<CommandBuffer>, Error> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool.expect("Command pool is null"))
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num);

        unsafe { Ok(device.ptr().allocate_command_buffers(&allocate_info)?) }
    }

    pub fn free(&self, device: &Device, command_buffers: &Vec<CommandBuffer>) {
        unsafe { device.ptr().free_command_buffers(self.command_pool.expect("Command pool is null"), command_buffers.as_slice()); }
    }

    pub fn destroy(&mut self, device: &vulkanalia::Device) {
        unsafe { device.destroy_command_pool(self.command_pool.take().expect("This command pool is already destroyed"), None); }
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        if self.command_pool.is_some() {
            panic!("Command pool have not been destroyed using CommandPool::destroy()");
        }
    }
}