use crate::application::gfx::command_buffer::{CommandBuffer, Scissors, Viewport};
use crate::application::gfx::device::QueueFlag::Graphic;
use crate::application::gfx::device::{DeviceCtx, QueueFlag};
use crate::application::gfx::frame_graph::frame_graph_definition::FrameGraph;
use crate::application::gfx::resources::image::Image;
use crate::application::gfx::swapchain::SwapchainCtx;
use std::cell::RefCell;
use types::resource_handle::{Resource, ResourceHandle};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};

pub struct FrameGraphInstance {
    present_pass: Resource<RenderPassInstance>,
    target: FrameGraphTargetInstance,
    ctx: DeviceCtx,
    base: FrameGraph
}

#[derive(Clone)]
pub enum FrameGraphTargetInstance {
    Swapchain(SwapchainCtx),
    Image(ResourceHandle<Image>),
}


pub struct SwapchainImage {
    pub image_view: vk::ImageView,
    pub wait_semaphore: vk::Semaphore,
    pub work_finished_fence: vk::Fence,
}

impl FrameGraphInstance {
    pub fn new(base: FrameGraph, target: FrameGraphTargetInstance, ctx: DeviceCtx) -> Resource<Self> {
        Resource::new(Self {
            present_pass,
            ctx,
            base,
        })
    }

    pub fn resize(&self, width: usize, height: usize, swapchain_images: &Vec<vk::ImageView>) {
        self.present_pass.resize(width, height, swapchain_images);
    }

    pub fn draw(&self, image_index: usize, swapchain_image: SwapchainImage) {
        self.present_pass.draw(image_index, Some(swapchain_image));
    }

    pub fn present_to_swapchain(&self, image_index: usize, swapchain: &vk::SwapchainKHR) -> bool {
        let signal_semaphores = &[self.present_pass.framebuffers[image_index].render_finished_semaphore];
        let swapchains = &[*swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices)
            .build();

        let result = self.ctx.queues().present(&present_info);
        result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR) || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR)
    }
}

pub struct RenderPass {
    dependencies: Vec<ResourceHandle<RenderPass>>,
    pub create_infos: RenderPassCreateInfos,
    pub render_pass: vk::RenderPass,
    ctx: DeviceCtx,
    _instances: Vec<Resource<RenderPassInstance>>,
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
            let (present_pass, format) = match &attachment.source {
                AttachmentSource::Surface(surface) => {
                    (true, surface.format())
                }
                AttachmentSource::Image(format) => {
                    (false, *format)
                }
            };

            let attachment_index: u32 = attachment_descriptions.len() as u32;

            attachment_descriptions.push(vk::AttachmentDescription::builder()
                .format(format)
                .samples(vk::SampleCountFlags::_1)
                .load_op(match attachment.clear_value {
                    ClearValues::DontClear => { vk::AttachmentLoadOp::DONT_CARE }
                    _ => { vk::AttachmentLoadOp::CLEAR }
                })
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if present_pass { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL })
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
                let format = match &attachment.source {
                    AttachmentSource::Surface(swapchain) => { swapchain.format() }
                    AttachmentSource::Image(format) => { *format }
                };
                attachment_descriptions.push(vk::AttachmentDescription::builder()
                    .format(format)
                    .samples(vk::SampleCountFlags::_1)
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


        let render_pass = unsafe { ctx.device().create_render_pass(&render_pass_infos, None) }.unwrap();


        Self {
            dependencies: vec![],
            create_infos,
            render_pass,
            ctx,
            _instances: vec![],
        }
    }

    pub fn attach(&mut self, dependency: ResourceHandle<RenderPass>) {
        self.dependencies.push(dependency);
    }

