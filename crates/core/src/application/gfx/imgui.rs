use crate::application::gfx::render_pass::{RenderPass, RenderPassAttachment, RenderPassCreateInfos};
use crate::application::gfx::resources::descriptor_sets::{DescriptorSets, ShaderInstanceBinding};
use crate::application::gfx::resources::image::{Image, ImageCreateOptions};
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType};
use crate::application::gfx::resources::pipeline::AlphaMode;
use crate::application::gfx::resources::pipeline::{Pipeline, PipelineConfig};
use crate::application::gfx::resources::sampler::Sampler;
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageBindings, ShaderStageInfos, ShaderStageInputs};
use anyhow::Error;
use imgui::sys::{igCreateContext, igEndFrame, igGetDrawData, igGetIO, igGetMainViewport, igGetStyle, igNewFrame, igRender, igShowDemoWindow, igStyleColorsDark, ImDrawIdx, ImDrawVert, ImFontAtlas_GetTexDataAsRGBA32, ImGuiBackendFlags_HasMouseCursors, ImGuiBackendFlags_HasSetMousePos, ImGuiBackendFlags_PlatformHasViewports, ImGuiConfigFlags_DockingEnable, ImGuiConfigFlags_NavEnableGamepad, ImGuiConfigFlags_NavEnableKeyboard, ImGuiConfigFlags_ViewportsEnable, ImVec2, ImVec4};
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use std::ffi::c_char;
use std::ptr::null_mut;
use std::slice;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, ImageType};
use winit::event::MouseButton;
use crate::application::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::swapchain::{SwapchainCtx, SwapchainData};

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

pub struct ImGui {
    _compiler: HlslCompiler,
    mesh: DynamicMesh,
    render_pass: RenderPass,
    pipeline: Pipeline,
    descriptor_sets: DescriptorSets,
    font_texture: Image,
    sampler: Sampler,
    ctx: SwapchainCtx,
}

pub struct ImGuiPushConstants {
    _scale_x: f32,
    _scale_y: f32,
    _translate_x: f32,
    _translate_y: f32,
}

