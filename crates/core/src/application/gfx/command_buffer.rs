use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{CommandBufferBeginInfo, CommandBufferUsageFlags, DeviceV1_0, HasBuilder};
use crate::application::gfx::device::{Device, QueueFamilyIndices};
use crate::application::window::CtxAppWindow;
use crate::engine::CtxEngine;

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

    pub fn allocate(&self, device: &Device, num: u32) -> Result<Vec<vk::CommandBuffer>, Error> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool.expect("Command pool is null"))
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num);

        unsafe { Ok(device.ptr().allocate_command_buffers(&allocate_info)?) }
    }

    pub fn free(&self, ctx: &CtxEngine, command_buffers: &Vec<vk::CommandBuffer>) -> Result<(), Error> {
        unsafe { ctx.engine.device()?.ptr().free_command_buffers(self.command_pool.expect("Command pool is null"), command_buffers.as_slice()); }
        Ok(())
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

pub struct CommandBuffer {
    command_buffer: Option<vk::CommandBuffer>,
}

impl CommandBuffer {
    pub fn new(ctx: &CtxEngine) -> Result<Self, Error> {
        let command_buffer = ctx.engine.device()?.command_pool().allocate(&*ctx.engine.device()?, 1)?;
        Ok(Self {
            command_buffer: Some(command_buffer[0])
        })
    }

    pub fn begin_one_time(&self, ctx: &CtxEngine) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder()
            .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { ctx.engine.device()?.ptr().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }
    
    pub fn end(&self, ctx: &CtxEngine) -> Result<(), Error> {
        unsafe { ctx.engine.device()?.ptr().end_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?)?; }
        Ok(())
    }

    pub fn ptr(&self) -> Result<&vk::CommandBuffer, Error> {
        self.command_buffer.as_ref().ok_or(anyhow!("Invalid command buffer"))
    }

    pub fn destroy(&mut self, ctx: &CtxEngine) -> Result<(), Error> {
        if let Some(command_buffer) = self.command_buffer {
            ctx.engine.device()?.command_pool().free(ctx, &vec![command_buffer])?;
        }
        self.command_buffer = None;
        Ok(())
    }
}

impl Drop for CommandBuffer {
    fn drop(&mut self) {
        if self.command_buffer.is_some() {
            panic!("Command buffer have not been destroyed using CommandBuffer::destroy()")
        }
    }
}