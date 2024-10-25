use std::ops::Deref;
use std::sync::Arc;
use image::Frame;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use crate::application::gfx::command_buffer::{CommandBuffer, Scissors, Viewport};
use crate::application::gfx::device::{DeviceCtx, Fence, QueueFlag};
use crate::application::gfx::resources::image::Image;

pub struct FrameGraph {}

#[derive(Copy, Clone, Default)]
pub enum ClearValues {
    #[default]
    DontClear,
    Color(glam::Vec4),
    DepthStencil(glam::Vec2),
}

#[derive(Clone)]
pub struct RenderPassAttachment {
    pub clear_value: ClearValues,
    pub image_format: vk::Format,
}

#[derive(Clone)]
pub struct RenderPassCreateInfos {
    pub color_attachments: Vec<RenderPassAttachment>,
    pub depth_attachment: Option<RenderPassAttachment>,
    pub is_present_pass: bool,
}

pub enum AttachmentSource {
    PresentSurface,
    Images,
}

pub struct RenderPass {
    dependencies: Vec<Arc<RenderPass>>,
    attachments: Vec<RenderPassAttachment>,
    render_pass: vk::RenderPass,
    sources: AttachmentSource
}

impl RenderPass {
    pub fn new(ctx: DeviceCtx, create_infos: RenderPassCreateInfos) -> Self {
        let mut attachment_descriptions = Vec::<vk::AttachmentDescription>::new();
        let mut color_attachment_references = Vec::<vk::AttachmentReference>::new();
        let mut _depth_attachment_reference = vk::AttachmentReference::default();
        let mut clear_values = Vec::new();

        // add color color_attachments
        for attachment in &create_infos.color_attachments
        {
            let attachment_index: u32 = attachment_descriptions.len() as u32;

            attachment_descriptions.push(vk::AttachmentDescription::builder()
                .format(attachment.image_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(match attachment.clear_value {
                    ClearValues::DontClear => { vk::AttachmentLoadOp::DONT_CARE }
                    _ => { vk::AttachmentLoadOp::CLEAR }
                })
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if create_infos.is_present_pass { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL })
                .build());

            color_attachment_references.push(vk::AttachmentReference {
                attachment: attachment_index,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            });

            clear_values.push(attachment.clear_value);
        }

        let mut subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachment_references.as_slice())
            .build();

        // add depth attachment
        match &create_infos.depth_attachment {
            None => {}
            Some(attachment) => {
                let attachment_index: u32 = attachment_descriptions.len() as u32;

                attachment_descriptions.push(vk::AttachmentDescription::builder()
                    .format(attachment.image_format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(match attachment.clear_value {
                        ClearValues::DontClear => { vk::AttachmentLoadOp::DONT_CARE }
                        _ => { vk::AttachmentLoadOp::CLEAR }
                    })
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .build());

                _depth_attachment_reference = vk::AttachmentReference::builder()
                    .attachment(attachment_index)
                    .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .build();
                subpass.depth_stencil_attachment = &_depth_attachment_reference;
                clear_values.push(attachment.clear_value);
            }
        };

        let dependencies = vec![
            vk::SubpassDependency::builder()
                .src_subpass(vk::SUBPASS_EXTERNAL)                                                             // Producer of the dependency
                .dst_subpass(0)                                                                            // Consumer is our single subpass that will wait for the execution dependency
                .src_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)                                        // Match our pWaitDstStageMask when we vkQueueSubmit
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)                               // is a loadOp stage for color color_attachments
                .src_access_mask(vk::AccessFlags::MEMORY_READ)                                                 // semaphore wait already does memory dependency for us
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE) // is a loadOp CLEAR access mask for color color_attachments
                .dependency_flags(vk::DependencyFlags::BY_REGION)
                .build(),
            vk::SubpassDependency::builder()
                .src_subpass(0)                                                                            // Producer of the dependency is our single subpass
                .dst_subpass(vk::SUBPASS_EXTERNAL)                                                             // Consumer are all commands outside of the render pass
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)                               // is a storeOp stage for color color_attachments
                .dst_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)                                        // Do not block any subsequent work
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE) // is a storeOp `STORE` access mask for color color_attachments
                .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                .dependency_flags(vk::DependencyFlags::BY_REGION)
                .build(),
        ];

        let render_pass_infos = vk::RenderPassCreateInfo::builder()
            .attachments(attachment_descriptions.as_slice())
            .subpasses(&[subpass])
            .dependencies(dependencies.as_slice())
            .build();


        let render_pass = unsafe { ctx.get().device().create_render_pass(&render_pass_infos, None) }.unwrap();


        Self {
            dependencies: vec![],
            attachments: vec![],
            render_pass,
            sources: AttachmentSource::PresentSurface,
        }
    }
}


