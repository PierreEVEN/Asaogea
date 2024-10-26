use crate::application::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::descriptor_sets::{DescriptorSets, ShaderInstanceBinding};
use crate::application::gfx::resources::image::{Image, ImageCreateOptions};
use crate::application::gfx::resources::mesh::DynamicMesh;
use crate::application::gfx::resources::pipeline::{AlphaMode, Pipeline, PipelineConfig};
use crate::application::gfx::resources::sampler::Sampler;
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageBindings, ShaderStageInfos, ShaderStageInputs};
use crate::application::gfx::swapchain::SwapchainCtx;
use crate::assets::gltf_importer::gltf_importer::GltfImporter;
use crate::test_app::camera::Camera;
use anyhow::Error;
use glam::{DVec2, Mat4, Vec3};
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use std::f32::consts::PI;
use std::ops::Sub;
use std::path::PathBuf;
use tracing::info;
use vulkanalia::vk;
use winit::keyboard::{Key, NamedKey, SmolStr};
use job_sys::{Job, JobSystem};
use types::rwarc::RwArc;
use crate::application::gfx::frame_graph::frame_graph::RenderPass;

const PIXEL: &str = r#"
struct VSInput {
    [[vk::location(0)]] float3 aPos 	: POSITION;
};
struct VsToFs {
    float4 Pos 		: SV_Position;
    float4 vtxpos : POSITION0;
};
struct PushConsts {
    float4x4 model;
    float4x4 camera;
};
[[vk::push_constant]] ConstantBuffer<PushConsts> pc;
VsToFs main(VSInput input) {
    VsToFs Out;
    Out.Pos 	= mul(pc.camera, mul(pc.model, float4(input.aPos, 1)));
    Out.vtxpos = float4(input.aPos, 1);
    return Out;
}
"#;

const FRAGMENT: &str = r#"
struct VsToFs {
    float4 Pos 		: SV_Position;
    float4 vtxpos : POSITION0;
};

float pow_cord(float val) {
    return pow(abs(sin(val * 50)), 10);
}

[[vk::binding(0)]]   SamplerState sSampler;
[[vk::binding(1)]]   Texture2D	 sTexture;

float4 main(VsToFs input) : SV_TARGET {
    float intens = ((pow_cord(input.vtxpos.x) + pow_cord(input.vtxpos.y) + pow_cord(input.vtxpos.z)) * 0.4 + 0.1) * 0.01;
    return sTexture.Sample(sSampler, input.vtxpos.xy * 0.01) + float4(float3(intens, intens, intens), 1);
}
"#;

pub struct TestApp {
    pipeline: Pipeline,
    meshes: Vec<DynamicMesh>,
    ctx: SwapchainCtx,
    descriptor_sets: Vec<DescriptorSets>,
    camera: Camera,
    pitch: f32,
    yaw: f32,
    speed: f32,
    last_mouse: DVec2,
    _images: Vec<Image>,
    _sampler: Sampler,
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

