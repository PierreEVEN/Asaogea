use crate::engine::CtxEngine;
use anyhow::{anyhow, Error};
use std::ops::Deref;
use vulkanalia::vk;
use vulkanalia::vk::{HasBuilder};
use vulkanalia_vma::{Alloc, AllocationCreateFlags};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum BufferAccess
{
    // Choose best configuration
    Default,
    // Data will be cached on GPU
    GpuOnly,
    // frequent transfer from CPU to GPU
    CpuToGpu,
    // frequent transfer from GPU to CPU
    GpuToCpu,
}

#[derive(Copy, Clone)]
pub struct BufferCreateInfo {
    pub usage: vk::BufferUsageFlags,
    pub access: BufferAccess,
}

pub struct Buffer {
    buffer: Option<vk::Buffer>,
    buffer_memory: Option<vulkanalia_vma::Allocation>,
    size: usize,
    create_infos: BufferCreateInfo,
}

impl Buffer {
    pub fn new(ctx: &CtxEngine, mut size: usize, create_infos: BufferCreateInfo) -> Result<Self, Error> {
        if size == 0 {
            size = 1;
        }
        let mut buffer = Self {
            buffer: None,
            buffer_memory: None,
            create_infos,
            size,
        };
        buffer.create(ctx)?;
        Ok(buffer)
    }

    pub fn resize(&mut self, ctx: &CtxEngine, mut size: usize) -> Result<(), Error> {
        if size == 0 {
            size = 1;
        }
        if size == self.size {
            return Ok(());
        }

        self.destroy(ctx)?;

        self.size = size;

        self.create(ctx)?;

        Ok(())
    }

    pub fn set_data(&mut self, ctx: &CtxEngine, start_offset: usize, data: &[u8]) -> Result<(), Error> {
        if start_offset + data.len() > self.size {
            return Err(anyhow!("buffer is to small : size={}, expected={}", self.size, start_offset + data.len()));
        }
        unsafe {
            let mapped_memory = ctx.engine.device()?.allocator().map_memory(self.buffer_memory.unwrap()).unwrap();
            data.as_ptr().copy_to(mapped_memory.add(start_offset), data.len());
            ctx.engine.device()?.allocator().unmap_memory(self.buffer_memory.unwrap());
        }
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn ptr(&self) -> Result<&vk::Buffer, Error> {
        self.buffer.as_ref().ok_or(anyhow!("Invalid buffer"))
    }

    pub fn create(&mut self, ctx: &CtxEngine) -> Result<(), Error> {
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(self.size as u64)
            .usage(self.create_infos.usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let mut options = vulkanalia_vma::AllocationOptions::default();

        match self.create_infos.access {
            BufferAccess::Default => {}
            BufferAccess::GpuOnly => {
                options.flags = AllocationCreateFlags::HOST_ACCESS_RANDOM;
                options.required_flags = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            }
            BufferAccess::CpuToGpu => {
                options.flags = AllocationCreateFlags::HOST_ACCESS_RANDOM;
                options.required_flags = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            }
            BufferAccess::GpuToCpu => {
                options.required_flags = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            }
        }

        let (buffer, buffer_memory) = unsafe { ctx.engine.device()?.allocator().create_buffer(buffer_info, &options) }.unwrap();

        self.buffer = Some(buffer);
        self.buffer_memory = Some(buffer_memory);
        Ok(())
    }

    pub fn destroy(&mut self, ctx: &CtxEngine) -> Result<(), Error> {
        unsafe { ctx.engine.device()?.allocator().destroy_buffer(self.buffer.take().expect("Buffer have already been destroyed"), self.buffer_memory.take().expect("Buffer memory have already been destroyed")) }
        Ok(())
    }
}

impl Deref for Buffer {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().expect("Buffer have been destroyed !")
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.buffer.is_some() {
            panic!("Buffer have not been destroyed using Buffer::destroy()");
        }
        if self.buffer_memory.is_some() {
            panic!("Buffer memory have not been destroyed using Buffer::destroy()");
        }
    }
}