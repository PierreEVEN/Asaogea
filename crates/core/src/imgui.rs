use crate::device::Device;
use crate::shader_module::ShaderModule;
use anyhow::Error;
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use vulkanalia::vk;
use vulkanalia::vk::HasBuilder;
use crate::pipeline::Pipeline;

pub struct ImGui {
    compiler: HlslCompiler
}

const PIXEL: &str = r#"
struct VsToFs {
    float4 Pos 		: SV_Position;
    float4 Color 	: COLOR;
    float2 UV 	 	: TEXCOORD;
};
struct VSInput {
    float2 aPos 	: POSITION;
    float2 aUV 		: TEXCOORD;
    float4 aColor 	: COLOR;
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
Texture2D	 sTexture;
SamplerState sSampler;

float4 main(VsToFs input) : SV_TARGET {
    return input.Color * sTexture.Sample(sSampler, input.UV);
}
"#;

impl ImGui {

    pub fn new(device: &Device) -> Result<Self, Error> {
        let mut compiler = HlslCompiler::new()?;

        let vertex = compiler.compile(&RawShaderDefinition::new("imgui-vertex", "vs_6_0", PIXEL.to_string()))?;
        let fragment = compiler.compile(&RawShaderDefinition::new("imgui-fragment", "ps_6_0", FRAGMENT.to_string()))?;

        let vertex = ShaderModule::new(device.ptr(), &vertex.raw())?;
        let fragment = ShaderModule::new(device.ptr(), &fragment.raw())?;

        let vert_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(*vertex)
            .name(b"main\0");

        let frag_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(*vertex)
            .name(b"main\0");

        let mut pipeline = Pipeline::new(device.ptr(), vertex, fragment)?;
        pipeline.destroy(device.ptr());



        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder();
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false);
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::_1);

        let attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::ONE)  // Optional
            .dst_color_blend_factor(vk::BlendFactor::ZERO) // Optional
            .color_blend_op(vk::BlendOp::ADD)              // Optional
            .src_alpha_blend_factor(vk::BlendFactor::ONE)  // Optional
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO) // Optional
            .alpha_blend_op(vk::BlendOp::ADD);             // Optional

        let attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);

        let attachments = &[attachment];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);

        let dynamic_states = &[
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::LINE_WIDTH,
        ];

        let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states);

        Ok(Self {
            compiler
        })
    }


}