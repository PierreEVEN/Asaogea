use crate::core::gfx::device::{DeviceCtx};
use crate::core::gfx::resources::buffer::BufferMemory;
use crate::core::gfx::resources::descriptor_sets::DescriptorSets;
use crate::core::gfx::resources::mesh::Mesh;
use crate::core::gfx::resources::pipeline::Pipeline;
use anyhow::{anyhow, Error};
use std::collections::HashMap;
use std::thread;
use types::rwslock::RwSLock;
use vulkanalia::vk;
use vulkanalia::vk::{CommandBufferBeginInfo, CommandBufferResetFlags, CommandBufferUsageFlags, DeviceV1_0, Handle, HasBuilder};
use crate::core::gfx::queues::QueueFlag;

pub struct CommandPool {
    command_pool: RwSLock<HashMap<thread::ThreadId, vk::CommandPool>>,
    ctx: DeviceCtx,
    queue_family: usize,
}

impl CommandPool {
    pub fn new(ctx: DeviceCtx, queue_family: usize) -> Result<Self, Error> {
        Ok(Self {
            command_pool: RwSLock::new(Default::default()),
            ctx,
            queue_family,
        })
    }

    pub fn allocate(&self, num: u32) -> Result<Vec<vk::CommandBuffer>, Error> {
        let thread = thread::current().id();

        let mut command_pool = if let Some(command_pool) = self.command_pool.read()?.get(&thread) {
            *command_pool
        } else { vk::CommandPool::null() };
        if command_pool.is_null()
        {
            let info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // Optional.
                .queue_family_index(self.queue_family as u32);
            command_pool = unsafe { self.ctx.device().create_command_pool(&info, None) }?;
            self.command_pool.write()?.insert(thread, command_pool);
        };

        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num);

        unsafe { Ok(self.ctx.device().allocate_command_buffers(&allocate_info)?) }
    }

    pub fn free(&self, command_buffers: &Vec<vk::CommandBuffer>, thread_id: &thread::ThreadId) -> Result<(), Error> {
        assert_eq!(*thread_id, thread::current().id());
        let pools = self.command_pool.read()?;
        let command_pool = pools.get(thread_id).unwrap();
        unsafe { self.ctx.device().free_command_buffers(*command_pool, command_buffers.as_slice()); }
        Ok(())
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        let pools = self.command_pool.read().unwrap();
        for (_, pool) in &*pools {
            unsafe { self.ctx.device().destroy_command_pool(*pool, None); }
        }
    }
}

pub struct CommandBuffer {
    command_buffer: Option<vk::CommandBuffer>,
    ctx: DeviceCtx,
    queue_flag: QueueFlag,
    thread_id: thread::ThreadId,
}

#[derive(Clone, Copy)]
pub struct Scissors {
    pub min_x: i32,
    pub min_y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy)]
pub struct Viewport {
    pub min_x: f32,
    pub min_y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl CommandBuffer {
    pub fn new(ctx: DeviceCtx, queue_flag: &QueueFlag) -> Result<Self, Error> {
        let command_buffer = ctx.command_pool(queue_flag).allocate(1)?;
        Ok(Self {
            command_buffer: Some(command_buffer[0]),
            ctx,
            queue_flag: *queue_flag,
            thread_id: thread::current().id(),
        })
    }

    pub fn begin_one_time(&self) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder()
            .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { self.ctx.device().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }

    pub fn begin(&self) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder().build();
        unsafe { self.ctx.device().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }
    pub fn reset(&self) -> Result<(), Error> {
        unsafe { self.ctx.device().reset_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, CommandBufferResetFlags::empty())?; }
        Ok(())
    }