        let vertex = ShaderStage::new(ctx.device().clone(), &vertex.raw(), ShaderStageInfos {
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
        let fragment = ShaderStage::new(ctx.device().clone(), &fragment.raw(),
                                        ShaderStageInfos {
                                            descriptor_bindings: vec![
                                                ShaderStageBindings { binding: 0, descriptor_type: vk::DescriptorType::SAMPLER },
                                                ShaderStageBindings { binding: 1, descriptor_type: vk::DescriptorType::SAMPLED_IMAGE },
                                            ],
                                            push_constant_size: None,
                                            stage_input: vec![],
                                            stage: vk::ShaderStageFlags::FRAGMENT,
                                            entry_point: "main".to_string(),
                                        })?;

        let pipeline = Pipeline::new(ctx.device().clone(), render_pass, vec![vertex, fragment], &PipelineConfig {
            culling: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Opaque,
            depth_test: true,
            line_width: 1.0,
        })?;

        let mut camera = Camera::default();
        camera.set_position(Vec3::new(0f32, 0f32, 0.5f32));

        let gltf = RwArc::new(GltfImporter::new(PathBuf::from("resources/models/samples/Sponza/glTF/Sponza.gltf"))?);

        let mut images_handles = vec![];

        let mut images = vec![];
        {
            let js = JobSystem::new(JobSystem::num_cpus());

            let device = ctx.device().clone();

            let num_images = gltf.read().num_images();
            for i in 0..num_images {
                let ctx = device.clone();
                let gltf = gltf.clone();
                images_handles.push((js.push(Job::new(move || {
                    Image::from_dynamic_image(ctx, &gltf.read().load_image(i)?, ImageCreateOptions {
                        usage: vk::ImageUsageFlags::SAMPLED,
                        mips_levels: 1,
                        is_depth: false,
                        ..Default::default()
                    })
                })), i));
            }

            for res in images_handles {
                let image = res.0.wait().unwrap()?;
                images.push(image);
                info!("Load image {} / {}", res.1, num_images);
            }
        }
        let sampler = Sampler::new(ctx.device().clone())?;

        let mut descriptor_sets = vec![];

        for image in &images {
            let mut descriptor_set = DescriptorSets::new(ctx.device().clone(), pipeline.descriptor_set_layout())?;
            descriptor_set.update(vec![
                (ShaderInstanceBinding::Sampler(*sampler.ptr()), 0),
                (ShaderInstanceBinding::SampledImage(*image.view()?, *image.layout()), 1)
            ])?;
            descriptor_sets.push(descriptor_set)
        }
        let mut meshes = vec![];

        for mesh in gltf.write().get_meshes()? {
            for primitive in mesh {
                let mut temp_mesh = DynamicMesh::new(size_of::<Vec3>(), ctx.device().clone())?;
                if let Some(index_buffer) = &primitive.index {
                    temp_mesh.set_indexed_vertices(0, &primitive.vertex, 0, index_buffer)?;
                }
                meshes.push(temp_mesh);
            }
        }

        Ok(Self {
            pipeline,
            meshes,
            ctx,
            descriptor_sets,
            camera,
            pitch: 0.0,
            yaw: 0.0,
            speed: 2f32,
            last_mouse: Default::default(),
            _images: images,
            _sampler: sampler,
        })
    }

    pub fn render(&mut self, command_buffer: &CommandBuffer) -> Result<(), Error> {
        command_buffer.bind_pipeline(&self.pipeline);

        let w = self.ctx.window();
        let inputs = w.input_manager();
        let ds = self.ctx.window().delta_time;

        let speed = self.speed;

        let mut delta = Vec3::default();

        self.speed *= inputs.scroll_delta().y as f32 * 0.25f32 + 1f32;

        if inputs.is_key_pressed(&Key::Named(NamedKey::PageUp)) {
            self.speed += 50f32 * ds as f32;
        };

        if inputs.is_key_pressed(&Key::Named(NamedKey::PageDown)) {
            self.speed -= 50f32 * ds as f32;
            if self.speed < 0.1 {
                self.speed = 0.1;
            }
        };

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


        let perspective = Mat4::perspective_rh(PI / 2f32, self.ctx.window().width()? as f32 / self.ctx.window().height()? as f32, 0.001f32, 10000f32);
        command_buffer.push_constant(&self.pipeline, &BufferMemory::from_struct(Pc {
            model: Mat4::IDENTITY,
            camera: perspective.mul_mat4(&self.camera.matrix()),
        }), vk::ShaderStageFlags::VERTEX);

        command_buffer.set_scissor(Scissors {
            min_x: 0,
            min_y: 0,
            width: self.ctx.window().width()?,
            height: self.ctx.window().height()?,
        });

        for (i, mesh) in self.meshes.iter().enumerate() {
            command_buffer.bind_descriptors(&self.pipeline, &self.descriptor_sets[i % self.descriptor_sets.len()]);

            command_buffer.draw_mesh(mesh, 1, 0);
        }
        Ok(())
    }
}
