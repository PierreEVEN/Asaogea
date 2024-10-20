use anyhow::{anyhow, Error};
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::info;
use vulkanalia::vk::{CommandBuffer, CommandBufferResetFlags, DeviceV1_0, Extent2D, Handle, HasBuilder, Image, KhrSurfaceExtension, KhrSwapchainExtension, KhrWin32SurfaceExtension, SurfaceKHR, HINSTANCE};
use vulkanalia::{vk};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use types::rwarc::RwArc;
use crate::application::gfx::device::{Device, SwapchainSupport};
use crate::application::gfx::imgui::ImGui;
use crate::application::gfx::render_pass::{RenderPass, RenderPassAttachment, RenderPassCreateInfos};
use crate::application::window::CtxAppWindow;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct Surface {
    surface: Option<SurfaceKHR>,
    swapchain: Option<vk::SwapchainKHR>,
    swapchain_images: Vec<Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_extent: Extent2D,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    images_in_flight: RwArc<Vec<vk::Fence>>,

    framebuffers: Vec<vk::Framebuffer>,
    command_buffer: Vec<CommandBuffer>,
    present_pass: Option<RenderPass>,

    frame: AtomicUsize,

    imgui_temp: Option<ImGui>,
}

impl Surface {
    pub fn new(ctx: &CtxAppWindow) -> Result<Self, Error> {
        let surface = match ctx.window.ptr()?.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => {
                let hinstance = match handle.hinstance {
                    None => { return Err(anyhow!("Invalid hinstance")) }
                    Some(hinstance) => { hinstance }
                };
                let info = vk::Win32SurfaceCreateInfoKHR::builder()
                    .hinstance(hinstance.get() as HINSTANCE)
                    .hwnd(handle.hwnd.get() as HINSTANCE);
                unsafe { ctx.engine().instance()?.ptr()?.create_win32_surface_khr(&info, None) }?
            }
            value => {
                return Err(anyhow!("Unsupported window platform : {:?}", value));
            }
        };