impl ImGui {
    pub fn new(ctx: SwapchainCtx) -> Result<Self, Error> {
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

        let render_pass = RenderPass::new(ctx.get().device().clone(), RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: None,
                image_format: vk::Format::B8G8R8A8_SRGB,
            }],
            depth_attachment: None,
            is_present_pass: false,
        })?;

        let pipeline = Pipeline::new(ctx.get().device().clone(), &render_pass, vec![vertex, fragment], &PipelineConfig {
            culling: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Translucent,
            depth_test: true,
            line_width: 1.0,
        })?;

        unsafe { igCreateContext(null_mut()) };


        let io = unsafe { &mut *igGetIO() };
        io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard as i32;
        io.ConfigFlags |= ImGuiConfigFlags_NavEnableGamepad as i32;
        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable as i32;
        io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable as i32;

        io.BackendPlatformUserData = null_mut();
        io.BackendPlatformName = "imgui backend".as_ptr() as *const c_char;
        io.BackendFlags |= ImGuiBackendFlags_HasMouseCursors as i32;
        io.BackendFlags |= ImGuiBackendFlags_HasSetMousePos as i32;
        io.BackendFlags |= ImGuiBackendFlags_PlatformHasViewports as i32;
        io.MouseHoveredViewport = 0;

        let style = unsafe { &mut *igGetStyle() };
        unsafe { igStyleColorsDark(igGetStyle()) };
        style.WindowRounding = 0.0;
        style.ScrollbarRounding = 0.0;
        style.TabRounding = 0.0;
        style.WindowBorderSize = 1.0;
        style.PopupBorderSize = 1.0;
        style.WindowTitleAlign.x = 0.5;
        style.FramePadding.x = 6.0;
        style.FramePadding.y = 6.0;
        style.WindowPadding.x = 4.0;
        style.WindowPadding.y = 4.0;
        style.GrabMinSize = 16.0;
        style.ScrollbarSize = 20.0;
        style.IndentSpacing = 30.0;

        let main_viewport = unsafe { &mut *igGetMainViewport() };
        main_viewport.PlatformHandle = null_mut();

        let mut pixels = null_mut();
        let mut width: i32 = 0;
        let mut height: i32 = 0;
        assert_ne!(io.Fonts as usize, 0, "ImGui font is not valid");
        unsafe { ImFontAtlas_GetTexDataAsRGBA32(io.Fonts, &mut pixels, &mut width, &mut height, null_mut()) }
        let data_size = width * height * 4i32;

        let mut font_texture = Image::new(ctx.get().device().clone(), ImageCreateOptions {
            image_type: ImageType::_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED,
            width: width as u32,
            height: height as u32,
            depth: 1,
            mips_levels: 1,
            is_depth: false,
        }).unwrap();

        font_texture.set_data(&BufferMemory::from_raw(pixels as *const u8, 1, data_size as usize))?;

        let mesh = DynamicMesh::new(size_of::<ImDrawVert>(), ctx.get().device().clone())?.index_type(IndexBufferType::Uint16);


        //unsafe { (&mut *io.Fonts).TexID = font_texture.__static_view_handle() as ImTextureID; }

        let mut desc_set = DescriptorSets::new(ctx.get().device().clone(), pipeline.descriptor_set_layout())?;

        let sampler = Sampler::new(ctx.get().device().clone())?;

        desc_set.update(vec![
            (ShaderInstanceBinding::SampledImage(*font_texture.view()?, *font_texture.layout()), 0),
            (ShaderInstanceBinding::Sampler(*sampler.ptr()), 1),
        ])?;
        Ok(Self {
            _compiler: compiler,
            mesh,
            render_pass,
            pipeline,
            descriptor_sets: desc_set,
            font_texture,
            sampler,
            ctx: ctx,
        })
    }

    pub fn render(&mut self, command_buffer: &CommandBuffer) -> Result<(), Error> {
        let io = unsafe { &mut *igGetIO() };

        let window = self.ctx.get().window().get();
        let window_data = window.read();

        io.DisplaySize = ImVec2 { x: window_data.width()? as f32, y: window_data.height()? as f32 };
        io.DisplayFramebufferScale = ImVec2 { x: 1.0, y: 1.0 };
        io.DeltaTime = f32::max(window_data.delta_time as f32, 0.0000000001f32);

        // Update mouse
        io.MouseDown[0] = window_data.input_manager().is_mouse_button_pressed(&MouseButton::Left);
        io.MouseDown[1] = window_data.input_manager().is_mouse_button_pressed(&MouseButton::Right);
        io.MouseDown[2] = window_data.input_manager().is_mouse_button_pressed(&MouseButton::Middle);
        io.MouseDown[3] = window_data.input_manager().is_mouse_button_pressed(&MouseButton::Other(0));
        io.MouseDown[4] = window_data.input_manager().is_mouse_button_pressed(&MouseButton::Other(1));
        let mouse_pos = window_data.input_manager().mouse_position();
        io.MouseHoveredViewport = 0;
        io.MousePos = ImVec2 { x: mouse_pos.x as f32, y: mouse_pos.y as f32 };

        unsafe { igNewFrame(); }

        unsafe { igShowDemoWindow(null_mut()); }


        unsafe { igEndFrame(); }
        unsafe { igRender(); }
        let draw_data = unsafe { &*igGetDrawData() };
        let width = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        let height = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        if width <= 0.0 || height <= 0.0 || draw_data.TotalVtxCount == 0 {
            return Ok(());
        }
        /*
         * BUILD VERTEX BUFFERS
         */
        unsafe {
            let mut vertex_start = 0;
            let mut index_start = 0;

            self.mesh.reserve_vertices(draw_data.TotalVtxCount as usize)?;
            self.mesh.reserve_indices(draw_data.TotalIdxCount as usize)?;

            for n in 0..draw_data.CmdListsCount
            {
                let cmd_list = &**draw_data.CmdLists.offset(n as isize);

                self.mesh.set_indexed_vertices(vertex_start, &BufferMemory::from_raw(cmd_list.VtxBuffer.Data as *const u8, size_of::<ImDrawVert>(), cmd_list.VtxBuffer.Size as usize),
                                               index_start, &BufferMemory::from_raw(cmd_list.IdxBuffer.Data as *const u8, size_of::<ImDrawIdx>(), cmd_list.IdxBuffer.Size as usize))?;

                vertex_start += cmd_list.VtxBuffer.Size as usize;
                index_start += cmd_list.IdxBuffer.Size as usize;
            }
        }

        /*
         * PREPARE MATERIALS
         */
        let scale_x = 2.0 / draw_data.DisplaySize.x;
        let scale_y = -2.0 / draw_data.DisplaySize.y;

        #[repr(C, align(4))]
        pub struct ImGuiPushConstants {
            scale_x: f32,
            scale_y: f32,
            translate_x: f32,
            translate_y: f32,
        }

        command_buffer.push_constant(&self.pipeline, &BufferMemory::from_struct(ImGuiPushConstants {
            scale_x,
            scale_y,
            translate_x: -1.0 - draw_data.DisplayPos.x * scale_x,
            translate_y: 1.0 - draw_data.DisplayPos.y * scale_y,
        }), vk::ShaderStageFlags::VERTEX);

        // Will project scissor/clipping rectangles into framebuffer space
        let clip_off = draw_data.DisplayPos;         // (0,0) unless using multi-viewports
        let clip_scale = draw_data.FramebufferScale; // (1,1) unless using retina display which are often (2,2)

        // Render command lists
        // (Because we merged all buffers into a single one, we maintain our own offset into them)
        let mut global_idx_offset = 0;
        let mut global_vtx_offset = 0;

        for n in 0..draw_data.CmdListsCount
        {
            let cmd = unsafe { &**draw_data.CmdLists.offset(n as isize) };
            for cmd_i in 0..cmd.CmdBuffer.Size
            {
                let pcmd = unsafe { &*cmd.CmdBuffer.Data.offset(cmd_i as isize) };
                match pcmd.UserCallback {
                    Some(callback) => {
                        unsafe { callback(cmd, pcmd); }
                    }
                    None => {
                        // Project scissor/clipping rectangles into framebuffer space
                        let mut clip_rect = ImVec4 {
                            x: (pcmd.ClipRect.x - clip_off.x) * clip_scale.x,
                            y: (pcmd.ClipRect.y - clip_off.y) * clip_scale.y,
                            z: (pcmd.ClipRect.z - clip_off.x) * clip_scale.x,
                            w: (pcmd.ClipRect.w - clip_off.y) * clip_scale.y,
                        };

                        if clip_rect.x < window_data.width()? as f32 && clip_rect.y < window_data.height()? as f32 && clip_rect.z >= 0.0 && clip_rect.w >= 0.0
                        {
                            // Negative offsets are illegal for vkCmdSetScissor
                            if clip_rect.x < 0.0 {
                                clip_rect.x = 0.0;
                            }
                            if clip_rect.y < 0.0 {
                                clip_rect.y = 0.0;
                            }

                            command_buffer.set_scissor(Scissors {
                                min_x: clip_rect.x as i32,
                                min_y: clip_rect.y as i32,
                                width: (clip_rect.z - clip_rect.x) as u32,
                                height: (clip_rect.w - clip_rect.y) as u32,
                            });

                            // Bind descriptor set with font or user texture
                            /* @TODO : handle images
                            if pcmd.TextureId {
                                imgui_material_instance.bind_texture("test", nullptr);
                            }
                             */

                            command_buffer.bind_pipeline(&self.pipeline);
                            command_buffer.bind_descriptors(&self.pipeline, &self.descriptor_sets);

                            command_buffer.draw_mesh_advanced(&self.mesh, pcmd.IdxOffset + global_idx_offset, pcmd.VtxOffset + global_vtx_offset, pcmd.ElemCount, 1, 0);
                        }
                    }
                }
            }
            global_idx_offset += cmd.IdxBuffer.Size as u32;
            global_vtx_offset += cmd.VtxBuffer.Size as u32;
        }
        Ok(())
    }
}