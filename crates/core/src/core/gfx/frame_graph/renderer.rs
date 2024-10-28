use crate::core::gfx::command_buffer::{CommandBuffer, Scissors, Viewport};
use crate::core::gfx::device::DeviceCtx;
use crate::core::gfx::frame_graph::frame_graph_definition::{ClearValues, RenderPass, RenderPassName, RenderTarget, Renderer, RendererStage};
use crate::core::gfx::imgui::{ImGui, UiPtr};
use crate::core::gfx::queues::QueueFlag;
use crate::core::gfx::resources::image::{Image, ImageCreateOptions};
use crate::core::gfx::swapchain::{FrameData, SwapchainCtx};
use types::resource_handle::{Resource, ResourceHandle};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Extent2D, HasBuilder};

pub enum FrameGraphTargetInstance {
    Swapchain(SwapchainCtx),
    Image(Vec<ResourceHandle<Image>>),
    Internal(Vec<AttachmentInstance>),
}

pub struct AttachmentInstance {
    images: Vec<Resource<Image>>,
}

impl AttachmentInstance {
    pub fn new(ctx: &DeviceCtx, format: vk::Format, is_depth: bool, image_count: u32, res: vk::Extent2D) -> Self {
        let mut images = vec![];

        for _ in 0..image_count {
            images.push(Image::new(ctx.clone(), ImageCreateOptions {
                image_type: vk::ImageType::_2D,
                format,
                usage: if is_depth { vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT } else { vk::ImageUsageFlags::COLOR_ATTACHMENT } | vk::ImageUsageFlags::SAMPLED,
                width: res.width,
                height: res.height,
                depth: 1,
                mips_levels: 1,
                is_depth,
            }).unwrap());
        }

        Self {
            images,
        }
    }
}


pub struct RendererInstance {
    present_pass: Resource<RenderPassInstance>,
    imgui: Resource<ImGui>,
}

impl RendererInstance {
    pub fn new(ctx: DeviceCtx, base: Renderer, target: FrameGraphTargetInstance) -> Resource<Self> {
        let render_pass_object = ctx.find_render_pass(&base.present_stage.name).unwrap();

        let render_res = match &target {
            FrameGraphTargetInstance::Swapchain(swapchain) => {
                Extent2D {
                    width: swapchain.window().width().unwrap(),
                    height: swapchain.window().height().unwrap(),
                }
            }
            FrameGraphTargetInstance::Image(images) => { images[0].res() }
            FrameGraphTargetInstance::Internal(_) => { panic!("Invalild target") }
        };

        let imgui = ImGui::new(ctx.clone(), render_res, &render_pass_object).unwrap();

        if let FrameGraphTargetInstance::Swapchain(swapchain) = &target {
            imgui.set_target_window_for_inputs(swapchain.window().clone());
        }

        let mut renderer = Resource::new(Self {
            present_pass: Default::default(),
            imgui,
        });
        renderer.present_pass = render_pass_object.instantiate(base.present_stage, target, renderer.handle());


        renderer
    }

    pub fn resize(&mut self) {
        self.present_pass.resize();
    }

    pub fn draw(&mut self, data: &FrameData, target_index: usize) {
        self.present_pass.draw(data, target_index);
    }

    pub fn ui<'a>(&self) -> UiPtr<'a> {
        self.imgui.ui()
    }

    pub fn present_pass(&self) -> &RenderPassInstance {
        &self.present_pass
    }
}

pub struct RenderPassObject {
    ctx: DeviceCtx,
    base: RenderPass,
    render_pass: vk::RenderPass,
    _instances: Vec<Resource<RenderPassInstance>>,
    self_ctx: ResourceHandle<RenderPassObject>,
}

