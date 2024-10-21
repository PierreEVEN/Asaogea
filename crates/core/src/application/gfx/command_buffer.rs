use crate::application::gfx::device::{DeviceSharedData, QueueFamilyIndices};
use anyhow::{anyhow, Error};
use std::sync::RwLock;
use vulkanalia::vk;
use vulkanalia::vk::{CommandBufferBeginInfo, CommandBufferUsageFlags, DeviceV1_0, HasBuilder};

pub struct CommandPool {
    command_pool: Option<vk::CommandPool>,
    ctx: RwLock<Option<DeviceSharedData>>
}

impl CommandPool {
    pub fn new(device: &vulkanalia::Device, queue_family_indices: &QueueFamilyIndices) -> Result<Self, Error> {
        let info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // Optional.
            .queue_family_index(queue_family_indices.graphics);
        let command_pool = unsafe { device.create_command_pool(&info, None) }?;

        Ok(Self {
            command_pool: Some(command_pool),
            ctx: Default::default(),
        })
    }

    pub fn init(&self, ctx: DeviceSharedData) {
        *self.ctx.write().unwrap() = Some(ctx);
    }

    pub fn allocate(&self, num: u32) -> Result<Vec<vk::CommandBuffer>, Error> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool.expect("Command pool is null"))
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num);

        unsafe { Ok(self.ctx.read().unwrap().as_ref().unwrap().device().allocate_command_buffers(&allocate_info)?) }
    }

    pub fn free(&self, command_buffers: &Vec<vk::CommandBuffer>) -> Result<(), Error> {
        unsafe { self.ctx.read().unwrap().as_ref().unwrap().device().free_command_buffers(self.command_pool.expect("Command pool is null"), command_buffers.as_slice()); }
        Ok(())
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.ctx.read().unwrap().as_ref().unwrap().device().destroy_command_pool(self.command_pool.take().expect("This command pool is already destroyed"), None); }
    }
}

pub struct CommandBuffer {
    command_buffer: Option<vk::CommandBuffer>,
    ctx: DeviceSharedData
}

impl CommandBuffer {
    pub fn new(ctx: DeviceSharedData) -> Result<Self, Error> {
        let command_buffer = ctx.command_pool().allocate(1)?;
        Ok(Self {
            command_buffer: Some(command_buffer[0]),
            ctx,
        })
    }

    pub fn begin_one_time(&self) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder()
            .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { self.ctx.device().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }
    
    pub fn end(&self) -> Result<(), Error> {
        unsafe { self.ctx.device().end_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?)?; }
        Ok(())
    }

    pub fn ptr(&self) -> Result<&vk::CommandBuffer, Error> {
        self.command_buffer.as_ref().ok_or(anyhow!("Invalid command buffer"))
    }
}

impl Drop for CommandBuffer {
    fn drop(&mut self) {
        if let Some(command_buffer) = self.command_buffer {
            self.ctx.command_pool().free( &vec![command_buffer]).unwrap();
        }
        self.command_buffer = None;
    }
}