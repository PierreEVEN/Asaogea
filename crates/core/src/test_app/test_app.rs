use anyhow::Error;
use imgui::sys::ImDrawVert;
use vulkanalia::vk;
use vulkanalia::vk::DeviceV1_0;
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use crate::application::gfx::imgui::ImGuiPushConstants;
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::descriptor_sets::DescriptorSets;
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType, MeshCreateInfos};
use crate::application::gfx::resources::pipeline::{AlphaMode, Pipeline, PipelineConfig};
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageBindings, ShaderStageInfos, ShaderStageInputs};
use crate::application::gfx::swapchain::SwapchainCtx;
const PIXEL: &str = r#"
struct VSInput {
    [[vk::location(0)]] float2 aPos 	: POSITION;
    [[vk::location(1)]] float2 aUV 		: TEXCOORD;
    [[vk::location(2)]] float4 aColor 	: COLOR;
};
struct VsToFs {
    float4 Pos 		: SV_Position;
    float4 Color 	: COLOR;
    float2 UV 	 	: TEXCOORD;
};
struct PushConsts {
    float2 uScale;
    float2 uTranslate;
};
[[vk::push_constant]] ConstantBuffer<PushConsts> pc;
VsToFs main(VSInput input) {
    VsToFs Out;
    Out.Color	= input.aColor;
    Out.UV 		= input.aUV;
    Out.Pos 	= float4(input.aPos * pc.uScale + pc.uTranslate, 0, 1);
    return Out;
}
"#;

const FRAGMENT: &str = r#"
struct VsToFs {
    float4 Pos 		: SV_Position;
    float4 Color 	: COLOR;
    float2 UV 	 	: TEXCOORD;
};
[[vk::binding(0)]]   Texture2D	 sTexture;
[[vk::binding(1)]]   SamplerState sSampler;

float4 main(VsToFs input) : SV_TARGET {
    return input.Color * sTexture.Sample(sSampler, input.UV);
}
"#;

pub struct TestApp {
    pipeline: Pipeline,
    mesh: DynamicMesh,
    ctx: SwapchainCtx,
    descriptor_sets: DescriptorSets,
    
}


impl TestApp {
    
    pub fn new(ctx: SwapchainCtx, render_pass: &RenderPass) -> Result<Self, Error> {
        let mut compiler = HlslCompiler::new()?;
        let vertex = compiler.compile(&RawShaderDefinition::new("imgui-vertex", "vs_6_0", PIXEL.to_string()))?;
        let fragment = compiler.compile(&RawShaderDefinition::new("imgui-fragment", "ps_6_0", FRAGMENT.to_string()))?;

        let vertex = ShaderStage::new(ctx.get().device().clone(), &vertex.raw(), ShaderStageInfos {
            descriptor_bindings: vec![],
            push_constant_size: Some(size_of::<ImGuiPushConstants>() as u32),
            stage_input: vec![
                ShaderStageInputs {
                    location: 0,
                    offset: 0,
                    input_size: 8,
                    property_type: vk::Format::R32G32_SFLOAT,
                },
                ShaderStageInputs {
                    location: 1,
                    offset: 8,
                    input_size: 8,
                    property_type: vk::Format::R32G32_SFLOAT,
                },
                ShaderStageInputs {
                    location: 2,
                    offset: 16,
                    input_size: 4,
                    property_type: vk::Format::R8G8B8A8_UNORM,
                }],
            stage: vk::ShaderStageFlags::VERTEX,
            entry_point: "main".to_string(),
        })?;
        let fragment = ShaderStage::new(ctx.get().device().clone(), &fragment.raw(),
                                        ShaderStageInfos {
                                            descriptor_bindings: vec![
                                                ShaderStageBindings {
                                                    binding: 0,
                                                    descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
                                                },
                                                ShaderStageBindings {
                                                    binding: 1,
                                                    descriptor_type: vk::DescriptorType::SAMPLER,
                                                }],
                                            push_constant_size: None,
                                            stage_input: vec![],
                                            stage: vk::ShaderStageFlags::FRAGMENT,
                                            entry_point: "main".to_string(),
                                        })?;


        let pipeline = Pipeline::new(ctx.get().device().clone(), render_pass, vec![vertex, fragment], &PipelineConfig {
            culling: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Translucent,
            depth_test: true,
            line_width: 1.0,
        })?;

        let mut descriptor_sets = DescriptorSets::new(ctx.get().device().clone(), pipeline.descriptor_set_layout())?;

        let mesh = DynamicMesh::new(size_of::<ImDrawVert>(), ctx.get().device().clone(), MeshCreateInfos {
            index_type: IndexBufferType::Uint32,
        })?;
        Ok(Self {
            pipeline,
            mesh,
            ctx,
            descriptor_sets
        })        
    }
    
    pub fn render(&self, command_buffer: &vk::CommandBuffer) -> Result<(), Error> {

        let device = self.ctx.get().device().get();
        let device_vulkan = device.device();
        
        unsafe {
            device_vulkan.cmd_bind_pipeline(
                *command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                *self.pipeline.ptr_pipeline(),
            );
        }

        unsafe {
            device_vulkan.cmd_bind_descriptor_sets(
                *command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                *self.pipeline.ptr_pipeline_layout(),
                0,
                &[*self.descriptor_sets.ptr()?],
                &[],
            );
        }

        // Draw mesh
        unsafe {
            device_vulkan.cmd_bind_index_buffer(
                *command_buffer,
                *self.mesh.index_buffer()?.ptr()?,
                0 as vk::DeviceSize,
                vk::IndexType::UINT16);
            device_vulkan.cmd_bind_vertex_buffers(
                *command_buffer,
                0,
                &[*self.mesh.vertex_buffer()?.ptr()?],
                &[0]);
            
            device_vulkan.cmd_draw_indexed(*command_buffer, self.mesh.index_buffer()?.size() as u32, 1, 0, 0, 0);
        }
        
        Ok(())
    }
}