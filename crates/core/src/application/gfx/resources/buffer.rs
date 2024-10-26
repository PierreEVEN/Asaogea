use crate::application::gfx::device::{DeviceCtx};
use anyhow::{anyhow, Error};
use std::ptr::slice_from_raw_parts;
use vulkanalia::vk;
use vulkanalia::vk::{HasBuilder};
use vulkanalia_vma::{Alloc, AllocationCreateFlags};
use types::resource_handle::Resource;

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

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub enum BufferType
{
    // No updates allowed
    #[default]
    Immutable,
    // Pretty never updated. Updating data would cause some freezes. Low memory footprint
    Static,
    // Data is stored internally, then automatically submitted. Can lead to a memory overhead depending on the get size.
    Dynamic,
    // Data need to be submitted every frames
    Immediate,
}

struct BufferResource {
    buffer: vk::Buffer,
    allocation: vulkanalia_vma::Allocation,
    ctx: DeviceCtx,
}

impl BufferResource {
    pub fn new(buffer: vk::Buffer, allocation: vulkanalia_vma::Allocation, ctx: DeviceCtx) -> Self {
        Self {
            buffer,
            allocation,
            ctx,
        }
    }
}

impl Drop for BufferResource {
    fn drop(&mut self) {
        unsafe { self.ctx.allocator().destroy_buffer(self.buffer, self.allocation) }
    }
}

#[derive(Copy, Clone)]
pub struct BufferCreateInfo {
    pub usage: vk::BufferUsageFlags,
    pub access: BufferAccess,
    pub buffer_type: BufferType,
}

pub struct Buffer {
    resource: Resource<Vec<BufferResource>>,
    elements: usize,
    stride: usize,
    create_infos: BufferCreateInfo,
    ctx: DeviceCtx,
}

