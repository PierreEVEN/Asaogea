use crate::core::gfx::command_buffer::{CommandBuffer, Scissors};
use crate::core::gfx::device::DeviceCtx;
use crate::core::gfx::frame_graph::renderer::RenderPassObject;
use crate::core::gfx::resources::buffer::{BufferMemory, BufferType};
use crate::core::gfx::resources::descriptor_sets::{DescriptorSets, ShaderInstanceBinding};
use crate::core::gfx::resources::image::{Image, ImageCreateOptions};
use crate::core::gfx::resources::mesh::Mesh;
use crate::core::gfx::resources::pipeline::AlphaMode;
use crate::core::gfx::resources::pipeline::{Pipeline, PipelineConfig};
use crate::core::gfx::resources::sampler::Sampler;
use crate::core::gfx::resources::shader_module::{ShaderStage, ShaderStageBindings, ShaderStageInfos, ShaderStageInputs};
use crate::core::gfx::ui::context::{ImGuiContext, SuspendedContext};
use crate::core::gfx::ui::ui::Ui;
use crate::core::window::WindowCtx;
use anyhow::Error;
use imgui::sys::{igGetIO, igGetMainViewport, igGetStyle, igStyleColorsDark, ImDrawIdx, ImDrawVert, ImFontAtlas_GetTexDataAsRGBA32, ImGuiBackendFlags_HasMouseCursors, ImGuiBackendFlags_HasSetMousePos, ImGuiBackendFlags_PlatformHasViewports, ImGuiConfigFlags_DockingEnable, ImGuiConfigFlags_NavEnableGamepad, ImGuiConfigFlags_NavEnableKeyboard, ImGuiConfigFlags_ViewportsEnable, ImVec2, ImVec4};
use shaders::compiler::{HlslCompiler, RawShaderDefinition};
use std::ffi::c_char;
use std::ops::{Deref, DerefMut};
use std::ptr::null_mut;
use std::sync::{Mutex, MutexGuard, RwLock};
use types::resource_handle::{Resource, ResourceHandle};
use vulkanalia::vk;
use vulkanalia::vk::{Extent2D, ImageType};
use winit::event::MouseButton;

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
    mesh: RwLock<Mesh>,
    pipeline: Pipeline,
    descriptor_sets: DescriptorSets,
    _font_texture: Resource<Image>,
    _sampler: Sampler,
    window_ctx: RwLock<WindowCtx>,
    context: RwLock<Option<SuspendedContext>>,
    self_ref: ResourceHandle<ImGui>,
}


pub struct ImGuiPushConstants {
    _scale_x: f32,
    _scale_y: f32,
    _translate_x: f32,
    _translate_y: f32,
}

pub struct ActiveContext {
    imgui: ResourceHandle<ImGui>,
    context: ImGuiContext,
}

static mut IMGUI_ACTIVE_CONTEXT: Option<Mutex<Resource<ActiveContext>>> = None;

pub fn initialize_imgui() {
    unsafe {
        if IMGUI_ACTIVE_CONTEXT.is_none() {
            IMGUI_ACTIVE_CONTEXT = Some(Mutex::new(Resource::default()));
        }
    }
}

pub struct UiPtr<'a> {
    imgui: MutexGuard<'a, Resource<ActiveContext>>,
}

impl<'a> Deref for UiPtr<'a> {
    type Target = Ui;
    fn deref(&self) -> &Self::Target {
        self.imgui.context.ui()
    }
}

impl<'a> DerefMut for UiPtr<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.imgui.context.ui_mut()
    }
}

