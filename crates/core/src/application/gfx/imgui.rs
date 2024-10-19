use crate::application::gfx::device::Device;
use crate::application::gfx::instance::Instance;
use crate::application::gfx::resources::pipeline::AlphaMode;
use crate::application::gfx::resources::mesh::DynamicMesh;
use crate::application::gfx::resources::pipeline::{Pipeline, PipelineConfig};
use crate::application::gfx::resources::shader_module::{ShaderStage, ShaderStageInfos};
use anyhow::Error;
use imgui::sys::{igGetIO, ImDrawVert};
use vulkanalia::vk;
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use vulkanalia::vk::CommandBuffer;
use crate::application::gfx::render_pass::{RenderPass, RenderPassCreateInfos};

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

pub struct ImGui {
    compiler: HlslCompiler,
    mesh: DynamicMesh,
    render_pass: RenderPass
}

impl ImGui {
    pub fn new(instance: &Instance) -> Result<Self, Error> {
        let mut compiler = HlslCompiler::new()?;

        let vertex = compiler.compile(&RawShaderDefinition::new("imgui-vertex", "vs_6_0", PIXEL.to_string()))?;
        let fragment = compiler.compile(&RawShaderDefinition::new("imgui-fragment", "ps_6_0", FRAGMENT.to_string()))?;

        let vertex = ShaderStage::new(instance.device().ptr(), &vertex.raw(), ShaderStageInfos {
            descriptor_bindings: vec![],
            push_constant_size: None,
            stage_input: vec![],
            stage: vk::ShaderStageFlags::VERTEX,
            entry_point: "main".to_string(),
        })?;
        let fragment = ShaderStage::new(instance.device().ptr(), &fragment.raw(),
                                        ShaderStageInfos {
                                            descriptor_bindings: vec![],
                                            push_constant_size: None,
                                            stage_input: vec![],
                                            stage: vk::ShaderStageFlags::FRAGMENT,
                                            entry_point: "main".to_string(),
                                        })?;

        let render_pass = RenderPass::new(RenderPassCreateInfos {
            color_attachments: vec![],
            depth_attachment: None,
            is_present_pass: false,
        }, instance.device().ptr())?;
        
        let mut pipeline = Pipeline::new(instance.device().ptr(), &render_pass, vec![vertex, fragment], &PipelineConfig {
            shader_version: "".to_string(),
            culling: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Opaque,
            depth_test: false,
            line_width: 1.0,
        })?;
        pipeline.destroy(instance.device().ptr());

        let mesh = DynamicMesh::new(size_of::<ImDrawVert>(), instance)?;

        Ok(Self {
            compiler,
            mesh,
            render_pass
        })
    }

    pub fn render(&mut self, command_buffer: &CommandBuffer, device: &Device) {
        let io = unsafe { &mut *igGetIO() };
        /*
        io.DisplaySize = ImVec2 { x: command_buffer.get_surface().get_extent().x as f32, y: command_buffer.get_surface().get_extent().y as f32 };
        io.DisplayFramebufferScale = ImVec2 { x: 1.0, y: 1.0 };
        io.DeltaTime = 1.0 / 60.0; //@TODO application::get().delta_time();

        // Update mouse
        let input_manager = engine.platform.input_manager();
        io.MouseDown[0] = input_manager.is_input_pressed(InputMapping::MouseButton(MouseButton::Left));
        io.MouseDown[1] = input_manager.is_input_pressed(InputMapping::MouseButton(MouseButton::Right));
        io.MouseDown[2] = input_manager.is_input_pressed(InputMapping::MouseButton(MouseButton::Middle));
        io.MouseDown[3] = input_manager.is_input_pressed(InputMapping::MouseButton(MouseButton::Button1));
        io.MouseDown[4] = input_manager.is_input_pressed(InputMapping::MouseButton(MouseButton::Button2));
        io.MouseHoveredViewport = 0;
        io.MousePos = ImVec2 { x: input_manager.get_mouse_position().x, y: input_manager.get_mouse_position().y };

        unsafe { igNewFrame(); }
        unsafe { igShowDemoWindow(null_mut()); }
        unsafe { igEndFrame(); }
        unsafe { igRender(); }
        let draw_data = unsafe { &*igGetDrawData() };
        let width = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        let height = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        if width <= 0.0 || height <= 0.0 || draw_data.TotalVtxCount == 0 {
            return;
        }
        /*
         * BUILD VERTEX BUFFERS
         */
        unsafe {
            let mut vertex_start = 0;
            let mut index_start = 0;

            self.mesh.resize(draw_data.TotalVtxCount as u32, draw_data.TotalIdxCount)?;

            for n in 0..draw_data.CmdListsCount
            {
                let cmd_list = &**draw_data.CmdLists.offset(n as isize);

                self.mesh.set_data(
                    vertex_start,
                    slice::from_raw_parts(
                        cmd_list.VtxBuffer.Data as *const u8,
                        cmd_list.VtxBuffer.Size as usize * size_of::<ImDrawVert>() as usize,
                    ),
                    index_start,
                    slice::from_raw_parts(
                        cmd_list.IdxBuffer.Data as *const u8,
                        cmd_list.IdxBuffer.Size as usize * size_of::<ImDrawIdx>() as usize,
                    ),
                )?;

                vertex_start += cmd_list.VtxBuffer.Size as u32;
                index_start += cmd_list.IdxBuffer.Size as u32;
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

        command_buffer.push_constant(
            &shader_program,
            BufferMemory::from_struct(&ImGuiPushConstants {
                scale_x,
                scale_y,
                translate_x: -1.0 - draw_data.DisplayPos.x * scale_x,
                translate_y: 1.0 - draw_data.DisplayPos.y * scale_y,
            }),
            ShaderStage::Vertex,
        );

        shader_instance.bind_texture(&BindPoint::new("sTexture"), &font_texture);

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

                        if clip_rect.x < command_buffer.get_surface().get_extent().x as f32 && clip_rect.y < command_buffer.get_surface().get_extent().y as f32 && clip_rect.z >= 0.0 && clip_rect.w >= 0.0
                        {
                            // Negative offsets are illegal for vkCmdSetScissor
                            if clip_rect.x < 0.0 {
                                clip_rect.x = 0.0;
                            }
                            if clip_rect.y < 0.0 {
                                clip_rect.y = 0.0;
                            }

                            // Apply scissor/clipping rectangle
                            command_buffer.set_scissor(Scissors {
                                min_x: clip_rect.x as i32,
                                min_y: clip_rect.y as i32,
                                width: (clip_rect.z - clip_rect.x) as u32,
                                height: (clip_rect.w - clip_rect.y) as u32,
                            });

                            // Bind descriptor set with font or user texture
                            /*
                            if pcmd.TextureId {
                                imgui_material_instance.bind_texture("test", nullptr); // TODO handle textures
                            }
                            */

                            command_buffer.bind_program(&shader_program);
                            command_buffer.bind_shader_instance(&shader_instance);

                            command_buffer.draw_mesh_advanced(
                                &mesh,
                                pcmd.IdxOffset + global_idx_offset,
                                (pcmd.VtxOffset + global_vtx_offset) as i32,
                                pcmd.ElemCount,
                                1,
                                0,
                            );
                        }
                    }
                }
            }
            global_idx_offset += cmd.IdxBuffer.Size as u32;
            global_vtx_offset += cmd.VtxBuffer.Size as u32;
        }

         */
    }
}