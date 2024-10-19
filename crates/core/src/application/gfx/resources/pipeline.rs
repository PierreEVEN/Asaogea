use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::shader_module::ShaderStage;
use anyhow::{anyhow, Error};
use vulkanalia::vk::{DeviceV1_0, HasBuilder, ShaderStageFlags, SuccessCode};
use vulkanalia::vk;

pub struct Pipeline {
    pipeline_layout: Option<vk::PipelineLayout>,
    pipeline: Option<vk::Pipeline>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlphaMode
{
    Opaque,
    Translucent,
    Additive,
}

pub struct PipelineConfig {
    pub shader_version: String,
    pub culling: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub topology: vk::PrimitiveTopology,
    pub polygon_mode: vk::PolygonMode,
    pub alpha_mode: AlphaMode,
    pub depth_test: bool,
    pub line_width: f32,
}

impl Pipeline {
    pub fn new(device: &vulkanalia::Device, render_pass: &RenderPass, mut stages: Vec<ShaderStage>, config: &PipelineConfig) -> Result<Self, Error> {
        let mut bindings = Vec::<vk::DescriptorSetLayoutBinding>::new();
        for stage in &stages {
            for binding in &stage.infos().descriptor_bindings {
                bindings.push(vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding.binding)
                    .descriptor_type(binding.descriptor_type)
                    .descriptor_count(1)
                    .stage_flags(stage.infos().stage)
                    .build());
            }
        }

        let ci_descriptor_set_layout = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(bindings.as_slice())
            .build();

        let descriptor_set_layout = unsafe {device.create_descriptor_set_layout(&ci_descriptor_set_layout, None)}?;

        let mut push_constants = Vec::<vk::PushConstantRange>::new();
        for stage in &stages {
            if let Some(push_constant_size) = stage.infos().push_constant_size {
                push_constants.push(vk::PushConstantRange::builder()
                    .stage_flags(stage.infos().stage)
                    .offset(0)
                    .size(push_constant_size)
                    .build());
            }
        }

        let pipeline_layout_infos = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&[descriptor_set_layout])
            .push_constant_ranges(push_constants.as_slice())
            .build();
        let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_infos, None) }?;

        let mut vertex_attribute_description = Vec::<vk::VertexInputAttributeDescription>::new();

        let mut vertex_input_size = 0;
        for stage in &stages {
            let infos = stage.infos();
            if infos.stage == ShaderStageFlags::VERTEX {

                for input in &infos.stage_input {
                    if input.location == 0 {
                        continue;
                    }
                    vertex_attribute_description.push(vk::VertexInputAttributeDescription::builder()
                        .location(input.location as _)
                        .format(input.property_type)
                        .offset(input.offset)
                        .build());
                    vertex_input_size += input.input_size
                }

                break;
            }
        }

        let mut binding_descriptions = Vec::new();
        if vertex_input_size > 0 {
            binding_descriptions.push(vk::VertexInputBindingDescription::builder()
                .binding(0)
                .stride(vertex_input_size)
                .input_rate(vk::VertexInputRate::VERTEX)
                .build());
        }

        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(binding_descriptions.as_slice())
            .vertex_attribute_descriptions(vertex_attribute_description.as_slice())
            .build();

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(config.topology)
            .primitive_restart_enable(false)
            .build();

        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1)
            .build();

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(config.polygon_mode)
            .cull_mode(config.culling)
            .front_face(config.front_face)
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .line_width(config.line_width)
            .build();

        let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::_1)
            .sample_shading_enable(false)
            .min_sample_shading(1.0)
            .sample_mask(&[])
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .build();


        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(config.depth_test)
            .depth_write_enable(config.depth_test)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0)
            .build();

        let mut color_blend_attachment = Vec::<vk::PipelineColorBlendAttachmentState>::new();

        for _ in &render_pass.config().color_attachments
        {
            color_blend_attachment.push(vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(if config.alpha_mode == AlphaMode::Opaque { false } else { true })
                .src_color_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ZERO } else { vk::BlendFactor::SRC_ALPHA })
                .dst_color_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ZERO } else { vk::BlendFactor::ONE_MINUS_SRC_ALPHA })
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ONE } else { vk::BlendFactor::ONE_MINUS_SRC_ALPHA })
                .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B | vk::ColorComponentFlags::A)
                .build());
        }


        let mut stage_modules = vec![];
        for stage in &stages {
            stage_modules.push(vk::PipelineShaderStageCreateInfo::builder()
                                   .stage(stage.infos().stage)
                                   .module(*stage.shader_module())
                                   .name(stage.infos().entry_point.as_bytes())
                                   .build())
        }

        let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
            .attachments(color_blend_attachment.as_slice())
            .build();


        let mut dynamic_states_array = Vec::from([vk::DynamicState::SCISSOR, vk::DynamicState::VIEWPORT]);
        if config.line_width != 1.0 {
            dynamic_states_array.push(vk::DynamicState::LINE_WIDTH);
        }

        let dynamic_states = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states_array.as_slice())
            .build();

        let ci_pipeline = vk::GraphicsPipelineCreateInfo::builder()
            .stages(stage_modules.as_slice())
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_states)
            .layout(pipeline_layout)
            .render_pass(*render_pass.ptr()?)
            .subpass(0)
            .base_pipeline_handle(vk::Pipeline::default())
            .base_pipeline_index(-1)
            .build();

        for mut stage in &mut stages {
            stage.destroy(device);
        }

        let (pipeline, success_code) = unsafe { device.create_graphics_pipelines(vk::PipelineCache::default(), &[ci_pipeline], None) }?;

        if success_code != SuccessCode::SUCCESS || pipeline.len() != 1 {
            return Err(anyhow!("Failed to create pipeline : {:?}", success_code))
        }
        
        Ok(Self {
            pipeline_layout: Some(pipeline_layout),
            pipeline: Some(pipeline[0])
        })
    }

    pub fn destroy(&mut self, device: &vulkanalia::Device) {
        unsafe { device.destroy_pipeline_layout(self.pipeline_layout.take().expect("Shader module have already been destroyed"), None); }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        if self.pipeline_layout.is_some() {
            panic!("Pipeline have not been destroyed using Pipeline::destroy()");
        }
    }
}