use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use anyhow::{anyhow, Error};
use tracing::info;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Extent2D, Handle, HasBuilder, Image, KhrSwapchainExtension};
use types::rwarc::RwArc;
use crate::application::gfx::command_buffer::{CommandBuffer, Viewport};
use crate::application::gfx::device::{DeviceCtx, QueueFlag, SwapchainSupport};
use crate::application::gfx::imgui::ImGui;
use crate::application::gfx::render_pass::{RenderPass, RenderPassAttachment, RenderPassCreateInfos};
use crate::application::window::WindowCtx;
use crate::test_app::test_app::TestApp;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct Swapchain {
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

    data: Arc<SwapchainData>,
    test_app: Option<TestApp>,
}

#[derive(Clone)]
pub struct SwapchainCtx(Weak<SwapchainData>);
impl SwapchainCtx {
    pub fn get(&self) -> Arc<SwapchainData> {
        self.0.upgrade().unwrap()
    }
}
pub struct SwapchainData {
    device: DeviceCtx,
    window: WindowCtx,
}
impl SwapchainData {
    pub fn device(&self) -> &DeviceCtx {
        &self.device
    }
    pub fn window(&self) -> &WindowCtx {
        &self.window
    }
}


impl Swapchain {
    pub fn new(device_ctx: DeviceCtx, window_ctx: WindowCtx) -> Result<Self, Error> {
        let mut swapchain = Self {
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
            frame: Default::default(),
            imgui_temp: None,
            data: Arc::new(SwapchainData {
                device: device_ctx,
                window: window_ctx,
            }),
            test_app: None,
        };

        swapchain.create_or_recreate_swapchain()?;

        Ok(swapchain)
    }

    pub fn create_or_recreate_swapchain(&mut self) -> Result<(), Error> {
        let device = self.data.device().get();
        let device_vulkan = device.device();

        if self.swapchain.is_some() {
            self.destroy_swapchain()?;
        }

        let swapchain_support = SwapchainSupport::get(
            device.get().instance(),
            self.data.window.get().read().surface().ptr(),
            *device.physical_device().ptr())?;

        let surface_format = Self::get_swapchain_surface_format(&swapchain_support);
        let present_mode = Self::get_swapchain_present_mode(&swapchain_support);
        let extent = Self::get_swapchain_extent(&self.data.window, &swapchain_support)?;
        let image_count = std::cmp::min(swapchain_support.capabilities.min_image_count + 1,
                                        swapchain_support.capabilities.max_image_count);

        let mut queue_family_indices = vec![];
        let image_sharing_mode = if device.physical_device().queue_families_indices().graphics != device.physical_device().queue_families_indices().present {
            queue_family_indices.push(device.physical_device().queue_families_indices().graphics);
            queue_family_indices.push(device.physical_device().queue_families_indices().present);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };
        self.swapchain_extent = extent;
        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(*self.data.window.get().read().surface().ptr())
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
        let swapchain = unsafe { device_vulkan.create_swapchain_khr(&info, None) }?;
        self.swapchain = Some(swapchain);

        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder()
            .flags(vk::FenceCreateFlags::SIGNALED);
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            unsafe { self.image_available_semaphores.push(device_vulkan.create_semaphore(&semaphore_info, None)?); }
            unsafe { self.render_finished_semaphores.push(device_vulkan.create_semaphore(&semaphore_info, None)?); }
            unsafe { self.in_flight_fences.push(device_vulkan.create_fence(&fence_info, None)?); }
        }