    fn instantiate(&self, image_count: usize) -> Resource<RenderPassInstance> {
        let mut children = vec![];

        for dependency in &self.dependencies {
            children.push(dependency.instantiate(image_count));
        }

        let mut attachments = self.create_infos.color_attachments.clone();
        if let Some(depth) = &self.create_infos.depth_attachment {
            attachments.push(depth.clone());
        }


        let mut framebuffers = vec![];
        for _ in 0..image_count {
            framebuffers.push(Framebuffer::new(self.ctx.clone()));
        }

        let instance = Resource::new(RenderPassInstance {
            framebuffers,
            children,
            ctx: self.ctx.clone(),
            attachments,
            render_pass: self.render_pass,
            width: 0,
            height: 0,
        });
        instance
    }
}


pub struct RenderPassInstance {
    pub framebuffers: Vec<Framebuffer>,
    children: Vec<Resource<RenderPassInstance>>,
    ctx: DeviceCtx,
    attachments: Vec<RenderPassAttachment>,
    render_pass: vk::RenderPass,
    width: usize,
    height: usize,
}

impl RenderPassInstance {
    pub fn resize(&self, width: usize, height: usize, swapchain_images: &Vec<vk::ImageView>) {
        for child in &self.children {
            child.resize(width, height, swapchain_images);
        }

        for framebuffer in &self.framebuffers {
            framebuffer.create_or_recreate(width, height, self.render_pass, swapchain_images);
        }
    }


    fn draw(&self, frame_index: usize, swapchain_image: Option<SwapchainImage>) {
        for child in &*self.children {
            child.draw(frame_index, None);
        }

        let device = &self.ctx;

        let framebuffer = &self.framebuffers[frame_index];


        // Begin buffer
        framebuffer.command_buffer.begin().unwrap();


        let mut clear_values = Vec::new();
        for attachment in &self.attachments {
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
            .render_pass(self.render_pass)
            .framebuffer(*framebuffer.vk_framebuffer.borrow())
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width: self.width as u32, height: self.height as u32 },
            })
            .clear_values(clear_values.as_slice())
            .build();


        unsafe { device.device().cmd_begin_render_pass(*framebuffer.command_buffer.ptr().unwrap(), &begin_infos, vk::SubpassContents::INLINE); }

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
        // todo!();

        // End pass
        unsafe { device.device().cmd_end_render_pass(*framebuffer.command_buffer.ptr().unwrap()); }
        framebuffer.command_buffer.end().unwrap();

        // Submit buffer
        let mut wait_semaphores = Vec::new();
        if let Some(swapchain_image) = swapchain_image {
            wait_semaphores.push(swapchain_image.wait_semaphore);
        }
        for _ in &self.children {
            wait_semaphores.push(framebuffer.render_finished_semaphore);
        }

        let wait_stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT; wait_semaphores.len()];

        let command_buffers = vec![*framebuffer.command_buffer.ptr().unwrap()];
        let signal_semaphores = vec![framebuffer.render_finished_semaphore];

        let submit_infos = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores.as_slice())
            .wait_dst_stage_mask(wait_stages.as_slice())
            .command_buffers(command_buffers.as_slice())
            .signal_semaphores(signal_semaphores.as_slice())
            .build();
        let submit_infos = vec![submit_infos];
        self.ctx.queues().submit(&QueueFlag::Graphic, submit_infos.as_slice(), None);
    }
}


pub struct Framebuffer {
    vk_framebuffer: RefCell<vk::Framebuffer>,
    command_buffer: CommandBuffer,
    render_finished_semaphore: vk::Semaphore,
    ctx: DeviceCtx,
}

impl Framebuffer {
    pub fn new(ctx: DeviceCtx) -> Self {
        Self {
            vk_framebuffer: RefCell::default(),
            command_buffer: CommandBuffer::new(ctx.clone(), &Graphic).unwrap(),
            render_finished_semaphore: Default::default(),
            ctx,
        }
    }

    pub fn create_or_recreate(&self, width: usize, height: usize, render_pass: vk::RenderPass, swapchain_images: &Vec<vk::ImageView>) {
        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .attachments(swapchain_images)
            .width(width as u32)
            .height(height as u32)
            .layers(1);
        self.vk_framebuffer.replace(unsafe { self.ctx.device().create_framebuffer(&create_info, None) }.unwrap());
    }
}