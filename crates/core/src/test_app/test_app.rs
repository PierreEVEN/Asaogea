use crate::application::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::descriptor_sets::DescriptorSets;
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType};
use crate::application::gfx::resources::pipeline::{AlphaMode, Pipeline, PipelineConfig};
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageInfos, ShaderStageInputs};
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
use vulkanalia::vk;
use winit::keyboard::{Key, NamedKey, SmolStr};

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

float4 main(VsToFs input) : SV_TARGET {

    

    float intens = (pow_cord(input.vtxpos.x) + pow_cord(input.vtxpos.y) + pow_cord(input.vtxpos.z)) * 0.4 + 0.1;
    
    return float4(float3(intens, intens, intens), 1);
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
            alpha_mode: AlphaMode::Opaque,
            depth_test: true,
            line_width: 1.0,
        })?;

        let descriptor_sets = DescriptorSets::new(ctx.get().device().clone(), pipeline.descriptor_set_layout())?;

        let mut camera = Camera::default();
        camera.set_position(Vec3::new(0f32, 0f32, 0.5f32));


        let Gltf { document, blob } = Gltf::open("resources/models/sample/Lantern.glb")?;
        let buffers = Self::import_buffer_data(&document, Some(PathBuf::from("resources/models/sample/scifihelmet/").as_path()), blob)?;


        let mut dyn_mesh = DynamicMesh::new(size_of::<Vec3>(), ctx.get().device().clone())?;

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let mut vertices = vec![];

                let reader = primitive.reader(|data| Some(&buffers[data.index()]));
                let positions = reader.read_positions().unwrap();
                vertices.reserve(positions.len());
                for position in positions {
                    vertices.push(position);
                }

                match reader.read_indices() {
                    None => {
                        dyn_mesh.set_vertices(0, &BufferMemory::from_vec(&vertices))?;
                    }
                    Some(indices) => {
                        match indices {
                            ReadIndices::U8(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint8);
                                let indices: Vec<u8> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                            ReadIndices::U16(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint16);
                                let indices: Vec<u16> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                            ReadIndices::U32(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint32);
                                let indices: Vec<u32> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                        }
                    }
                }

            }
        }


        Ok(Self {
            pipeline,
            mesh: dyn_mesh,
            ctx,
            descriptor_sets,
            camera,
            pitch: 0.0,
            yaw: 0.0,
            speed: 2f32,
            last_mouse: Default::default(),
        })
    }

    fn import_buffer_data(document: &gltf::Document, base: Option<&Path>, mut blob: Option<Vec<u8>>) -> Result<Vec<gltf::buffer::Data>, Error> {
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let mut data = match buffer.source() {
                gltf::buffer::Source::Uri(uri) => Scheme::read(base, uri),
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


/// Represents the set of URI schemes the importer supports.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Scheme<'a> {
    /// `data:[<media type>];base64,<data>`.
    Data(Option<&'a str>, &'a str),

    /// `file:[//]<absolute file path>`.
    ///
    /// Note: The file scheme does not implement authority.
    File(&'a str),

    /// `../foo`, etc.
    Relative,

    /// Placeholder for an unsupported URI scheme identifier.
    Unsupported,
}

impl<'a> Scheme<'a> {
    fn parse(uri: &str) -> Scheme<'_> {
        if uri.contains(':') {
            if let Some(rest) = uri.strip_prefix("data:") {
                let mut it = rest.split(";base64,");

                match (it.next(), it.next()) {
                    (match0_opt, Some(match1)) => Scheme::Data(match0_opt, match1),
                    (Some(match0), _) => Scheme::Data(None, match0),
                    _ => Scheme::Unsupported,
                }
            } else if let Some(rest) = uri.strip_prefix("file://") {
                Scheme::File(rest)
            } else if let Some(rest) = uri.strip_prefix("file:") {
                Scheme::File(rest)
            } else {
                Scheme::Unsupported
            }
        } else {
            Scheme::Relative
        }
    }

    fn read(base: Option<&Path>, uri: &str) -> Result<Vec<u8>, Error> {
        match Scheme::parse(uri) {
            // The path may be unused in the Scheme::Data case
            // Example: "uri" : "data:application/octet-stream;base64,wsVHPgA...."
            Scheme::Data(_, base64) => base64::decode(&base64).map_err(|err| anyhow!("Failed to decode base64 : {err}")),
            Scheme::File(path) if base.is_some() => Self::read_to_end(path),
            Scheme::Relative if base.is_some() => Self::read_to_end(base.unwrap().join(uri)),
            Scheme::Unsupported => Err(anyhow!("Unsupported data type")),
            _ => Err(anyhow!("Unknown data type")),
        }
    }

    fn read_to_end<P>(path: P) -> Result<Vec<u8>, Error>
    where
        P: AsRef<Path>,
    {
        use std::io::Read;
        let file = File::open(path.as_ref())?;
        // Allocate one extra byte so the buffer doesn't need to grow before the
        // final `read` call at the end of the file.  Don't worry about `usize`
        // overflow because reading will fail regardless in that case.
        let length = file.metadata().map(|x| x.len() + 1).unwrap_or(0);
        let mut reader = BufReader::new(file);
        let mut data = Vec::with_capacity(length as usize);
        reader.read_to_end(&mut data)?;
        Ok(data)
    }
}
