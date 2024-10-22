use std::f32::consts::PI;
use std::ops::{Add, Sub};
use std::slice;
use anyhow::Error;
use glam::{DVec2, Mat4, Vec3};
use imgui::sys::ImDrawVert;
use tracing::info;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0};
use winit::keyboard::{Key, NamedKey, SmolStr};
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use crate::application::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::descriptor_sets::DescriptorSets;
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType, MeshCreateInfos};
use crate::application::gfx::resources::pipeline::{AlphaMode, Pipeline, PipelineConfig};
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageInfos, ShaderStageInputs};
use crate::application::gfx::swapchain::SwapchainCtx;
use crate::test_app::camera::Camera;

const PIXEL: &str = r#"
struct VSInput {
    [[vk::location(0)]] float3 aPos 	: POSITION;
};
struct VsToFs {
    float4 Pos 		: SV_Position;
};
struct PushConsts {
    float4x4 model;
    float4x4 camera;
};
[[vk::push_constant]] ConstantBuffer<PushConsts> pc;
VsToFs main(VSInput input) {
    VsToFs Out;
    Out.Pos 	= mul(pc.camera, mul(pc.model, float4(input.aPos, 1)));
    return Out;
}
"#;

const FRAGMENT: &str = r#"
struct VsToFs {
    float4 Pos 		: SV_Position;
};

float4 main(VsToFs input) : SV_TARGET {
    return float4(1, 0, 0, 1);
}
"#;

pub struct TestApp {
    pipeline: Pipeline,
    mesh: DynamicMesh,
    ctx: SwapchainCtx,
    descriptor_sets: DescriptorSets,
    camera: Camera,
    pitch: f32,
    yaw: f32,
    last_mouse: DVec2,
}

pub struct Pc {
    pub model: Mat4,
    pub camera: Mat4,
}

impl TestApp {
    pub fn new(ctx: SwapchainCtx, render_pass: &RenderPass) -> Result<Self, Error> {
        let mut compiler = HlslCompiler::new()?;
        let vertex = compiler.compile(&RawShaderDefinition::new("imgui-vertex", "vs_6_0", PIXEL.to_string()))?;
        let fragment = compiler.compile(&RawShaderDefinition::new("imgui-fragment", "ps_6_0", FRAGMENT.to_string()))?;

        let vertex = ShaderStage::new(ctx.get().device().clone(), &vertex.raw(), ShaderStageInfos {
            descriptor_bindings: vec![],
            push_constant_size: Some(size_of::<Pc>() as u32),
            stage_input: vec![
                ShaderStageInputs {
                    location: 0,
                    offset: 0,
                    input_size: 12,
                    property_type: vk::Format::R32G32B32_SFLOAT,
                }],
            stage: vk::ShaderStageFlags::VERTEX,
            entry_point: "main".to_string(),
        })?;
        let fragment = ShaderStage::new(ctx.get().device().clone(), &fragment.raw(),
                                        ShaderStageInfos {
                                            descriptor_bindings: vec![],
                                            push_constant_size: None,
                                            stage_input: vec![],
                                            stage: vk::ShaderStageFlags::FRAGMENT,
                                            entry_point: "main".to_string(),
                                        })?;

        let pipeline = Pipeline::new(ctx.get().device().clone(), render_pass, vec![vertex, fragment], &PipelineConfig {
            culling: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Translucent,
            depth_test: true,
            line_width: 1.0,
        })?;

        let descriptor_sets = DescriptorSets::new(ctx.get().device().clone(), pipeline.descriptor_set_layout())?;

        let mut mesh = DynamicMesh::new(size_of::<ImDrawVert>(), ctx.get().device().clone(), MeshCreateInfos { index_type: IndexBufferType::Uint32 })?;

        let vertices = [Vec3::new(-1f32, -1f32, 0f32), Vec3::new(1f32, -1f32, 0f32), Vec3::new(1f32, 1f32, 0f32), Vec3::new(-1f32, 1f32, 0f32)];
        let sl_vertices = unsafe { slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * size_of::<Vec3>()) };
        let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];
        let sl_indices = unsafe { slice::from_raw_parts(indices.as_ptr() as *const u8, indices.len() * size_of::<u32>()) };
        mesh.set_data(0, sl_vertices, 0, sl_indices)?;

        let mut camera = Camera::default();
        camera.set_position(Vec3::new(0f32, 0f32, 0.5f32));

        Ok(Self {
            pipeline,
            mesh,
            ctx,
            descriptor_sets,
            camera,
            pitch: 0.0,
            yaw: 0.0,
            last_mouse: Default::default(),
        })
    }

    pub fn render(&mut self, command_buffer: &CommandBuffer) -> Result<(), Error> {
        command_buffer.bind_pipeline(&self.pipeline);

        let window = self.ctx.get().window().get();
        let w = window.read();
        let inputs = w.input_manager();
        let ds = self.ctx.get().window().get().read().delta_time;

        let speed = 10f32;

        let mut delta = Vec3::default();
        if inputs.is_key_pressed(&Key::Character(SmolStr::from("z"))) {
            delta += &Vec3::new(ds as f32 * speed, 0f32, 0f32);
        };
        if inputs.is_key_pressed(&Key::Character(SmolStr::from("s"))) {
            delta += &Vec3::new(-ds as f32 * speed, 0f32, 0f32);
        };
        if inputs.is_key_pressed(&Key::Character(SmolStr::from("q"))) {
            delta += &Vec3::new(0f32, ds as f32 * speed, 0f32);
        };
        if inputs.is_key_pressed(&Key::Character(SmolStr::from("d"))) {
            delta += &Vec3::new(0f32, -ds as f32 * speed, 0f32);
        };
        if inputs.is_key_pressed(&Key::Named(NamedKey::Space)) {
            delta += &Vec3::new(0f32, 0f32, ds as f32 * speed);
        };
        if inputs.is_key_pressed(&Key::Named(NamedKey::Shift)) {
            delta += Vec3::new(0f32, 0f32, -ds as f32 * speed);
        };
        self.camera.set_position(self.camera.position() + self.camera.rotation().inverse().mul_vec3(delta));

        let delta = inputs.mouse_position().sub(self.last_mouse);

        if delta.x.abs() < 100f64 && delta.y.abs() < 100f64 {
            self.pitch += delta.y as f32 * 0.01;
            self.yaw += delta.x as f32 * 0.01;
        }
        self.camera.set_rotation_euler(0f32, -self.pitch, self.yaw);

        self.last_mouse = *inputs.mouse_position();


        let perspective = Mat4::perspective_rh(PI / 2f32, self.ctx.get().window().get().read().width()? as f32 / self.ctx.get().window().get().read().height()? as f32, 0.001f32, 10000f32);
        let pc = Pc {
            model: Mat4::IDENTITY,
            camera: perspective.mul_mat4(&self.camera.matrix()),
        };

        command_buffer.push_constant(&self.pipeline, &BufferMemory::from_struct(&pc), vk::ShaderStageFlags::VERTEX);

        command_buffer.set_scissor(Scissors {
            min_x: 0,
            min_y: 0,
            width: self.ctx.get().window().get().read().width()?,
            height: self.ctx.get().window().get().read().height()?,
        });

        command_buffer.bind_descriptors(&self.pipeline, &self.descriptor_sets);

        command_buffer.draw_mesh(&self.mesh, 1, 0);

        Ok(())
    }
}