        self.present_pass = Some(RenderPass::new(self.data.device.clone(), RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: None,
                image_format: surface_format.format,
            }],
            depth_attachment: None,
            is_present_pass: true,
        })?);

        self.update_swapchain_images(&swapchain_support)?;

        self.command_buffer = vec![];

        for _ in 0..self.swapchain_images.len() {
            self.command_buffer.push(CommandBuffer::new(self.data.device.clone())?)
        }
        *self.images_in_flight.write() = self.swapchain_images
            .iter()
            .map(|_| vk::Fence::null())
            .collect();

        info!("Created new swapchain : {:?}", extent);

        Ok(())
    }

    pub fn get_swapchain_extent(window_ctx: &WindowCtx, swapchain_support: &SwapchainSupport) -> Result<vk::Extent2D, Error> {
        Ok(if swapchain_support.capabilities.current_extent.width != u32::MAX {
            swapchain_support.capabilities.current_extent
        } else {
            vk::Extent2D::builder()
                .width(window_ctx.get().read().width()?.clamp(
                    swapchain_support.capabilities.min_image_extent.width,
                    swapchain_support.capabilities.max_image_extent.width,
                ))
                .height(window_ctx.get().read().width()?.clamp(
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


    fn destroy_swapchain(&mut self) -> Result<(), Error> {
        let device = self.data.device.get();

        unsafe { device.device().device_wait_idle()?; }

        unsafe {
            self.framebuffers
                .iter()
                .for_each(|f| device.device().destroy_framebuffer(*f, None));

            self.command_buffer.clear();

            self.swapchain_image_views
                .iter()
                .for_each(|v| device.device().destroy_image_view(*v, None));

            self.swapchain_image_views.clear();

            if let Some(swapchain) = self.swapchain.take() {
                device.device().destroy_swapchain_khr(swapchain, None);
            }
            self.present_pass = None;
        }
        Ok(())
    }

    fn update_swapchain_images(&mut self, swapchain_support: &SwapchainSupport) -> Result<(), Error> {
        let device = self.data.device.get();
        let device_vulkan = device.device();
        self.swapchain_images = unsafe { device_vulkan.get_swapchain_images_khr(self.swapchain.expect("The swapchain have not been initialized yet"))? };

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

                unsafe { device_vulkan.create_image_view(&info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let render_pass = self.present_pass.as_ref().expect("Present pass have not been initialized yet").ptr();
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

                unsafe { device_vulkan.create_framebuffer(&create_info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    pub fn render(&mut self) -> Result<bool, Error> {
        let frame = self.frame.load(Ordering::SeqCst);
        let swapchain = self.swapchain.ok_or(anyhow!("Swapchain is not valid. Maybe you forget to call Swapchain::create_or_recreate()"))?;
        let device = self.data.device.get();
        let device_vulkan = device.device();

        unsafe { device_vulkan.wait_for_fences(&[self.in_flight_fences[frame]], true, u64::MAX)?; }

        let result = unsafe { device_vulkan.acquire_next_image_khr(swapchain, u64::MAX, self.image_available_semaphores[frame], vk::Fence::null()) };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => { return Ok(true) }
            Err(e) => return Err(anyhow!("Failed to acquire next image : {}", e)),
        };

        if !self.images_in_flight.read()[image_index].is_null() {
            unsafe { device_vulkan.wait_for_fences(&[self.images_in_flight.read()[image_index]], true, u64::MAX)?; }
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

        self.command_buffer[image_index].reset()?;
        self.command_buffer[image_index].begin()?;

        let info = vk::RenderPassBeginInfo::builder()
            .render_pass(*self.present_pass.as_ref().unwrap().ptr())
            .framebuffer(self.framebuffers[image_index])
            .render_area(render_area)
            .clear_values(clear_values);

        unsafe { device_vulkan.cmd_begin_render_pass(*self.command_buffer[image_index].ptr()?, &info, vk::SubpassContents::INLINE); }


        self.command_buffer[image_index].set_viewport(&Viewport {
            min_x: 0.0,
            min_y: self.swapchain_extent.height as _,
            width: self.swapchain_extent.width as _,
            height: -(self.swapchain_extent.height as f32),
            min_depth: 0.0,
            max_depth: 0.0,
        });


        if self.test_app.is_none() {
           self.test_app = Some(TestApp::new(self.ctx(), self.present_pass.as_ref().unwrap())?);
        }
        self.test_app.as_mut().unwrap().render(&self.command_buffer[image_index])?;

        if self.imgui_temp.is_none() {
            self.imgui_temp = Some(ImGui::new(self.ctx())?);
        }
        self.imgui_temp.as_mut().unwrap().render(&self.command_buffer[image_index])?;

        unsafe { device_vulkan.cmd_end_render_pass(*self.command_buffer[image_index].ptr()?); }
        self.command_buffer[image_index].end()?;

        let wait_semaphores = &[self.image_available_semaphores[frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[*self.command_buffer[image_index].ptr()?];
        let signal_semaphores = &[self.render_finished_semaphores[frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores)
            .build();

        device.queues().submit(&QueueFlag::Graphic, &[submit_info], Some(&self.in_flight_fences[frame]));
        
        let swapchains = &[swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices)
            .build();

        let result = device.queues().present(&present_info);

        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR) || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);

        if changed {
            return Ok(true);
        } else if let Err(e) = result {
            return Err(anyhow!("Failed to present image : {}", e));
        }

        self.frame.store((frame + 1) % MAX_FRAMES_IN_FLIGHT, Ordering::SeqCst);
        Ok(false)
    }

    pub fn ctx(&self) -> SwapchainCtx {
        SwapchainCtx(Arc::downgrade(&self.data))
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        let device = self.data.device.get();
        unsafe { device.device().device_wait_idle().unwrap(); }

        self.imgui_temp = None;

        self.present_pass = None;
        self.destroy_swapchain().unwrap();

        unsafe {
            self.render_finished_semaphores
                .iter()
                .for_each(|s| device.device().destroy_semaphore(*s, None));
            self.image_available_semaphores
                .iter()
                .for_each(|s| device.device().destroy_semaphore(*s, None));
            self.in_flight_fences
                .iter()
                .for_each(|f| device.device().destroy_fence(*f, None));
        }

        self.present_pass = None;
    }
}