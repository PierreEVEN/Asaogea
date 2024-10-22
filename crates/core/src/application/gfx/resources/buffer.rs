use anyhow::{anyhow, Error};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Weak;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use vulkanalia_vma::{Alloc, AllocationCreateFlags};
use crate::application::gfx::device::DeviceCtx;

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
    elements: usize,
    stride: usize,
    create_infos: BufferCreateInfo,
    ctx: DeviceCtx,
}

impl Buffer {
    pub fn new(ctx: DeviceCtx, stride: usize, elements: usize, create_infos: BufferCreateInfo) -> Result<Self, Error> {
        assert!(stride > 0);
        let mut buffer = Self {
            buffer: None,
            buffer_memory: None,
            elements,
            stride,
            create_infos,
            ctx,
        };
        buffer.create()?;
        Ok(buffer)
    }

    pub fn from_buffer_memory(ctx: DeviceCtx, memory: &BufferMemory, create_infos: BufferCreateInfo) -> Result<Self, Error> {
        let mut buffer = Self::new(ctx, memory.stride, memory.elements, create_infos)?;
        buffer.set_data(0, memory)?;
        Ok(buffer)
    }
    
    pub fn resize(&mut self, mut new_element_count: usize) -> Result<(), Error> {
        if new_element_count == 0 {
            new_element_count = 1;
        }
        if new_element_count == self.elements {
            return Ok(());
        }

        self.destroy();
        self.elements = new_element_count;
        self.create()?;
        Ok(())
    }

    pub fn set_data(&mut self, start_offset: usize, data: &BufferMemory) -> Result<(), Error> {
        if start_offset + data.get_size() > self.size() {
            return Err(anyhow!("buffer is to small : size={}, expected={}", self.size(), start_offset + data.get_size()));
        }
        unsafe {
            let mapped_memory = self.ctx.get().allocator().map_memory(self.buffer_memory.unwrap())?;
            data.get_ptr(0).copy_to(mapped_memory.add(start_offset), data.get_size());
            self.ctx.get().allocator().unmap_memory(self.buffer_memory.unwrap());
        }
        Ok(())
    }
    pub fn size(&self) -> usize {
        self.elements * self.stride
    }
    pub fn elements(&self) -> usize {
        self.elements
    }
    pub fn stride(&self) -> usize {
        self.stride
    }
    pub fn ptr(&self) -> Result<&vk::Buffer, Error> {
        self.buffer.as_ref().ok_or(anyhow!("Buffer is null"))
    }

    pub fn create(&mut self) -> Result<(), Error> {
        if self.elements == 0 {
            return Ok(());
        }

        let buffer_info = vk::BufferCreateInfo::builder()
            .size(self.size() as u64)
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

        let (buffer, buffer_memory) = unsafe { self.ctx.get().allocator().create_buffer(buffer_info, &options) }.unwrap();

        self.buffer = Some(buffer);
        self.buffer_memory = Some(buffer_memory);
        Ok(())
    }

    fn destroy(&self) {
        if let Some(buffer) = self.buffer {
            unsafe { self.ctx.get().allocator().destroy_buffer(buffer, self.buffer_memory.unwrap()) }
        }
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
        unsafe { self.ctx.get().device().device_wait_idle().unwrap(); }

        self.destroy();
    }
}


pub struct BufferMemory {
    data: *const u8,
    stride: usize,
    elements: usize,
}

impl BufferMemory {
    pub fn from_raw(data: *const u8, stride: usize, elements: usize) -> Self {
        assert!(stride >= 1);
        assert!(elements >= 1);
        Self { data, stride, elements }
    }

    pub fn from_struct<T: Sized>(structure: &T) -> Self {
        Self {
            data: structure as *const T as *const u8,
            stride: size_of::<T>(),
            elements: 1,
        }
    }

    pub fn from_vec<T: Sized>(structure: &Vec<T>) -> Self {
        Self {
            data: structure.as_ptr() as *const u8,
            stride: size_of::<T>(),
            elements: structure.len(),
        }
    }

    pub fn elements(&self) -> usize {
        self.elements
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn get_size(&self) -> usize {
        self.stride * self.elements
    }

    pub fn get_ptr(&self, offset: usize) -> *mut u8 {
        let data = self.data as *mut u8;
        unsafe { data.offset(offset as isize) }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { ::std::slice::from_raw_parts(self.data, self.get_size()) }
    }
}
