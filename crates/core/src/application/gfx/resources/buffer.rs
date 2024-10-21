use crate::engine::CtxEngine;
use anyhow::{anyhow, Error};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Weak;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use vulkanalia_vma::{Alloc, AllocationCreateFlags};
use crate::application::gfx::device::DeviceSharedData;

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
    ctx: DeviceSharedData
}

impl Buffer {
    pub fn new(ctx: DeviceSharedData, mut size: usize, create_infos: BufferCreateInfo) -> Result<Self, Error> {
        if size == 0 {
            size = 1;
        }
        let mut buffer = Self {
            buffer: None,
            buffer_memory: None,
            create_infos,
            size,
            ctx,
        };
        buffer.create()?;
        Ok(buffer)
    }

    pub fn resize(&mut self, mut size: usize) -> Result<(), Error> {
        if size == 0 {
            size = 1;
        }
        if size == self.size {
            return Ok(());
        }

        self.destroy();

        self.size = size;

        self.create()?;

        Ok(())
    }

    pub fn set_data(&mut self, start_offset: usize, data: &[u8]) -> Result<(), Error> {
        if start_offset + data.len() > self.size {
            return Err(anyhow!("buffer is to small : size={}, expected={}", self.size, start_offset + data.len()));
        }
        unsafe {
            let mapped_memory = self.ctx.allocator().map_memory(self.buffer_memory.unwrap()).unwrap();
            data.as_ptr().copy_to(mapped_memory.add(start_offset), data.len());
            self.ctx.allocator().unmap_memory(self.buffer_memory.unwrap());
        }
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn ptr(&self) -> Result<&vk::Buffer, Error> {
        self.buffer.as_ref().ok_or(anyhow!("Invalid buffer"))
    }

    pub fn create(&mut self) -> Result<(), Error> {
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

        let (buffer, buffer_memory) = unsafe { self.ctx.allocator().create_buffer(buffer_info, &options) }.unwrap();

        println!("create : {:?}", buffer_memory);

        self.buffer = Some(buffer);
        self.buffer_memory = Some(buffer_memory);
        Ok(())
    }

    fn destroy(&self) {
        println!("Destroy ! : {:?}", self.buffer_memory.unwrap());

        unsafe { self.ctx.allocator().destroy_buffer(self.buffer.unwrap(), self.buffer_memory.unwrap()) }
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

        //@TODO REMOVE THIS
        unsafe { self.ctx.device().device_wait_idle().unwrap(); }

        self.destroy();
    }
}