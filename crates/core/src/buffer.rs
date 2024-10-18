use anyhow::{anyhow, Error};
use std::ops::Deref;
use vulkanalia::vk::{DeviceV1_0, HasBuilder, InstanceV1_0};
use vulkanalia::{vk, Device};
use crate::instance::Instance;

pub struct Buffer {
    buffer: Option<vk::Buffer>,
    buffer_memory: Option<vk::DeviceMemory>,
    usage: vk::BufferUsageFlags,
    size: usize,
}

impl Buffer {
    pub fn new(mut size: usize, usage: vk::BufferUsageFlags, device: &Device) -> Result<Self, Error> {
        if size == 0 {
            size = 1;
        }
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let memory_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(Self::get_memory_type_index(
                instance,
                vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
                requirements,
            )?);
        let buffer_memory = unsafe { device.allocate_memory(&memory_info, None) }?;
        unsafe { device.bind_buffer_memory(*buffer, buffer_memory, 0)?; }

        Ok(Self {
            buffer: Some(buffer),
            buffer_memory: Some(buffer_memory),
            usage,
            size,
        })
    }

    fn get_memory_type_index(
        instance: &Instance,
        properties: vk::MemoryPropertyFlags,
        requirements: vk::MemoryRequirements,
    ) -> Result<u32, Error> {
        let memory = unsafe { instance.get_physical_device_memory_properties(*instance.device().physical_device().ptr()) };
        (0..memory.memory_type_count)
            .find(|i| {
                let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
                let memory_type = memory.memory_types[*i as usize];
                suitable && memory_type.property_flags.contains(properties)
            })
            .ok_or_else(|| anyhow!("Failed to find suitable memory type."))?;

        Ok((0))
    }

    pub fn resize(&mut self, device: &Device, size: usize) -> Result<(), Error> {
        if size == 0 || size == self.size {
            return Ok(());
        }

        self.destroy(device);

        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(self.usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.buffer = unsafe { Some(device.create_buffer(&buffer_info, None)?) };
        self.size = size;

        Ok(())
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn destroy(&mut self, device: &Device) {
        unsafe { device.destroy_buffer(self.buffer.take().expect("Buffer have already been destroyed"), None); }
    }
}

impl Deref for Buffer {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer.as_ref().expect("Buffer have been destroyed !")
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.buffer.is_some() {
            panic!("Buffer have not been destroyed using Buffer::destroy()");
        }
    }
}