use std::ffi::c_char;
use anyhow::Error;
use imgui::sys::{igBegin, igEnd};
use core::application::Application;
use core::core::gfx::frame_graph::frame_graph_definition::*;
use core::engine::Engine;
use core::options::{Options, RenderingOption, WindowOptions};
use vulkanalia::vk;
use types::profiler::Profiler;

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
    Profiler::get().enable(true);
    engine.run()
}

#[derive(Default)]
pub struct GameTestApp {}

impl Application for GameTestApp {
    fn instantiate(&mut self, window: &mut core::core::window::WindowCtxMut) {
        let device = Engine::get().instance().device();
        window.engine().instance().device().declare_render_pass(RenderPass::new(RenderPassName::Named("depth_pass".to_string()))
            .depth_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::D32_SFLOAT)))).unwrap();

        window.engine().instance().device().declare_render_pass(RenderPass::new(RenderPassName::Named("forward".to_string()))
            .color_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::R16G16B16A16_SFLOAT)))
            .depth_attachment(RenderPassAttachment::new(RenderTarget::Internal(vk::Format::D32_SFLOAT)))).unwrap();


        device.declare_render_pass(RenderPass::new(RenderPassName::Present(window.as_ref()))
            .color_attachment(RenderPassAttachment::new(RenderTarget::Window).clear(ClearValues::Color(glam::Vec4::new(0.5f32, 1.5f32, 0.05f32, 1.0f32))))).unwrap();

        let window_ref = window.as_ref();

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
                        render_callback: Box::new(move || {
                        }),
                        name: RenderPassName::Named("depth_pass".to_string()),
                        dependencies: vec![],
                    }],
            },
            name: format!("MAIN_WINDOW"),
        };
        window.set_renderer(renderer).unwrap();


        let mut secondary_window = Engine::get_mut().create_window(&WindowOptions {
            name: "BLBLBL".to_string(),
        }).unwrap();

        device.declare_render_pass(RenderPass::new(RenderPassName::Present(secondary_window.as_ref()))
            .color_attachment(RenderPassAttachment::new(RenderTarget::Window).clear(ClearValues::Color(glam::Vec4::new(0.5f32, 0.5f32, 1.0f32, 1.0f32))))).unwrap();

        let window_ref = secondary_window.as_ref();
        let renderer = Renderer {
            present_stage: RendererStage {
                render_callback: Box::new(|| {}),
                name: RenderPassName::Present(secondary_window.as_ref()),
                dependencies: vec![
                    RendererStage {
                        render_callback: Box::new(|| {}),
                        name: RenderPassName::Named("forward".to_string()),
                        dependencies: vec![],
                    },
                    RendererStage {
                        render_callback: Box::new(move || {
                            let ui = window_ref.swapchain().renderer().ui();


                            let mut open = true;
                            if unsafe { igBegin("coucou toto\0".as_ptr() as *const imgui::sys::cty::c_char, (&mut open) as *mut bool, 0) } {
                                unsafe { igEnd(); }
                            }





                            println!("draw recorded data");
                            for elem in Profiler::get().current() {
                                println!("{} : {:?}", elem.name, elem.elapsed);
                            }
                        }),
                        name: RenderPassName::Named("depth_pass".to_string()),
                        dependencies: vec![],
                    }],
            },
            name: format!("MAIN_WINDOW"),
        };
        secondary_window.set_renderer(renderer).unwrap();
    }

    fn create_window(&mut self, _: &mut core::core::window::WindowCtxMut) {}

    fn pre_draw_window(&mut self, _: &core::core::window::WindowCtx) {}

    fn tick(&mut self, _: &core::engine::EngineCtx) {}

    fn destroy(&mut self) {}
}