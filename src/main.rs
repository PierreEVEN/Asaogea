use anyhow::Error;
use vulkanalia::vk;
use core::engine::Engine;
use core::application::Application;
use core::options::{Options, WindowOptions, RenderingOption};
use core::core::gfx::frame_graph::frame_graph_definition::*;

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut engine = Engine::new::<GameTestApp>(Options {
        rendering: RenderingOption {
            validation_layers: true,
            image_count: 2,
        },
        main_window: WindowOptions {
            name: "Asaogea".to_string()
        },
    })?;
    engine.run()
}

#[derive(Default)]
pub struct GameTestApp {}

impl Application for GameTestApp {
    fn instantiate(&mut self, device: &core::core::gfx::device::DeviceCtx) {
        device.declare_render_pass(RenderPass::new(RenderPassName::Named("depth_pass".to_string()))
            .depth_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::D32_SFLOAT)))).unwrap();

        device.declare_render_pass(RenderPass::new(RenderPassName::Named("forward".to_string()))
            .color_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::R16G16B16A16_SFLOAT)))
            .depth_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::D32_SFLOAT)))).unwrap();
    }

    fn create_window(&mut self, window: &mut core::core::window::WindowCtxMut) {
        let device = window.engine().instance().device();

        device.declare_render_pass(RenderPass::new(RenderPassName::Present(window.as_ref()))
            .color_attachment(RenderPassAttachment::new(RenderTarget::Window))).unwrap();

        let renderer = Renderer {
            present_stage: RendererStage {
                render_callback: Box::new(|| {}),
                name: RenderPassName::Present(window.as_ref()),
                dependencies: vec![
                    RendererStage {
                        render_callback: Box::new(|| {}),
                        name: RenderPassName::Named("forward".to_string()),
                        dependencies: vec![],
                    },
                    RendererStage {
                        render_callback: Box::new(|| {
                            
                        }),
                        name: RenderPassName::Named("depth_pass".to_string()),
                        dependencies: vec![],
                    }],
            },
        };

        window.set_renderer(renderer).unwrap();
    }

    fn pre_draw_window(&mut self, _: &core::core::window::WindowCtx) {
    }

    fn tick(&mut self, _: &core::engine::EngineCtx) {
    }

    fn destroy(&mut self) {
    }
}