impl Buffer {
    pub fn new(ctx: DeviceCtx, stride: usize, elements: usize, create_infos: BufferCreateInfo) -> Result<Self, Error> {
        assert!(stride > 0);
        let mut buffer = Self {
            resource: Resource::default(),
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

    pub fn resize(&mut self, new_element_count: usize) -> Result<(), Error> {
        if let BufferType::Immutable = self.create_infos.buffer_type {
            return Err(anyhow!("Cannot resize an immutable buffer"));
        }
        if new_element_count == self.elements {
            return Ok(());
        }
        self.elements = new_element_count;
        self.create()
    }

    pub fn set_data(&mut self, start_offset: usize, data: &BufferMemory) -> Result<(), Error> {
        if start_offset + data.get_size() > self.size() {
            return Err(anyhow!("buffer is to small : size={}, expected={}", self.size(), start_offset + data.get_size()));
        }
        unsafe {
            let resource = match self.create_infos.buffer_type {
                BufferType::Immutable | BufferType::Static => {
                    self.resource[0].allocation
                }
                BufferType::Immediate | BufferType::Dynamic => {
                    let frame = self.ctx.instance().engine().current_frame();
                    self.resource[frame].allocation
                }
            };

            let mapped_memory = self.ctx.allocator().map_memory(resource)?;
            data.get_ptr(0).copy_to(mapped_memory.add(start_offset), data.get_size());
            self.ctx.allocator().unmap_memory(resource);
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
        if !self.resource.is_valid() {
            return Err(anyhow!("Buffer is null"));
        }

        match self.create_infos.buffer_type {
            BufferType::Immutable | BufferType::Static => {
                Ok(&self.resource[0].buffer)
            }
            BufferType::Immediate | BufferType::Dynamic => {
                let frame = self.ctx.instance().engine().current_frame();
                Ok(&self.resource[frame].buffer)
            }
        }
    }

    pub fn queue_resource_for_destruction(&mut self) {
        if self.resource.is_valid() {
            self.ctx.queue_resource_cleanup(self.resource.take());
        }
    }

    pub fn create(&mut self) -> Result<(), Error> {
        self.queue_resource_for_destruction();
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

        match self.create_infos.buffer_type {
            BufferType::Immutable | BufferType::Static => {
                let (buffer, buffer_memory) = unsafe { self.ctx.allocator().create_buffer(buffer_info, &options) }?;
                self.resource = Resource::new(vec![BufferResource::new(buffer, buffer_memory, self.ctx.clone())])
            }
            BufferType::Immediate | BufferType::Dynamic => {
                self.resource = Resource::new(Vec::new());
                for _ in 0..self.ctx.instance().engine().params().rendering.image_count {
                    let (buffer, buffer_memory) = unsafe { self.ctx.allocator().create_buffer(buffer_info, &options) }?;
                    self.resource.push(BufferResource::new(buffer, buffer_memory, self.ctx.clone()))
                }
            }
        }
        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.queue_resource_for_destruction();
    }
}

pub struct BufferMemory<'a> {
    data: BufferDataType<'a>,
    stride: usize,
    elements: usize,
}
enum BufferDataType<'a> {
    Ptr(&'a [u8]),
    Raw(Box<dyn BufferDataTrait>),
}


impl<'a> BufferMemory<'a> {
    pub fn from_raw(data: *const u8, stride: usize, elements: usize) -> Self {
        assert!(stride >= 1);
        assert!(elements >= 1);
        unsafe { Self { data: BufferDataType::Ptr(slice_from_raw_parts(data, stride * elements).as_ref().unwrap()), stride, elements } }
    }

    pub fn from_slice<T: Sized>(structure: &'a [T]) -> Self {
        unsafe {
            Self {
                data: BufferDataType::Ptr(slice_from_raw_parts(structure.as_ptr() as *const u8, size_of_val(structure)).as_ref().unwrap()),
                stride: size_of::<T>(),
                elements: structure.len(),
            }
        }
    }

    pub fn from_struct_ref<T: Sized>(structure: &'a T) -> Self {
        let data = unsafe { slice_from_raw_parts(structure as *const T as *const u8, size_of::<T>()).as_ref().unwrap() };
        Self {
            data: BufferDataType::Ptr(data),
            stride: size_of::<T>(),
            elements: 1,
        }
    }

    pub fn from_struct<T: 'static + Sized>(structure: T) -> Self {
        Self {
            data: BufferDataType::Raw(Box::new(BufferData { object: vec![structure] })),
            stride: size_of::<T>(),
            elements: 1,
        }
    }

    pub fn from_vec<T: 'static + Sized>(structure: Vec<T>) -> Self {
        let elements = structure.len();
        Self {
            data: BufferDataType::Raw(Box::new(BufferData { object: structure })),
            stride: size_of::<T>(),
            elements,
        }
    }

    pub fn from_vec_ref<T: Sized>(structure: &Vec<T>) -> Self {
        let data = unsafe { slice_from_raw_parts(structure.as_ptr() as *const u8, structure.len() * size_of::<T>()).as_ref().unwrap() };
        Self {
            data: BufferDataType::Ptr(data),
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

    pub fn get_ptr(&self, offset: usize) -> *const u8 {
        match &self.data {
            BufferDataType::Ptr(slice) => { unsafe { slice.as_ptr().add(offset) } }
            BufferDataType::Raw(raw) => { unsafe { raw.memory().as_ptr().add(offset) } }
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        match &self.data {
            BufferDataType::Ptr(slice) => { slice }
            BufferDataType::Raw(raw) => { raw.memory() }
        }
    }
}

trait BufferDataTrait {
    fn memory(&self) -> &[u8];
}

struct BufferData<T: Sized> {
    object: Vec<T>,
}

impl<T: Sized> BufferDataTrait for BufferData<T> {
    fn memory(&self) -> &[u8] {
        unsafe { slice_from_raw_parts(self.object.as_ptr() as *const u8, self.object.len() * size_of::<T>()).as_ref().unwrap() }
    }
}