pub struct RenderPassInstance {
    framebuffers: Vec<Framebuffer>,
    children: Vec<RenderPassInstance>,
    ctx: DeviceCtx,
    owner: Arc<RenderPass>,
    width: usize,
    height: usize,
}

impl RenderPassInstance {
    fn draw(&self, frame_index: usize) {
        for child in &*self.children {
            child.draw(frame_index);
        }

        let device = &self.ctx.get();

        let framebuffer = &self.framebuffers[frame_index];


        // Begin buffer
        framebuffer.command_buffer.begin().unwrap();


        let mut clear_values = Vec::new();
        for attachment in &self.owner.attachments {
            clear_values.push(match attachment.clear_value {
                ClearValues::DontClear => { vk::ClearValue::default() }
                ClearValues::Color(color) => {
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [color.x, color.y, color.z, color.w]
                        }
                    }
                }
                ClearValues::DepthStencil(depth_stencil) => {
                    vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: depth_stencil.x,
                            stencil: depth_stencil.y as u32,
                        }
                    }
                }
            });
        }

        // begin pass
        let begin_infos = vk::RenderPassBeginInfo::builder()
            .render_pass(self.owner.render_pass)
            .framebuffer(framebuffer.vk_framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width: self.width as u32, height: self.height  as u32},
            })
            .clear_values(clear_values.as_slice())
            .build();



        unsafe { device.device().cmd_begin_render_pass(*framebuffer.command_buffer.ptr()?, &begin_infos, vk::SubpassContents::INLINE); }

        framebuffer.command_buffer.set_viewport(&Viewport {
            min_x: 0.0,
            min_y: self.height as f32,
            width: self.width as f32,
            height: -(self.height as f32),
            min_depth: 0.0,
            max_depth: 1.0,
        });

        framebuffer.command_buffer.set_scissor(Scissors {
            min_x: 0,
            min_y: 0,
            width: self.width as u32,
            height: self.height as u32,
        });

        // Draw content
        /*
        back.write() {
            Ok(mut render_callback) => {
                match render_callback.as_mut() {
                    None => {}
                    Some(callback) => {
                        callback(&(self.pass_command_buffers.clone() as Arc<dyn GfxCommandBuffer>))
                    }
                }
            }
            Err(_) => { panic!("failed to access render callback") }
        }*/

        // End pass
        unsafe { device.device().cmd_end_render_pass(*framebuffer.command_buffer.ptr()?); }
        framebuffer.command_buffer.end().unwrap();

        // Submit buffer
        let mut wait_semaphores = Vec::new();
        if let AttachmentSource::PresentSurface() = self.owner.sources {
            wait_semaphores.push(self.wait_semaphores.read().unwrap().unwrap());
        }
        for child in &self.children {
            wait_semaphores.push(framebuffer.render_finished_semaphore);
        }

        let wait_stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT; wait_semaphores.len()];

        let command_buffers=  vec![ framebuffer.command_buffer];
        let signal_semaphores = vec![framebuffer.render_finished_semaphore];

        let submit_infos = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores.as_slice())
            .wait_dst_stage_mask(wait_stages.as_slice())
            .command_buffers(command_buffers.as_slice())
            .signal_semaphores(signal_semaphores.as_slice())
            .build();
        self.ctx.get().queues().submit(QueueFlag::Graphic, submit_infos, Fence::default());
    }
}

pub struct Framebuffer {
    images: Image,
    vk_framebuffer: vk::Framebuffer,
    command_buffer: CommandBuffer,
    render_finished_semaphore: vk::Semaphore
}

impl Framebuffer {}