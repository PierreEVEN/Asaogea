use crate::application::gfx::resources::buffer::Buffer;
use anyhow::Error;
use vulkanalia::vk;
use crate::application::window::CtxAppWindow;
use crate::engine::CtxEngine;

pub struct DynamicMesh {
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    vertex_structure_size: usize,
}

impl DynamicMesh {
    pub fn new(vertex_structure_size: usize, ctx: &CtxEngine) -> Result<Self, Error> {
        Ok(Self {
            vertex_buffer: Some(Buffer::new(ctx, 0, vk::BufferUsageFlags::VERTEX_BUFFER)?),
            index_buffer: Some(Buffer::new(ctx, 0, vk::BufferUsageFlags::INDEX_BUFFER)?),
            vertex_structure_size,
        })
    }

    pub fn set_data(&mut self, vertex_start: usize, vertex_data: &[u8], index_start: usize, index_data: &[u8]) -> Result<(), Error> {

        Ok(())
    }

    pub fn resize(&mut self, vertex_count: usize, index_count: usize) -> Result<(), Error> {
        Ok(())
    }

    pub fn destroy(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        if let Some(vertex_buffer) = &mut self.vertex_buffer {
            vertex_buffer.destroy(ctx)?;
        }
        self.vertex_buffer = None;
        if let Some(index_buffer) = &mut self.index_buffer {
            index_buffer.destroy(ctx)?;
        }
        self.index_buffer = None;
        Ok(())
    }
}

impl Drop for DynamicMesh {
    fn drop(&mut self) {
        if self.index_buffer.is_some() || self.vertex_buffer.is_some() {
            panic!("DynamicMesh::destroy() have not been called !");
        }
    }
}