    pub fn end(&self) -> Result<(), Error> {
        unsafe { self.ctx.device().end_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?)?; }
        Ok(())
    }

    pub fn ptr(&self) -> Result<&vk::CommandBuffer, Error> {
        self.command_buffer.as_ref().ok_or(anyhow!("Invalid command buffer"))
    }

    pub fn bind_pipeline(&self, program: &Pipeline) {
        unsafe {
            self.ctx.device().cmd_bind_pipeline(
                self.command_buffer.unwrap(),
                vk::PipelineBindPoint::GRAPHICS,
                *program.ptr_pipeline(),
            );
        }
    }

    pub fn bind_descriptors(&self, pipeline: &Pipeline, descriptors: &DescriptorSets) {
        unsafe {
            self.ctx.device().cmd_bind_descriptor_sets(
                self.command_buffer.unwrap(),
                vk::PipelineBindPoint::GRAPHICS,
                *pipeline.ptr_pipeline_layout(),
                0,
                &[*descriptors.ptr().unwrap()],
                &[],
            );
        }
    }

    pub fn draw_mesh(&self, mesh: &Mesh, _instance_count: u32, _first_instance: u32) {
        unsafe {
            let device = self.ctx.device();
            let vertex_buffer = if let Some(vertex_buffer) = mesh.vertex_buffer() { vertex_buffer } else { return; };
            device.cmd_bind_vertex_buffers(
                self.command_buffer.unwrap(),
                0,
                &[*vertex_buffer.ptr().unwrap()],
                &[0]);

            match mesh.index_buffer() {
                None => {
                    device.cmd_draw(self.command_buffer.unwrap(),
                                    mesh.index_count() as u32,
                                    1,
                                    0,
                                    0);
                }
                Some(index_buffer) => {
                    device.cmd_bind_index_buffer(
                        self.command_buffer.unwrap(),
                        *index_buffer.ptr().unwrap(),
                        0 as vk::DeviceSize,
                        mesh.vk_index_type());
                    device.cmd_draw_indexed(self.command_buffer.unwrap(),
                                            mesh.index_count() as u32,
                                            1,
                                            0,
                                            0,
                                            0);
                }
            }
        }
    }

    pub fn draw_mesh_advanced(&self, mesh: &Mesh, first_index: u32, vertex_offset: u32, index_count: u32, instance_count: u32, first_instance: u32) {
        unsafe {
            let vertex_buffer = if let Some(vertex_buffer) = mesh.vertex_buffer() { vertex_buffer } else { return; };
            self.ctx.device().cmd_bind_vertex_buffers(
                self.command_buffer.unwrap(),
                0,
                &[*vertex_buffer.ptr().unwrap()],
                &[0]);

            match mesh.index_buffer() {
                None => {
                    self.ctx.device().cmd_draw(self.command_buffer.unwrap(),
                                                     index_count,
                                                     instance_count,
                                                     vertex_offset,
                                                     first_instance);
                }
                Some(index_buffer) => {
                    self.ctx.device().cmd_bind_index_buffer(
                        self.command_buffer.unwrap(),
                        *index_buffer.ptr().unwrap(),
                        0 as vk::DeviceSize,
                        mesh.vk_index_type());
                    self.ctx.device().cmd_draw_indexed(self.command_buffer.unwrap(),
                                                             index_count,
                                                             instance_count,
                                                             first_index,
                                                             vertex_offset as i32,
                                                             first_instance);
                }
            }
        }
    }

    pub fn draw_procedural(&self, _vertex_count: u32, _first_vertex: u32, _instance_count: u32, _first_instance: u32) {
        todo!()
    }

    pub fn set_viewport(&self, viewport: &Viewport) {
        unsafe {
            self.ctx.device().cmd_set_viewport(self.command_buffer.unwrap(), 0, &[vk::Viewport::builder()
                .x(viewport.min_x)
                .y(viewport.min_y)
                .width(viewport.width)
                .height(viewport.height)
                .min_depth(viewport.min_depth)
                .max_depth(viewport.max_depth)
                .build()
            ])
        };
    }

    pub fn set_scissor(&self, scissors: Scissors) {
        unsafe {
            self.ctx.device().cmd_set_scissor(self.command_buffer.unwrap(), 0, &[vk::Rect2D {
                extent: vk::Extent2D { width: scissors.width, height: scissors.height },
                offset: vk::Offset2D { x: scissors.min_x, y: scissors.min_y },
            }])
        }
    }

    pub fn push_constant(&self, pipeline: &Pipeline, data: &BufferMemory, stage: vk::ShaderStageFlags) {
        unsafe {
            self.ctx.device().cmd_push_constants(self.command_buffer.unwrap(), *pipeline.ptr_pipeline_layout(), stage, 0, data.as_slice())
        }
    }
}

impl Drop for CommandBuffer {
    fn drop(&mut self) {
        if let Some(command_buffer) = self.command_buffer {
            self.ctx.command_pool(&self.queue_flag).free(&vec![command_buffer], &self.thread_id).unwrap();
        }
        self.command_buffer = None;
    }
}