impl ImGui {
    pub fn new(ctx: DeviceCtx, render_res: Extent2D, render_pass: &ResourceHandle<RenderPassObject>) -> Result<Resource<Self>, Error> {
        let mut compiler = HlslCompiler::new()?;

        let vertex = compiler.compile(&RawShaderDefinition::new("imgui-vertex", "vs_6_0", PIXEL.to_string()))?;
        let fragment = compiler.compile(&RawShaderDefinition::new("imgui-fragment", "ps_6_0", FRAGMENT.to_string()))?;

        let vertex = ShaderStage::new(ctx.clone(), &vertex.raw(), ShaderStageInfos {
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
        let fragment = ShaderStage::new(ctx.clone(), &fragment.raw(),
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


        let pipeline = Pipeline::new(ctx.clone(), &render_pass, vec![vertex, fragment], &PipelineConfig {
            culling: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon_mode: vk::PolygonMode::FILL,
            alpha_mode: AlphaMode::Translucent,
            depth_test: true,
            line_width: 1.0,
        })?;

        let context = ImGuiContext::new(null_mut());

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

        let mut font_texture = Image::new(ctx.clone(), ImageCreateOptions {
            image_type: ImageType::_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED,
            width: width as u32,
            height: height as u32,
            depth: 1,
            mips_levels: 1,
            is_depth: false,
        })?;

        font_texture.set_data(&BufferMemory::from_raw(pixels as *const u8, 1, data_size as usize))?;

        let mesh = Mesh::new(size_of::<ImDrawVert>(), ctx.clone(), BufferType::Immediate)?;

        //unsafe { (&mut *io.Fonts).TexID = font_texture.__static_view_handle() as ImTextureID; }

        let mut desc_set = DescriptorSets::new(ctx.clone(), pipeline.descriptor_set_layout())?;

        let sampler = Sampler::new(ctx.clone())?;

        desc_set.update(vec![
            (ShaderInstanceBinding::SampledImage(*font_texture.view()?, *font_texture.layout()), 0),
            (ShaderInstanceBinding::Sampler(*sampler.ptr()), 1),
        ])?;

        let mut imgui = Resource::new(Self {
            _compiler: compiler,
            mesh: RwLock::new(mesh),
            pipeline,
            descriptor_sets: desc_set,
            _font_texture: font_texture,
            _sampler: sampler,
            window_ctx: Default::default(),
            context: RwLock::new(Some(context.suspend())),
            self_ref: Default::default(),
        });
        imgui.self_ref = imgui.handle();
        imgui.begin(render_res)?;

        Ok(imgui)
    }

    pub fn set_target_window_for_inputs(&self, window: WindowCtx) {
        *self.window_ctx.write().unwrap() = window;
    }

    pub fn ui<'a>(&self) -> UiPtr<'a> {
        let ctx = self.activate();

        unsafe {
            UiPtr {
                imgui: ctx,
            }
        }
    }

    fn begin(&self, render_res: Extent2D) -> Result<(), Error> {
        let mut context = self.activate();


        let io = unsafe { &mut *igGetIO() };

        io.DisplaySize = ImVec2 { x: render_res.width as f32, y: render_res.height as f32 };
        let window_ctx = self.window_ctx.read().unwrap();
        if window_ctx.is_valid() {
            io.DisplayFramebufferScale = ImVec2 { x: 1.0, y: 1.0 };
            io.DeltaTime = f32::max(window_ctx.engine().delta_time().as_secs_f32(), 0.0000000001f32);

            // Update mouse
            io.MouseDown[0] = window_ctx.input_manager().is_mouse_button_pressed(&MouseButton::Left);
            io.MouseDown[1] = window_ctx.input_manager().is_mouse_button_pressed(&MouseButton::Right);
            io.MouseDown[2] = window_ctx.input_manager().is_mouse_button_pressed(&MouseButton::Middle);
            io.MouseDown[3] = window_ctx.input_manager().is_mouse_button_pressed(&MouseButton::Other(0));
            io.MouseDown[4] = window_ctx.input_manager().is_mouse_button_pressed(&MouseButton::Other(1));
            let mouse_pos = window_ctx.input_manager().mouse_position();
            io.MouseHoveredViewport = 0;
            io.MousePos = ImVec2 { x: mouse_pos.x as f32, y: mouse_pos.y as f32 };
        }

        context.context.new_frame();
        self.suspend(context);
        Ok(())
    }

    fn activate<'a>(&self) -> MutexGuard<'a, Resource<ActiveContext>> {
        let mut current_active_context = unsafe { IMGUI_ACTIVE_CONTEXT.as_ref().unwrap().lock().unwrap() };
        // Suspend potential existing context
        if current_active_context.is_valid() && current_active_context.imgui != self.self_ref {
            let old_context = current_active_context.take();
            *old_context.imgui.context.write().unwrap() = Some(old_context.context.suspend());
        }
        // Activate current context if not already activated
        if !current_active_context.is_valid() {
            *current_active_context = Resource::new(ActiveContext {
                imgui: self.self_ref.clone(),
                context: self.context.write().unwrap().take().unwrap().activate(),
            })
        }

        current_active_context
    }

    fn suspend(&self, mut context: MutexGuard<Resource<ActiveContext>>) {
        assert!(context.imgui == self.self_ref);
        if context.is_valid() {
            let imgui = context.imgui.clone();
            *imgui.context.write().unwrap() = Some(context.take().context.suspend());
        }
    }

    pub fn submit_frame(&self, command_buffer: &CommandBuffer, render_res: Extent2D) -> Result<(), Error> {
        let context = self.activate();

        let draw_data = context.context.render();
        let width = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        let height = draw_data.DisplaySize.x * draw_data.FramebufferScale.x;
        if width <= 0.0 || height <= 0.0 || draw_data.TotalVtxCount == 0 {
            self.suspend(context);
            self.begin(render_res)?;
            return Ok(());
        }
        /*
         * BUILD VERTEX BUFFERS
         */
        unsafe {
            let mut vertex_start = 0;
            let mut index_start = 0;

            self.mesh.write().unwrap().reserve_vertices(draw_data.TotalVtxCount as usize)?;
            self.mesh.write().unwrap().reserve_indices(draw_data.TotalIdxCount as usize)?;

            for n in 0..draw_data.CmdListsCount
            {
                let cmd_list = &**draw_data.CmdLists.offset(n as isize);

                self.mesh.write().unwrap().set_indexed_vertices(vertex_start, &BufferMemory::from_raw(cmd_list.VtxBuffer.Data as *const u8, size_of::<ImDrawVert>(), cmd_list.VtxBuffer.Size as usize),
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

                        if clip_rect.x < render_res.width as f32 && clip_rect.y < render_res.height as f32 && clip_rect.z >= 0.0 && clip_rect.w >= 0.0
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

                            command_buffer.draw_mesh_advanced(&self.mesh.write().unwrap(), pcmd.IdxOffset + global_idx_offset, pcmd.VtxOffset + global_vtx_offset, pcmd.ElemCount, 1, 0);
                        }
                    }
                }
            }
            global_idx_offset += cmd.IdxBuffer.Size as u32;
            global_vtx_offset += cmd.VtxBuffer.Size as u32;
        }
        self.suspend(context);

        self.begin(render_res)?;

        Ok(())
    }
}