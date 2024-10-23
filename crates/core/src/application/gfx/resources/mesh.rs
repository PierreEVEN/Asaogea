use crate::application::gfx::device::DeviceCtx;
use crate::application::gfx::resources::buffer::{Buffer, BufferAccess, BufferCreateInfo, BufferMemory};
use anyhow::{anyhow, Error};
use vulkanalia::vk;

pub struct DynamicMesh {
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    vertex_structure_size: usize,
    index_type: IndexBufferType,
    ctx: DeviceCtx,
}

#[derive(Copy, Clone, Default)]
pub enum IndexBufferType {
    Uint8 = 1,
    Uint16 = 2,
    #[default]
    Uint32 = 4,
}

impl DynamicMesh {
    pub fn new(vertex_structure_size: usize, ctx: DeviceCtx) -> Result<Self, Error> {
        Ok(Self {
            vertex_buffer: None,
            index_buffer: None,
            vertex_structure_size,
            index_type: IndexBufferType::Uint32,
            ctx,
        })
    }

    pub fn vk_index_type(&self) -> vk::IndexType {
        match self.index_type {
            IndexBufferType::Uint8 => { vk::IndexType::UINT8_KHR }
            IndexBufferType::Uint16 => { vk::IndexType::UINT16 }
            IndexBufferType::Uint32 => { vk::IndexType::UINT32 }
        }
    }

    pub fn set_indexed_vertices(&mut self, start_vertex: usize, vertex_data: &BufferMemory, start_index: usize, index_data: &BufferMemory) -> Result<(), Error> {
        self.set_vertices(start_vertex, vertex_data)?;
        match index_data.stride() {
            1 => { self.index_type = IndexBufferType::Uint8 }
            2 => { self.index_type = IndexBufferType::Uint16 }
            4 => { self.index_type = IndexBufferType::Uint32 }
            s => { return Err(anyhow!("Unsupported index buffer type size : {s}")) }
        }
        let index_size = self.index_type as usize;
        if self.index_buffer.is_none() {
            self.index_buffer = Some(Buffer::from_buffer_memory(self.ctx.clone(), index_data, BufferCreateInfo {
                usage: vk::BufferUsageFlags::INDEX_BUFFER,
                access: BufferAccess::GpuOnly,
            })?);
        } else {
            let idx = self.index_buffer.as_mut().unwrap();
            if start_index * index_size + index_data.get_size() > idx.size() {
                idx.resize(index_data.get_size())?;
            }
            idx.set_data(start_index * index_size, index_data)?;
        };
        Ok(())
    }

    pub fn set_vertices(&mut self, start_vertex: usize, vertex_data: &BufferMemory) -> Result<(), Error> {
        assert_eq!(vertex_data.stride(), self.vertex_structure_size);
        if self.vertex_buffer.is_none() {
            self.vertex_buffer = Some(Buffer::from_buffer_memory(self.ctx.clone(), vertex_data, BufferCreateInfo {
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                access: BufferAccess::GpuOnly,
            })?);
        } else {
            let vtx = self.vertex_buffer.as_mut().unwrap();
            if start_vertex * self.vertex_structure_size + vertex_data.get_size() > vtx.size() {
                vtx.resize(vertex_data.get_size())?;
            }
            vtx.set_data(start_vertex * self.vertex_structure_size, vertex_data)?;
        };

        Ok(())
    }

    pub fn reserve_vertices(&mut self, vertex_count: usize) -> Result<(), Error> {
        match &mut self.vertex_buffer {
            None => {
                self.vertex_buffer = Some(Buffer::new(self.ctx.clone(), self.vertex_structure_size, vertex_count, BufferCreateInfo {
                    usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                    access: BufferAccess::GpuOnly,
                })?);
            }
            Some(vtx) => {
                vtx.resize(vertex_count)?;
            }
        }
        Ok(())
    }

    pub fn reserve_indices(&mut self, index_count: usize) -> Result<(), Error> {
        match &mut self.index_buffer {
            None => {
                self.index_buffer = Some(Buffer::new(self.ctx.clone(), self.vertex_structure_size, index_count, BufferCreateInfo {
                    usage: vk::BufferUsageFlags::INDEX_BUFFER,
                    access: BufferAccess::GpuOnly,
                })?);
            }
            Some(idx) => {
                idx.resize(index_count)?;
            }
        }
        Ok(())
    }

    pub fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertex_buffer.as_ref()
    }

    pub fn index_buffer(&self) -> Option<&Buffer> {
        self.index_buffer.as_ref()
    }

    pub fn index_count(&self) -> usize {
        match &self.index_buffer {
            None => {
                match &self.vertex_buffer {
                    None => { 0 }
                    Some(vtx) => { vtx.elements() }
                }
            }
            Some(idx) => { idx.elements() }
        }
    }
}

impl Drop for DynamicMesh {
    fn drop(&mut self) {
        self.vertex_buffer = None;
        self.index_buffer = None;
    }
}