use vulkanalia::vk;
use types::resource_handle::ResourceHandle;
use crate::core::gfx::frame_graph::frame_graph_instance::FrameGraphTargetInstance;
use crate::core::gfx::resources::image::Image;
use crate::core::window::WindowCtx;

#[derive(Clone)]
pub enum RenderPassName {
    Present(WindowCtx),
    Named(String)
}

#[derive(Copy, Clone, Default)]
pub enum ClearValues {
    #[default]
    DontClear,
    Color(glam::Vec4),
    DepthStencil(glam::Vec2),
}

#[derive(Clone)]
pub enum RenderTarget {
    Window,
    Image(ResourceHandle<Image>),
    Internal(vk::Format),
}

#[derive(Clone)]
pub struct RenderPassAttachment {
    pub clear_value: ClearValues,
    pub source: RenderTarget,
}

impl RenderPassAttachment {
    pub fn new(source: RenderTarget) -> Self {
        Self {
            clear_value: Default::default(),
            source,
        }
    }
    
    pub fn clear(mut self, clear_value: ClearValues) -> Self {
        self.clear_value = clear_value;
        self
    }
}

#[derive(Clone)]
pub struct RenderPass {
    pub color_attachments: Vec<RenderPassAttachment>,
    pub depth_attachment: Option<RenderPassAttachment>,
    pub name: RenderPassName
}

impl RenderPass {

    pub fn new(name: RenderPassName) -> Self {
        Self {
            color_attachments: vec![],
            depth_attachment: None,
            name,
        }
    }
    pub fn color_attachment(mut self, attachment: RenderPassAttachment) -> Self {
        self.color_attachments.push(attachment);
        self
    }
    pub fn depth_attachment(mut self, attachment: RenderPassAttachment) -> Self {
        self.depth_attachment = Some(attachment);
        self
    }
}

pub struct Renderer {
    pub present_stage: RendererStage,
}

pub struct RendererStage {
    pub render_callback: Box<dyn FnMut()>,
    pub name: RenderPassName,
    pub dependencies: Vec<RendererStage>
}



