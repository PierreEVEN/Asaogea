use anyhow::{anyhow, Error};
use vulkanalia::{vk, Device};
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use crate::application::gfx::command_buffer::CommandBuffer;
use crate::application::gfx::device::DeviceCtx;

#[derive(Clone)]
pub struct RenderPassAttachment {
    pub clear_value: Option<vk::ClearValue>,
    pub image_format: vk::Format,
}

#[derive(Clone)]
pub struct RenderPassCreateInfos {
    pub color_attachments: Vec<RenderPassAttachment>,
    pub depth_attachment: Option<RenderPassAttachment>,
    pub is_present_pass: bool,
}

pub struct RenderPass {
    render_pass: vk::RenderPass,
    config: RenderPassCreateInfos,
    ctx: DeviceCtx
}

impl RenderPass {
    pub fn new(ctx: DeviceCtx, config: RenderPassCreateInfos) -> Result<Self, Error> {

        let mut attachment_descriptions = Vec::<vk::AttachmentDescription>::new();
        let mut color_attachment_references = Vec::<vk::AttachmentReference>::new();
        let mut _depth_attachment_reference = vk::AttachmentReference::default();
        let mut clear_values = Vec::new();

        for attachment in &config.color_attachments
        {
            match attachment.image_format {
                vk::Format::UNDEFINED => { panic!("wrong pixel format") }
                _ => {}
            };

            let attachment_index: u32 = attachment_descriptions.len() as u32;

            attachment_descriptions.push(vk::AttachmentDescription::builder()
                .format(attachment.image_format)
                .samples(vk::SampleCountFlags::_1)
                .load_op(if attachment.clear_value.is_some() { vk::AttachmentLoadOp::DONT_CARE } else { vk::AttachmentLoadOp::CLEAR })
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(if config.is_present_pass { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL })
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
        match &config.depth_attachment {
            None => {}
            Some(attachment) => {
                match attachment.image_format {
                    vk::Format::UNDEFINED => { panic!("wrong depth pixel format") }
                    _ => {}
                };

                let attachment_index: u32 = attachment_descriptions.len() as u32;

                attachment_descriptions.push(vk::AttachmentDescription::builder()
                    .format(attachment.image_format)
                    .samples(vk::SampleCountFlags::_1)
                    .load_op(if attachment.clear_value.is_some() { vk::AttachmentLoadOp::DONT_CARE } else { vk::AttachmentLoadOp::CLEAR })
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

        let render_pass = unsafe { ctx.get().device().create_render_pass(&render_pass_infos, None) }?;

        Ok(Self {
            render_pass,
            config,
            ctx
        })
    }

    pub fn ptr(&self) -> &vk::RenderPass {
        &self.render_pass
    }

    pub fn config(&self) -> &RenderPassCreateInfos {
        &self.config
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe { self.ctx.get().device().destroy_render_pass(self.render_pass, None); }
    }
}