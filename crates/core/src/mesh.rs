use anyhow::Error;
use vulkanalia::{vk, Device};
use crate::buffer::Buffer;

pub struct DynamicMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_structure_size: usize,
}

impl DynamicMesh {
    pub fn new(vertex_structure_size: usize, device: &Device) -> Result<Self, Error> {
        Ok(Self {
            vertex_buffer: Buffer::new(0, vk::BufferUsageFlags::VERTEX_BUFFER, device)?,
            index_buffer: Buffer::new(0, vk::BufferUsageFlags::INDEX_BUFFER, device)?,
            vertex_structure_size,
        })
    }

    pub fn set_data(&mut self, vertex_start: usize, vertex_data: &[u8], index_start: usize, index_data: &[u8]) -> Result<(), Error> {

        Ok(())
    }

    pub fn resize(&mut self, vertex_count: usize, index_count: usize) -> Result<(), Error> {
        Ok(())
    }
}