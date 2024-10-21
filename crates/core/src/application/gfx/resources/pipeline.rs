use crate::application::gfx::device::DeviceSharedData;
use crate::application::gfx::render_pass::RenderPass;
use crate::application::gfx::resources::shader_module::ShaderStage;
use anyhow::Error;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Handle, HasBuilder, ShaderStageFlags};

pub struct Pipeline {
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_set_layout: vk::DescriptorSetLayout,
    ctx: DeviceSharedData
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlphaMode
{
    Opaque,
    Translucent,
    Additive,
}

pub struct PipelineConfig {
    pub culling: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub topology: vk::PrimitiveTopology,
    pub polygon_mode: vk::PolygonMode,
    pub alpha_mode: AlphaMode,
    pub depth_test: bool,
    pub line_width: f32,
}

impl Pipeline {
    pub fn new(ctx: DeviceSharedData, render_pass: &RenderPass, mut stages: Vec<ShaderStage>, config: &PipelineConfig) -> Result<Self, Error> {

        // Push Constant Ranges
        let vert_push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(64);

        let frag_push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(64)
            .size(4);

        // Layout
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
        let descriptor_set_layout = unsafe {ctx.upgrade().device().create_descriptor_set_layout(&ci_descriptor_set_layout, None)}?;

        let set_layouts = &[descriptor_set_layout];
        let push_constant_ranges = &[vert_push_constant_range, frag_push_constant_range];
        let layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(set_layouts)
            .push_constant_ranges(push_constant_ranges);

        let pipeline_layout = unsafe { ctx.upgrade().device().create_pipeline_layout(&layout_info, None) }?;

        let mut vertex_attribute_description = Vec::<vk::VertexInputAttributeDescription>::new();

        let mut vertex_input_size = 0;
        for stage in &stages {
            let infos = stage.infos();
            if infos.stage == ShaderStageFlags::VERTEX {
                for input in &infos.stage_input {
                    vertex_attribute_description.push(vk::VertexInputAttributeDescription::builder()
                        .location(input.location)
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

        let mut color_blend_attachment = Vec::<vk::PipelineColorBlendAttachmentState>::new();

        for _ in 0..render_pass.config().color_attachments.len()
        {
            color_blend_attachment.push(vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(config.alpha_mode != AlphaMode::Opaque)
                .src_color_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ZERO } else { vk::BlendFactor::SRC_ALPHA })
                .dst_color_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ZERO } else { vk::BlendFactor::ONE_MINUS_SRC_ALPHA })
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(if config.alpha_mode == AlphaMode::Opaque { vk::BlendFactor::ONE } else { vk::BlendFactor::ONE_MINUS_SRC_ALPHA })
                .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B | vk::ColorComponentFlags::A)
                .build());
        }

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .attachments(color_blend_attachment.as_slice())
            .build();

        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(config.topology)
            .primitive_restart_enable(false)
            .build();

        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1)
            .build();

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
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

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::_1)
            .sample_shading_enable(false)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .build();


        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(config.depth_test)
            .depth_write_enable(config.depth_test)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0)
            .build();

        // Create descriptor sets
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

        // Create pipeline layout
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

        // Create
        let mut stage_modules = vec![];
        let mut entry_point_names = vec![];
        for stage in &stages {
            entry_point_names.push(format!("{}\0", stage.infos().entry_point));
            stage_modules.push(vk::PipelineShaderStageCreateInfo::builder()
                .stage(stage.infos().stage)
                .module(*stage.shader_module())
                .name(entry_point_names.last().unwrap().as_bytes())
                .build())
        }

        let mut dynamic_states_array = Vec::from([vk::DynamicState::SCISSOR, vk::DynamicState::VIEWPORT]);
        if config.line_width != 1.0 {
            dynamic_states_array.push(vk::DynamicState::LINE_WIDTH);
        }
        let dynamic_states = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states_array.as_slice())
            .build();

        let info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(stage_modules.as_slice())
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_states)
            .layout(pipeline_layout)
            .render_pass(*render_pass.ptr())
            .subpass(0)
            .build();

        let pipeline = unsafe { ctx.upgrade().device().create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) }?.0;
        
        Ok(Self {
            pipeline_layout,
            pipeline: pipeline[0],
            descriptor_set_layout,
            ctx,
        })
    }

    pub fn ptr_pipeline(&self) -> &vk::Pipeline {
        &self.pipeline
    }

    pub fn descriptor_set_layout(&self) -> &vk::DescriptorSetLayout {
        &self.descriptor_set_layout
    }

    pub fn ptr_pipeline_layout(&self) -> &vk::PipelineLayout {
        &self.pipeline_layout
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        let device = self.ctx.upgrade();
        unsafe { device.device().destroy_pipeline_layout(self.pipeline_layout, None); }
        unsafe { device.device().destroy_descriptor_set_layout(self.descriptor_set_layout, None); }
        unsafe { device.device().destroy_pipeline(self.pipeline, None); }
    }
}