        Ok(Self {
            surface: Some(surface),
            swapchain: None,
            swapchain_images: vec![],
            swapchain_image_views: vec![],
            swapchain_extent: Default::default(),
            image_available_semaphores: vec![],
            render_finished_semaphores: vec![],
            in_flight_fences: vec![],
            images_in_flight: RwArc::new(vec![]),
            framebuffers: vec![],
            command_buffer: vec![],
            present_pass: None,
            frame: AtomicUsize::new(0),
            imgui_temp: None,
        })
    }

    pub fn ptr(&self) -> Result<&SurfaceKHR, Error> {
        self.surface.as_ref().ok_or(anyhow!("Surface have already been destroyed"))
    }

    pub fn create_or_recreate_swapchain(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        let device = ctx.engine().device()?;

        if self.swapchain.is_some() {
            self.destroy_swapchain(ctx)?;
        }
        let surface = self.surface.expect("Surface have not been created yet !");

        let swapchain_support = SwapchainSupport::get(ctx, self, *ctx.engine().device()?.physical_device().ptr())?;

        let surface_format = Self::get_swapchain_surface_format(&swapchain_support);
        let present_mode = Self::get_swapchain_present_mode(&swapchain_support);
        let extent = Self::get_swapchain_extent(ctx, &swapchain_support)?;
        let image_count = std::cmp::min(swapchain_support.capabilities.min_image_count + 1,
                                        swapchain_support.capabilities.max_image_count);

        let mut queue_family_indices = vec![];
        let image_sharing_mode = if device.queue_families_indices().graphics != device.queue_families_indices().present {
            queue_family_indices.push(device.queue_families_indices().graphics);
            queue_family_indices.push(device.queue_families_indices().present);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };
        self.swapchain_extent = extent;
        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(self.swapchain_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(swapchain_support.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());
        let swapchain = unsafe { device.ptr().create_swapchain_khr(&info, None) }?;
        self.swapchain = Some(swapchain);

        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder()
            .flags(vk::FenceCreateFlags::SIGNALED);
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            unsafe { self.image_available_semaphores.push(device.ptr().create_semaphore(&semaphore_info, None)?); }
            unsafe { self.render_finished_semaphores.push(device.ptr().create_semaphore(&semaphore_info, None)?); }
            unsafe { self.in_flight_fences.push(device.ptr().create_fence(&fence_info, None)?); }
        }

        self.present_pass = Some(RenderPass::new(RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: None,
                image_format: surface_format.format,
            }],
            depth_attachment: None,
            is_present_pass: true,
        }, device.ptr())?);

        self.update_swapchain_images(&*device, &swapchain_support)?;

        self.command_buffer = device.command_pool().allocate(&*device, self.swapchain_images.len() as u32)?;

        *self.images_in_flight.write() = self.swapchain_images
            .iter()
            .map(|_| vk::Fence::null())
            .collect();

        info!("Created new swapchain : {:?}", extent);

        Ok(())
    }

    pub fn get_swapchain_extent(ctx: &CtxAppWindow, swapchain_support: &SwapchainSupport) -> Result<vk::Extent2D, Error> {
        Ok(if swapchain_support.capabilities.current_extent.width != u32::MAX {
            swapchain_support.capabilities.current_extent
        } else {
            vk::Extent2D::builder()
                .width(ctx.window.ptr()?.inner_size().width.clamp(
                    swapchain_support.capabilities.min_image_extent.width,
                    swapchain_support.capabilities.max_image_extent.width,
                ))
                .height(ctx.window.ptr()?.inner_size().height.clamp(
                    swapchain_support.capabilities.min_image_extent.height,
                    swapchain_support.capabilities.max_image_extent.height,
                ))
                .build()
        })
    }

    pub fn get_swapchain_surface_format(swapchain_support: &SwapchainSupport) -> vk::SurfaceFormatKHR {
        swapchain_support.formats
            .iter()
            .cloned()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or_else(|| swapchain_support.formats[0])
    }

    pub fn get_swapchain_present_mode(swapchain_support: &SwapchainSupport) -> vk::PresentModeKHR {
        swapchain_support.present_modes
            .iter()
            .cloned()
            .find(|m| *m == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO)
    }


    fn destroy_swapchain(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        let device = ctx.engine().device()?;

        unsafe { device.ptr().device_wait_idle()?; }

        unsafe {
            self.framebuffers
                .iter()
                .for_each(|f| device.ptr().destroy_framebuffer(*f, None));

            device.command_pool().free(ctx.ctx_engine(), &self.command_buffer)?;
            self.command_buffer.clear();

            self.swapchain_image_views
                .iter()
                .for_each(|v| device.ptr().destroy_image_view(*v, None));

            self.swapchain_image_views.clear();

            if let Some(swapchain) = self.swapchain.take() {
                device.ptr().destroy_swapchain_khr(swapchain, None);
            }
            if let Some(present_pass) = &mut self.present_pass {
                present_pass.destroy(ctx)?;
            }
            self.present_pass = None;
        }
        Ok(())
    }

    fn update_swapchain_images(&mut self, device: &Device, swapchain_support: &SwapchainSupport) -> Result<(), Error> {
        self.swapchain_images = unsafe { device.ptr().get_swapchain_images_khr(self.swapchain.expect("The swapchain have not been initialized yet"))? };

        self.swapchain_image_views = self
            .swapchain_images
            .iter()
            .map(|i| {
                let components = vk::ComponentMapping::builder()
                    .r(vk::ComponentSwizzle::IDENTITY)
                    .g(vk::ComponentSwizzle::IDENTITY)
                    .b(vk::ComponentSwizzle::IDENTITY)
                    .a(vk::ComponentSwizzle::IDENTITY);

                let subresource_range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);

                let info = vk::ImageViewCreateInfo::builder()
                    .image(*i)
                    .view_type(vk::ImageViewType::_2D)
                    .format(Self::get_swapchain_surface_format(&swapchain_support).format)
                    .components(components)
                    .subresource_range(subresource_range);

                unsafe { device.ptr().create_image_view(&info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let render_pass = self.present_pass.as_ref().expect("Present pass have not been initialized yet").ptr()?;
        self.framebuffers = self.swapchain_image_views
            .iter()
            .map(|i| {
                let attachments = &[*i];
                let create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(*render_pass)
                    .attachments(attachments)
                    .width(self.swapchain_extent.width)
                    .height(self.swapchain_extent.height)
                    .layers(1);

                unsafe { device.ptr().create_framebuffer(&create_info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    pub fn render(&mut self, ctx: &CtxAppWindow) -> Result<bool, Error> {
        let frame = self.frame.load(Ordering::SeqCst);
        let swapchain = self.swapchain.ok_or(anyhow!("Swapchain is not valid"))?;
        let device = ctx.engine().device()?;

        unsafe { device.ptr().wait_for_fences(&[self.in_flight_fences[frame]], true, u64::MAX)?; }
        unsafe { device.ptr().reset_fences(&[self.in_flight_fences[frame]])?; }

        let result = unsafe { device.ptr().acquire_next_image_khr(swapchain, u64::MAX, self.image_available_semaphores[frame], vk::Fence::null()) };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => { return Ok(true) }
            Err(e) => return Err(anyhow!("Failed to acquire next image : {}", e)),
        };

        if !self.images_in_flight.read()[image_index].is_null() {
            unsafe { device.ptr().wait_for_fences(&[self.images_in_flight.read()[image_index]], true, u64::MAX)?; }
        }

        self.images_in_flight.write()[image_index] = self.in_flight_fences[frame];


        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(self.swapchain_extent);
        let color_clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };
        let clear_values = &[color_clear_value];

        unsafe { device.ptr().reset_command_buffer(self.command_buffer[image_index], CommandBufferResetFlags::empty())?; }

        let inheritance = vk::CommandBufferInheritanceInfo::builder();
        let info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::empty()) // Optional.
            .inheritance_info(&inheritance);
        unsafe { device.ptr().begin_command_buffer(self.command_buffer[image_index], &info)?; }

        let info = vk::RenderPassBeginInfo::builder()
            .render_pass(*self.present_pass.as_ref().expect("Present pass have not been initialized yet").ptr()?)
            .framebuffer(self.framebuffers[image_index])
            .render_area(render_area)
            .clear_values(clear_values);

        unsafe { device.ptr().cmd_begin_render_pass(self.command_buffer[image_index], &info, vk::SubpassContents::INLINE); }


        unsafe {
            device.ptr().cmd_set_viewport(self.command_buffer[image_index], 0, &[vk::Viewport::builder()
                .x(0.0)
                .y(self.swapchain_extent.height as _)
                .width(self.swapchain_extent.width as _)
                .height(-(self.swapchain_extent.height as f32))
                .min_depth(0.0)
                .max_depth(1.0)
                .build()
            ])
        };
        

        if self.imgui_temp.is_none() {
            self.imgui_temp = Some(ImGui::new(ctx)?);
        }

        self.imgui_temp.as_mut().unwrap().render(ctx, &self.command_buffer[image_index])?;


        unsafe { device.ptr().cmd_end_render_pass(self.command_buffer[image_index]); }
        unsafe { device.ptr().end_command_buffer(self.command_buffer[image_index])?; }

        let wait_semaphores = &[self.image_available_semaphores[frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.command_buffer[image_index]];
        let signal_semaphores = &[self.render_finished_semaphores[frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        unsafe { device.ptr().queue_submit(*device.graphic_queue(), &[submit_info], self.in_flight_fences[frame])?; }


        let swapchains = &[swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = unsafe { device.ptr().queue_present_khr(*device.present_queue(), &present_info) };

        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR) || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);

        if changed {
            return Ok(true);
        } else if let Err(e) = result {
            return Err(anyhow!("Failed to present image : {}", e));
        }

        self.frame.store((frame + 1) % MAX_FRAMES_IN_FLIGHT, Ordering::SeqCst);
        Ok(false)
    }

    pub fn destroy(&mut self, ctx: &CtxAppWindow) -> Result<(), Error> {
        if let Some(imgui) = &mut self.imgui_temp {
            imgui.destroy(ctx)?;
        }
        self.imgui_temp = None;

        let device = ctx.engine().device()?;

        unsafe { device.ptr().device_wait_idle()?; }

        if let Some(present_pass) = &mut self.present_pass {
            present_pass.destroy(ctx)?;
        }
        self.destroy_swapchain(ctx)?;

        unsafe {
            let device = ctx.engine().device()?;

            self.render_finished_semaphores
                .iter()
                .for_each(|s| device.ptr().destroy_semaphore(*s, None));
            self.image_available_semaphores
                .iter()
                .for_each(|s| device.ptr().destroy_semaphore(*s, None));
            self.in_flight_fences
                .iter()
                .for_each(|f| device.ptr().destroy_fence(*f, None));

            ctx.engine().instance()?.ptr()?.destroy_surface_khr(self.surface.take().ok_or(anyhow!("This surface is already destroyed"))?, None);
        }

        self.present_pass = None;

        Ok(())
    }
}

impl Deref for Surface {
    type Target = SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        match &self.surface {
            None => { panic!("This surface have already been destroyed") }
            Some(surface) => { surface }
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.surface.is_some() || self.present_pass.is_some() {
            panic!("Surface have not been destroyed using Surface::destroy()");
        }
    }
}