impl RenderPassObject {
    pub fn new(ctx: DeviceCtx, base: &RenderPass) -> Resource<Self> {
        let mut attachment_descriptions = Vec::<vk::AttachmentDescription>::new();
        let mut color_attachment_references = Vec::<vk::AttachmentReference>::new();
        let mut _depth_attachment_reference = vk::AttachmentReference::default();
        let mut clear_values = Vec::new();

        // add color color_attachments
        for attachment in &base.color_attachments
        {
            let (present_pass, format) = match &attachment.source {
                RenderTarget::Window => {
                    if let RenderPassName::Present(window) = &base.name {
                        (true, window.swapchain().format())
                    } else {
                        panic!("Invalid render pass name")
                    }
                }
                RenderTarget::Image(image) => { (false, image.format()) }
                RenderTarget::Internal(format) => { (false, *format) }
            };

            let attachment_index: u32 = attachment_descriptions.len() as u32;

            attachment_descriptions.push(vk::AttachmentDescription::builder()
                .format(format)
                .samples(vk::SampleCountFlags::_1)
                .load_op(match attachment.clear_value {
                    ClearValues::DontClear => { vk::AttachmentLoadOp::DONT_CARE }
                    _ => { vk::AttachmentLoadOp::CLEAR }
                }
                )
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
        match &base.depth_attachment {
            None => {}
            Some(attachment) => {
                let attachment_index: u32 = attachment_descriptions.len() as u32;
                let format = match &attachment.source {
                    RenderTarget::Window => { panic!("Swapchain doesn't support depth target") }
                    RenderTarget::Image(image) => { image.format() }
                    RenderTarget::Internal(format) => { *format }
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

        let subpasses = vec![subpass];
        let render_pass_infos = vk::RenderPassCreateInfo::builder()
            .attachments(attachment_descriptions.as_slice())
            .subpasses(subpasses.as_slice())
            .dependencies(dependencies.as_slice())
            .build();


        let render_pass = unsafe { ctx.device().create_render_pass(&render_pass_infos, None) }.unwrap();

        let mut pass = Resource::new(Self {
            ctx,
            base: base.clone(),
            render_pass,
            _instances: vec![],
            self_ctx: Default::default(),
        });
        pass.self_ctx = pass.handle();

        pass
    }

    pub fn base(&self) -> &RenderPass {
        &self.base
    }

    pub fn ptr(&self) -> &vk::RenderPass {
        &self.render_pass
    }

    fn instantiate(&self, mut stage: RendererStage, target: FrameGraphTargetInstance, renderer: ResourceHandle<RendererInstance>) -> Resource<RenderPassInstance> {
        let mut children = vec![];

        let draw_res = match &target {
            FrameGraphTargetInstance::Swapchain(swapchain) => { Extent2D { width: swapchain.window().width().unwrap(), height: swapchain.window().height().unwrap() } }
            FrameGraphTargetInstance::Image(image) => { image[0].res() }
            FrameGraphTargetInstance::Internal(attachment) => { attachment[0].images[0].res() }
        };

        let (framebuffer_count, image_count) = match &target {
            FrameGraphTargetInstance::Swapchain(swapchain) => { (swapchain.get_swapchain_images().len(), swapchain.get_swapchain_images().len() - 1) }
            FrameGraphTargetInstance::Image(images) => { (images.len(), images.len()) }
            FrameGraphTargetInstance::Internal(attachments) => { (attachments[0].images.len(), attachments[0].images.len()) }
        };

        for stage in stage.dependencies {
            let child = self.ctx.find_render_pass(&stage.name).unwrap();

            let mut attachments = vec![];

            for color in &child.base.color_attachments {
                let format = match color.source {
                    RenderTarget::Internal(format) => { format }
                    _ => panic!("Only internal formats are allowed for children targets")
                };
                attachments.push(AttachmentInstance::new(&self.ctx, format, false, image_count as u32, draw_res));
            }
            if let Some(depth) = &child.base.depth_attachment {
                let format = match depth.source {
                    RenderTarget::Internal(format) => { format }
                    _ => panic!("Only internal formats are allowed for children targets")
                };
                attachments.push(AttachmentInstance::new(&self.ctx, format, true, image_count as u32, draw_res));
            }

            children.push(child.instantiate(stage, FrameGraphTargetInstance::Internal(attachments), renderer.clone()));
        }
        stage.dependencies = vec![];

        let mut instance = Resource::new(RenderPassInstance {
            framebuffers: vec![],
            children,
            ctx: self.ctx.clone(),
            object: self.self_ctx.clone(),
            current_draw_res: draw_res,
            stage,
            renderer,
            target,
            self_ctx: Default::default(),
        });
        instance.self_ctx = instance.handle();
        let handle = instance.handle();
        for i in 0..framebuffer_count {
            instance.framebuffers.push(Framebuffer::new(handle.clone(), i as u32));
        }
        assert!(!instance.framebuffers.is_empty());
        instance
    }
}

impl Drop for RenderPassObject {
    fn drop(&mut self) {
        unsafe { self.ctx.device().destroy_render_pass(self.render_pass, None) };
    }
}


pub struct RenderPassInstance {
    framebuffers: Vec<Framebuffer>,
    children: Vec<Resource<RenderPassInstance>>,
    object: ResourceHandle<RenderPassObject>,
    ctx: DeviceCtx,
    current_draw_res: Extent2D,
    stage: RendererStage,
    renderer: ResourceHandle<RendererInstance>,
    target: FrameGraphTargetInstance,
    self_ctx: ResourceHandle<RenderPassInstance>,
}

impl RenderPassInstance {
    pub fn resize(&mut self) {
        let draw_res = match &self.target {
            FrameGraphTargetInstance::Swapchain(swapchain) => { Extent2D { width: swapchain.window().width().unwrap(), height: swapchain.window().height().unwrap() } }
            FrameGraphTargetInstance::Image(image) => { image[0].res() }
            FrameGraphTargetInstance::Internal(attachment) => { attachment[0].images[0].res() }
        };
        self.current_draw_res = draw_res;
        for child in &mut self.children {
            child.resize();
        }

        let num_framebuffers = self.framebuffers.len();
        self.framebuffers.clear();
        for i in 0..num_framebuffers {
            self.framebuffers.push(Framebuffer::new(self.self_ctx.clone(), i as u32));
        }
    }

    pub fn render_pass_object(&self) -> &ResourceHandle<RenderPassObject> {
        &self.object
    }

    pub fn render_finished_semaphore(&self, image_index: usize) -> vk::Semaphore {
        self.framebuffers[image_index].render_finished_semaphore
    }

    fn draw(&mut self, data: &FrameData, target_index: usize) {
        for child in &mut *self.children {
            child.draw(data, data.frame_index);
        }

        let device = &self.ctx;

        let framebuffer = &self.framebuffers[target_index];


        // Begin buffer
        framebuffer.command_buffer.begin().unwrap();


        let mut clear_values = Vec::new();

        for attachment in &self.object.base.color_attachments {
            clear_values.push(match attachment.clear_value {
                ClearValues::DontClear => { vk::ClearValue::default() }
                ClearValues::Color(color) => {
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [color.x, color.y, color.z, color.w]
                        }
                    }
                }
                _ => { panic!("Not a color attachment") }
            });
        }
        if let Some(attachment) = &self.object.base.depth_attachment {
            clear_values.push(match attachment.clear_value {
                ClearValues::DontClear => { vk::ClearValue::default() }
                ClearValues::DepthStencil(depth_stencil) => {
                    vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: depth_stencil.x,
                            stencil: depth_stencil.y as u32,
                        }
                    }
                }
                _ => { panic!("Not a depth attachment") }
            });
        }

        // begin pass
        let begin_infos = vk::RenderPassBeginInfo::builder()
            .render_pass(self.object.render_pass)
            .framebuffer(framebuffer.vk_framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.current_draw_res,
            })
            .clear_values(clear_values.as_slice())
            .build();


        unsafe { device.device().cmd_begin_render_pass(*framebuffer.command_buffer.ptr().unwrap(), &begin_infos, vk::SubpassContents::INLINE); }

        framebuffer.command_buffer.set_viewport(&Viewport {
            min_x: 0.0,
            min_y: self.current_draw_res.height as f32,
            width: self.current_draw_res.width as f32,
            height: -(self.current_draw_res.height as f32),
            min_depth: 0.0,
            max_depth: 1.0,
        });

        framebuffer.command_buffer.set_scissor(Scissors {
            min_x: 0,
            min_y: 0,
            width: self.current_draw_res.width,
            height: self.current_draw_res.height,
        });

        // Draw content
        (self.stage.render_callback)();

        // Todo : have a flag for final passes
        match &self.target {
            FrameGraphTargetInstance::Swapchain(_) => {
                self.renderer.imgui.submit_frame(&framebuffer.command_buffer, self.current_draw_res).unwrap();
            }
            FrameGraphTargetInstance::Image(_) => {
                self.renderer.imgui.submit_frame(&framebuffer.command_buffer, self.current_draw_res).unwrap();
            }
            FrameGraphTargetInstance::Internal(_) => {}
        }

        // End pass
        unsafe { device.device().cmd_end_render_pass(*framebuffer.command_buffer.ptr().unwrap()); }
        framebuffer.command_buffer.end().unwrap();

        // Submit buffer
        let mut wait_semaphores = Vec::new();

        let mut signal_fence = None;

        if let FrameGraphTargetInstance::Swapchain(swapchain) = &self.target {
            wait_semaphores.push(*swapchain.get_image_available_semaphore(data.frame_index));
            signal_fence = Some(swapchain.get_in_flight_fence(data.frame_index));
        }

        for child in &self.children {
            wait_semaphores.push(child.framebuffers[data.frame_index].render_finished_semaphore);
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
        self.ctx.queues().submit(&QueueFlag::Graphic, submit_infos.as_slice(), signal_fence);
    }
}


pub struct Framebuffer {
    vk_framebuffer: vk::Framebuffer,
    command_buffer: CommandBuffer,
    render_finished_semaphore: vk::Semaphore,
    ctx: DeviceCtx,
}

impl Framebuffer {
    pub fn new(render_pass: ResourceHandle<RenderPassInstance>, image_index: u32) -> Self {
        let mut source_views = vec![];
        match &render_pass.target {
            FrameGraphTargetInstance::Swapchain(swapchain) => {
                source_views.push(swapchain.get_swapchain_images()[image_index as usize])
            }
            FrameGraphTargetInstance::Image(images) => {
                source_views.push(*images[image_index as usize].view().unwrap())
            }
            FrameGraphTargetInstance::Internal(attachments) => {
                for attachment in attachments {
                    source_views.push(*attachment.images[image_index as usize].view().unwrap())
                }
            }
        }

        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass.object.render_pass)
            .attachments(source_views.as_slice())
            .width(render_pass.current_draw_res.width)
            .height(render_pass.current_draw_res.height)
            .layers(1);


        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        Self {
            vk_framebuffer: unsafe { render_pass.ctx.device().create_framebuffer(&create_info, None) }.unwrap(),
            command_buffer: CommandBuffer::new(render_pass.ctx.clone(), &QueueFlag::Graphic).unwrap(),
            render_finished_semaphore: unsafe { render_pass.ctx.device().create_semaphore(&semaphore_info, None).unwrap() },
            ctx: render_pass.ctx.clone(),
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe { self.ctx.device().destroy_framebuffer(self.vk_framebuffer, None) };
        unsafe { self.ctx.device().destroy_semaphore(self.render_finished_semaphore, None) };
    }
}