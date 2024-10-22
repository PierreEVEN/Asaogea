use crate::application::gfx::device::{DeviceCtx, QueueFamilyIndices};
use anyhow::{anyhow, Error};
use std::sync::{Arc, RwLock};
use vulkanalia::vk;
use vulkanalia::vk::{CommandBufferBeginInfo, CommandBufferResetFlags, CommandBufferUsageFlags, DeviceV1_0, HasBuilder};
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::descriptor_sets::DescriptorSets;
use crate::application::gfx::resources::mesh::DynamicMesh;
use crate::application::gfx::resources::pipeline::Pipeline;

pub struct CommandPool {
    command_pool: Option<vk::CommandPool>,
    ctx: RwLock<Option<DeviceCtx>>,
}

impl CommandPool {
    pub fn new(device: &vulkanalia::Device, queue_family_indices: &QueueFamilyIndices) -> Result<Self, Error> {
        let info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // Optional.
            .queue_family_index(queue_family_indices.graphics);
        let command_pool = unsafe { device.create_command_pool(&info, None) }?;

        Ok(Self {
            command_pool: Some(command_pool),
            ctx: Default::default(),
        })
    }

    pub fn init(&self, ctx: DeviceCtx) {
        *self.ctx.write().unwrap() = Some(ctx);
    }

    pub fn allocate(&self, num: u32) -> Result<Vec<vk::CommandBuffer>, Error> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool.expect("Command pool is null"))
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(num);

        unsafe { Ok(self.ctx.read().unwrap().as_ref().expect("Command pool have not been initialized").get().device().allocate_command_buffers(&allocate_info)?) }
    }

    pub fn free(&self, command_buffers: &Vec<vk::CommandBuffer>) -> Result<(), Error> {
        unsafe { self.ctx.read().unwrap().as_ref().unwrap().get().device().free_command_buffers(self.command_pool.expect("Command pool is null"), command_buffers.as_slice()); }
        Ok(())
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.ctx.read().unwrap().as_ref().unwrap().get().device().destroy_command_pool(self.command_pool.take().expect("This command pool is already destroyed"), None); }
    }
}

pub struct CommandBuffer {
    command_buffer: Option<vk::CommandBuffer>,
    ctx: DeviceCtx,
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
    pub fn new(ctx: DeviceCtx) -> Result<Self, Error> {
        let command_buffer = ctx.get().command_pool().allocate(1)?;
        Ok(Self {
            command_buffer: Some(command_buffer[0]),
            ctx,
        })
    }

    pub fn begin_one_time(&self) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder()
            .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { self.ctx.get().device().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }

    pub fn begin(&self) -> Result<(), Error> {
        let begin_infos = CommandBufferBeginInfo::builder().build();
        unsafe { self.ctx.get().device().begin_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, &begin_infos)?; }
        Ok(())
    }
    pub fn reset(&self) -> Result<(), Error> {
        unsafe { self.ctx.get().device().reset_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?, CommandBufferResetFlags::empty())?; }
        Ok(())
    }

    pub fn end(&self) -> Result<(), Error> {
        unsafe { self.ctx.get().device().end_command_buffer(self.command_buffer.ok_or(anyhow!("Command buffer is not valid"))?)?; }
        Ok(())
    }

    pub fn ptr(&self) -> Result<&vk::CommandBuffer, Error> {
        self.command_buffer.as_ref().ok_or(anyhow!("Invalid command buffer"))
    }

    pub fn bind_pipeline(&self, program: &Pipeline) {
        unsafe {
            unsafe {
                self.ctx.get().device().cmd_bind_pipeline(
                    self.command_buffer.unwrap(),
                    vk::PipelineBindPoint::GRAPHICS,
                    *program.ptr_pipeline(),
                );
            }
        }
    }

    pub fn bind_descriptors(&self, pipeline: &Pipeline, descriptors: &DescriptorSets) {
        unsafe {
            self.ctx.get().device().cmd_bind_descriptor_sets(
                self.command_buffer.unwrap(),
                vk::PipelineBindPoint::GRAPHICS,
                *pipeline.ptr_pipeline_layout(),
                0,
                &[*descriptors.ptr().unwrap()],
                &[],
            );
        }
    }

    pub fn draw_mesh(&self, mesh: &DynamicMesh, _instance_count: u32, _first_instance: u32) {
        unsafe {
            self.ctx.get().device().cmd_bind_index_buffer(
                self.command_buffer.unwrap(),
                *mesh.index_buffer().unwrap().ptr().unwrap(),
                0 as vk::DeviceSize,
                mesh.index_buffer_type());
            self.ctx.get().device().cmd_bind_vertex_buffers(
                self.command_buffer.unwrap(),
                0,
                &[*mesh.vertex_buffer().unwrap().ptr().unwrap()],
                &[0]);
            self.ctx.get().device().cmd_draw_indexed(self.command_buffer.unwrap(),
                                                     mesh.index_count() as u32,
                                                     1,
                                                     0,
                                                     0,
                                                     0);
        }
    }

    pub fn draw_mesh_advanced(&self, mesh: &DynamicMesh, first_index: u32, vertex_offset: i32, index_count: u32, instance_count: u32, first_instance: u32) {
        unsafe {
            self.ctx.get().device().cmd_bind_index_buffer(
                self.command_buffer.unwrap(),
                *mesh.index_buffer().unwrap().ptr().unwrap(),
                0 as vk::DeviceSize,
                mesh.index_buffer_type());
            self.ctx.get().device().cmd_bind_vertex_buffers(
                self.command_buffer.unwrap(),
                0,
                &[*mesh.vertex_buffer().unwrap().ptr().unwrap()],
                &[0]);
            self.ctx.get().device().cmd_draw_indexed(self.command_buffer.unwrap(),
                                                     index_count,
                                                     instance_count,
                                                     first_index,
                                                     vertex_offset,
                                                     first_instance);
        }
    }

    pub fn draw_procedural(&self, vertex_count: u32, first_vertex: u32, instance_count: u32, first_instance: u32) {
        todo!()
    }

    pub fn set_viewport(&self, viewport: &Viewport) {
        unsafe {
            self.ctx.get().device().cmd_set_viewport(self.command_buffer.unwrap(), 0, &[vk::Viewport::builder()
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
            self.ctx.get().device().cmd_set_scissor(self.command_buffer.unwrap(), 0, &[vk::Rect2D {
                extent: vk::Extent2D { width: scissors.width, height: scissors.height },
                offset: vk::Offset2D { x: scissors.min_x, y: scissors.min_y },
            }])
        }
    }

    pub fn push_constant(&self, pipeline: &Pipeline, data: &BufferMemory, stage: vk::ShaderStageFlags) {
        unsafe {
            self.ctx.get().device().cmd_push_constants(self.command_buffer.unwrap(), *pipeline.ptr_pipeline_layout(), stage, 0, data.as_slice())
        }
    }
}

impl Drop for CommandBuffer {
    fn drop(&mut self) {
        if let Some(command_buffer) = self.command_buffer {
            self.ctx.get().command_pool().free(&vec![command_buffer]).unwrap();
        }
        self.command_buffer = None;
    }
}