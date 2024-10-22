use crate::application::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::descriptor_sets::{DescriptorSets, ShaderInstanceBinding};
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType};
use crate::application::gfx::resources::pipeline::{AlphaMode, Pipeline, PipelineConfig};
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageBindings, ShaderStageInfos, ShaderStageInputs};
use crate::application::gfx::swapchain::SwapchainCtx;
use crate::test_app::camera::Camera;
use anyhow::{anyhow, Error};
use glam::{DVec2, Mat4, Vec3};
use gltf::mesh::util::ReadIndices;
use gltf::Gltf;
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use std::f32::consts::PI;
use std::fs::File;
use std::io::BufReader;
use std::ops::Sub;
use std::path::{Path, PathBuf};
use image::{ColorType, DynamicImage, GenericImageView};
use image::ImageFormat::{Jpeg, Png};
use vulkanalia::vk;
use winit::keyboard::{Key, NamedKey, SmolStr};
use crate::application::gfx::resources::image::{Image, ImageCreateOptions};
use crate::application::gfx::resources::sampler::Sampler;

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
    return sTexture.Sample(sSampler, input.vtxpos.xy) + float4(float3(intens, intens, intens), 1);
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
    speed: f32,
    last_mouse: DVec2,
    images: Vec<Image>,
    sampler: Sampler,
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
                                            descriptor_bindings: vec![
                                                ShaderStageBindings { binding: 0, descriptor_type: vk::DescriptorType::SAMPLER },
                                                ShaderStageBindings { binding: 1, descriptor_type: vk::DescriptorType::SAMPLED_IMAGE },
                                            ],
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
            alpha_mode: AlphaMode::Opaque,
            depth_test: true,
            line_width: 1.0,
        })?;

        let mut descriptor_sets = DescriptorSets::new(ctx.get().device().clone(), pipeline.descriptor_set_layout())?;

        let mut camera = Camera::default();
        camera.set_position(Vec3::new(0f32, 0f32, 0.5f32));


        let Gltf { document, blob } = Gltf::open("resources/models/sample/Lantern.glb")?;
        let buffers = Self::import_buffer_data(&document, Some(PathBuf::from("resources/models/sample/").as_path()), blob)?;

        let mut images = vec![];

        for texture in document.textures() {
            let data = Self::import_image_data(&document, Some(PathBuf::from("resources/models/sample/").as_path()), &buffers, texture.index())?;
            println!("load : {:?}", data.color());
            if data.color() == ColorType::Rgb8 { continue; }

            let mut image = Image::new(ctx.get().device().clone(), ImageCreateOptions {
                image_type: vk::ImageType::_2D,
                format: if data.color() == ColorType::Rgb8 { vk::Format::R8G8B8_UNORM } else { vk::Format::R8G8B8A8_UNORM },
                usage: vk::ImageUsageFlags::SAMPLED,
                width: data.width(),
                height: data.height(),
                depth: 1,
                mips_levels: 1,
                is_depth: false,
            })?;

            image.set_data(&BufferMemory::from_slice(data.as_bytes()))?;
            images.push(image);
        }

        let sampler = Sampler::new(ctx.get().device().clone())?;

        descriptor_sets.update(vec![
            (ShaderInstanceBinding::Sampler(*sampler.ptr()), 0),
            (ShaderInstanceBinding::SampledImage(*images[0].view()?, *images[0].layout()), 1)
        ])?;


        Ok(Self {
            pipeline,
            ctx,
            descriptor_sets,
            camera,
            pitch: 0.0,
            yaw: 0.0,
            speed: 2f32,
            last_mouse: Default::default(),
            images,
            sampler,
        })
    }

    fn import_buffer_data(document: &gltf::Document, base: Option<&Path>, mut blob: Option<Vec<u8>>) -> Result<Vec<gltf::buffer::Data>, Error> {
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let mut data = match buffer.source() {
                gltf::buffer::Source::Uri(uri) => todo!(),//Scheme::read(base, uri),
                gltf::buffer::Source::Bin => blob.take().ok_or(anyhow!("Missing bin data")),
            }?;
            if data.len() < buffer.length() {
                return Err(anyhow!("Buffer too short"));
            }
            while data.len() % 4 != 0 {
                data.push(0);
            }
            buffers.push(gltf::buffer::Data(data));
        }
        Ok(buffers)
    }

    pub fn import_image_data(
        document: &gltf::Document,
        base: Option<&Path>,
        buffer_data: &[gltf::buffer::Data],
        image_index: usize,
    ) -> Result<DynamicImage, Error> {
        let guess_format = |encoded_image: &[u8]| match image::guess_format(encoded_image) {
            Ok(Png) => Some(Png),
            Ok(Jpeg) => Some(Jpeg),
            _ => None,
        };
        /*
        let result_image;
        let document_image = document.images().nth(image_index).unwrap();
        match document_image.source() {
            gltf::image::Source::Uri { uri, mime_type } if base.is_some() => {
                match Scheme::parse(uri) {
                    Scheme::Data(Some(annoying_case), base64) => {
                        let encoded_image = base64::decode(&base64)?;
                        let encoded_format = match annoying_case {
                            "image/png" => Png,
                            "image/jpeg" => Jpeg,
                            _ => match guess_format(&encoded_image) {
                                Some(format) => format,
                                None => return Err(anyhow!("unknown format")),
                            },
                        };
                        let decoded_image = image::load_from_memory_with_format(
                            &encoded_image,
                            encoded_format,
                        )?;
                        return Ok(decoded_image);
                    }
                    Scheme::Unsupported => return Err(anyhow!("unknown format")),
                    _ => {}
                }
                let encoded_image = Scheme::read(base, uri)?;
                let encoded_format = match mime_type {
                    Some("image/png") => Png,
                    Some("image/jpeg") => Jpeg,
                    Some(_) => match guess_format(&encoded_image) {
                        Some(format) => format,
                        None => return Err(anyhow!("unknown format")),
                    },
                    None => match uri.rsplit('.').next() {
                        Some("png") => Png,
                        Some("jpg") | Some("jpeg") => Jpeg,
                        _ => match guess_format(&encoded_image) {
                            Some(format) => format,
                            None => return Err(anyhow!("unknown format")),
                        },
                    },
                };
                let decoded_image = image::load_from_memory_with_format(&encoded_image, encoded_format)?;
                result_image = decoded_image;
            }
            gltf::image::Source::View { view, mime_type } => {
                let parent_buffer_data = &buffer_data[view.buffer().index()].0;
                let begin = view.offset();
                let end = begin + view.length();
                let encoded_image = &parent_buffer_data[begin..end];
                let encoded_format = match mime_type {
                    "image/png" => Png,
                    "image/jpeg" => Jpeg,
                    _ => match guess_format(encoded_image) {
                        Some(format) => format,
                        None => return Err(anyhow!("unknown format")),
                    },
                };
                let decoded_image =
                    image::load_from_memory_with_format(encoded_image, encoded_format)?;
                result_image = decoded_image;
            }
            _ => return Err(anyhow!("unknown format")),
        }
        Ok(result_image)
        
         */
        todo!()
    }

    pub fn render(&mut self, command_buffer: &CommandBuffer) -> Result<(), Error> {
        command_buffer.bind_pipeline(&self.pipeline);

        let window = self.ctx.get().window().get();
        let w = window.read();
        let inputs = w.input_manager();
        let ds = self.ctx.get().window().get().read().delta_time;

        let speed = self.speed;

        let mut delta = Vec3::default();

        self.speed *= inputs.scroll_delta().y as f32 * 0.25f32 + 1f32;

        if inputs.is_key_pressed(&Key::Named(NamedKey::PageUp)) {
            self.speed += 1f32 * ds as f32;
        };

        if inputs.is_key_pressed(&Key::Named(NamedKey::PageDown)) {
            self.speed -= 1f32 * ds as f32;
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
