use crate::application::gfx::resources::buffer::{Buffer, BufferAccess, BufferCreateInfo};
use anyhow::{anyhow, Error};
use vulkanalia::vk;
use crate::application::window::CtxAppWindow;
use crate::engine::CtxEngine;

pub struct DynamicMesh {
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    vertex_structure_size: usize,
    create_infos: MeshCreateInfos,
}

pub struct MeshCreateInfos {
    pub index_type: IndexBufferType,
}

#[derive(Copy, Clone)]
pub enum IndexBufferType {
    Uint16 = 2,
    Uint32 = 4,
}

impl DynamicMesh {
    pub fn new(vertex_structure_size: usize, ctx: &CtxEngine, create_infos: MeshCreateInfos) -> Result<Self, Error> {
        Ok(Self {
            vertex_buffer: Some(Buffer::new(ctx, 0, BufferCreateInfo {
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                access: BufferAccess::GpuOnly,
            })?),
            index_buffer: Some(Buffer::new(ctx, 0, BufferCreateInfo {
                usage: vk::BufferUsageFlags::INDEX_BUFFER,
                access: BufferAccess::GpuOnly,
            })?),
            vertex_structure_size,
            create_infos,
        })
    }

    fn index_buffer_type_size(&self) -> usize {
        match self.create_infos.index_type {
            IndexBufferType::Uint16 => { 2 }
            IndexBufferType::Uint32 => { 4 }
        }
    }

    pub fn set_data(&mut self, ctx: &CtxEngine, vertex_start: usize, vertex_data: &[u8], index_start: usize, index_data: &[u8]) -> Result<(), Error> {
        let index_size = self.index_buffer_type_size();
        let vtx = self.vertex_buffer.as_mut().unwrap();
        let idx = self.index_buffer.as_mut().unwrap();

        if vertex_start * self.vertex_structure_size + vertex_data.len() > vtx.size() {
            vtx.resize(ctx, vertex_data.len())?;
        }
        vtx.set_data(ctx, vertex_start * self.vertex_structure_size, vertex_data)?;

        if index_start * index_size + index_data.len() > idx.size() {
            idx.resize(ctx, index_data.len())?;
        }
        idx.set_data(ctx, index_start * index_size, index_data)?;

        Ok(())
    }

    pub fn resize(&mut self, ctx: &CtxEngine, vertex_count: usize, index_count: usize) -> Result<(), Error> {
        let index_size = self.index_buffer_type_size();
        let vtx = self.vertex_buffer.as_mut().unwrap();
        let idx = self.index_buffer.as_mut().unwrap();
        vtx.resize(ctx, vertex_count * self.vertex_structure_size)?;
        idx.resize(ctx, index_count * index_size)?;
        Ok(())
    }

    pub fn vertex_buffer(&self) -> Result<&Buffer, Error> {
        self.vertex_buffer.as_ref().ok_or(anyhow!("Vertex buffer is not valid"))
    }

    pub fn index_buffer(&self) -> Result<&Buffer, Error> {
        self.index_buffer.as_ref().ok_or(anyhow!("Index buffer is not valid"))
    }

    pub fn destroy(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        if let Some(vertex_buffer) = &mut self.vertex_buffer {
            vertex_buffer.destroy(ctx.ctx_engine())?;
        }
        self.vertex_buffer = None;
        if let Some(index_buffer) = &mut self.index_buffer {
            index_buffer.destroy(ctx.ctx_engine())?;
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