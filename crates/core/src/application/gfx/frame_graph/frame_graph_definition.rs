use vulkanalia::vk;
use types::resource_handle::ResourceHandle;
use crate::application::gfx::resources::image::Image;
use crate::application::gfx::surface::SurfaceCtx;

#[derive(Copy, Clone, Default)]
pub enum ClearValues {
    #[default]
    DontClear,
    Color(glam::Vec4),
    DepthStencil(glam::Vec2),
}

#[derive(Clone)]
pub struct FrameGraph {
    root: RenderPass
}

#[derive(Clone)]
pub enum RenderTarget {
    Surface(SurfaceCtx),
    Image(ResourceHandle<Image>),
    Internal(vk::Format),
}

#[derive(Clone)]
pub struct RenderPassAttachment {
    pub clear_value: ClearValues,
    pub source: RenderTarget,
}

#[derive(Clone)]
pub struct RenderPass {
    pub color_attachments: Vec<RenderPassAttachment>,
    pub depth_attachment: Option<RenderPassAttachment>,
    pub children: Vec<RenderPass>
}