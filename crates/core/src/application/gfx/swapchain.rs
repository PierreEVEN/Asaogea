use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use anyhow::{anyhow, Error};
use glam::Vec4;
use tracing::info;
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Extent2D, Handle, HasBuilder, Image, KhrSwapchainExtension};
use types::resource_handle::{Resource, ResourceHandle};
use types::rwarc::RwArc;
use crate::application::gfx::command_buffer::{CommandBuffer, Viewport};
use crate::application::gfx::device::{DeviceCtx, Fence, QueueFlag, SwapchainSupport};
use crate::application::gfx::frame_graph::frame_graph::{AttachmentSource, ClearValues, FrameGraph, RenderPass, RenderPassAttachment, RenderPassCreateInfos, SwapchainImage};
use crate::application::gfx::imgui::ImGui;
use crate::application::window::WindowCtx;
use crate::test_app::test_app::TestApp;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct Swapchain {
    data: Resource<SwapchainData>,
}

pub type SwapchainCtx = ResourceHandle<SwapchainData>;

pub struct SwapchainData {
    swapchain: Option<vk::SwapchainKHR>,
    swapchain_images: Vec<Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_extent: Extent2D,

    image_available_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<Arc<Fence>>,
    images_in_flight: RwArc<Vec<Arc<Fence>>>,
    frame_graph: FrameGraph,
    frame: usize,

    device: DeviceCtx,
    window: WindowCtx,

    surface_format: vk::Format
}


impl SwapchainData {
    pub fn device(&self) -> &DeviceCtx {
        &self.device
    }
    pub fn window(&self) -> &WindowCtx {
        &self.window
    }

    pub fn format(&self) -> vk::Format {self.surface_format}
}


impl Swapchain {

    pub fn new(device: DeviceCtx, window_ctx: WindowCtx) -> Result<Self, Error> {

        let swapchain_support = SwapchainSupport::get(
            device.get().instance(),
            window_ctx.surface().ptr(),
            *device.physical_device().ptr())?;
        let surface_format = Self::get_swapchain_surface_format(&swapchain_support);

        let mut present_pass = device.find_or_create_render_pass(RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: ClearValues::Color(Vec4::new(1f32, 0f32, 0f32, 1f32)),
                source: AttachmentSource::SwapchainImage(surface_format.format),
            }],
            depth_attachment: None,
        });

        present_pass.attach(Arc::new(RenderPass::new(device.clone(), RenderPassCreateInfos {
            color_attachments: vec![RenderPassAttachment {
                clear_value: ClearValues::Color(Vec4::new(1f32, 1f32, 0f32, 1f32)),
                source: AttachmentSource::DynamicImage(vk::Format::R16G16B16_UNORM),
            }],
            depth_attachment: Some(RenderPassAttachment {
                clear_value: ClearValues::DontClear,
                source: AttachmentSource::DynamicImage(vk::Format::D32_SFLOAT),
            }),
        })));

        let framegraph = FrameGraph::new(device.clone(), present_pass, MAX_FRAMES_IN_FLIGHT + 1);

        let mut swapchain = Self {
            data: Resource::new(SwapchainData {
                swapchain: None,
                swapchain_images: vec![],
                swapchain_image_views: vec![],
                swapchain_extent: Default::default(),
                image_available_semaphores: vec![],
                in_flight_fences: vec![],
                images_in_flight: RwArc::new(vec![]),
                frame_graph: framegraph,
                frame: Default::default(),
                device,
                window: window_ctx,
                surface_format: surface_format.format,
            }),
        };

        swapchain.create_or_recreate_swapchain()?;

        Ok(swapchain)
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

    pub fn create_or_recreate_swapchain(&mut self) -> Result<(), Error> {
        let device = self.data.device();
        let device_vulkan = device.device();

        if self.data.swapchain.is_some() {
            self.destroy_swapchain()?;
        }

        let swapchain_support = SwapchainSupport::get(
            device.instance(),
            self.data.window.surface().ptr(),
            *device.physical_device().ptr())?;

        let surface_format = Self::get_swapchain_surface_format(&swapchain_support);
        let present_mode = Self::get_swapchain_present_mode(&swapchain_support);
        let extent = Self::get_swapchain_extent(&self.data.window, &swapchain_support)?;
        let image_count = std::cmp::min(swapchain_support.capabilities.min_image_count + 1,
                                        swapchain_support.capabilities.max_image_count);

        let mut queue_family_indices = vec![];
        let graphic_queue = device.queues().find_queue(&QueueFlag::Graphic).expect("Missing required graphic queue");
        let present_queue = device.queues().find_queue(&QueueFlag::Present).expect("Missing required present queue");

        let image_sharing_mode = if graphic_queue.index() != present_queue.index() {
            queue_family_indices.push(graphic_queue.index() as u32);
            queue_family_indices.push(present_queue.index() as u32);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };
        self.data.swapchain_extent = extent;
        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(*self.data.window.surface().ptr())
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(self.data.swapchain_extent)
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
        self.data.swapchain = Some(swapchain);

        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            unsafe { self.data.image_available_semaphores.push(device_vulkan.create_semaphore(&semaphore_info, None)?); }
            self.data.in_flight_fences.push(Arc::new(Fence::new_signaled(self.ctx().device.clone())));
        }

        let device = &self.data.device;
        let device_vulkan = device.device();
        self.data.swapchain_images = unsafe { device_vulkan.get_swapchain_images_khr(self.data.swapchain.expect("The swapchain have not been initialized yet"))? };
        self.data.swapchain_image_views = self
            .data.swapchain_images
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

        *self.data.images_in_flight.write() = self.data.swapchain_images
            .iter()
            .map(|_| Arc::new(Fence::default()))
            .collect();

        self.data.frame_graph.resize(self.data.swapchain_extent.width as usize, self.data.swapchain_extent.height as usize, &self.data.swapchain_image_views);

        info!("Created new swapchain : {:?}", extent);

        Ok(())
    }

    pub fn get_swapchain_extent(window_ctx: &WindowCtx, swapchain_support: &SwapchainSupport) -> Result<vk::Extent2D, Error> {
        Ok(if swapchain_support.capabilities.current_extent.width != u32::MAX {
            swapchain_support.capabilities.current_extent
        } else {
            vk::Extent2D::builder()
                .width(window_ctx.width()?.clamp(
                    swapchain_support.capabilities.min_image_extent.width,
                    swapchain_support.capabilities.max_image_extent.width,
                ))
                .height(window_ctx.width()?.clamp(
                    swapchain_support.capabilities.min_image_extent.height,
                    swapchain_support.capabilities.max_image_extent.height,
                ))
                .build()
        })
    }

    pub fn get_swapchain_present_mode(swapchain_support: &SwapchainSupport) -> vk::PresentModeKHR {
        swapchain_support.present_modes
            .iter()
            .cloned()
            .find(|m| *m == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO)
    }


    fn destroy_swapchain(&mut self) -> Result<(), Error> {
        let device = &self.data.device;
        device.wait_idle();
        unsafe {
            self.data.swapchain_image_views
                .iter()
                .for_each(|v| device.device().destroy_image_view(*v, None));

            self.data.swapchain_image_views.clear();

            if let Some(swapchain) = self.data.swapchain.take() {
                device.device().destroy_swapchain_khr(swapchain, None);
            }
        }
        Ok(())
    }

    pub fn render(&mut self) -> Result<bool, Error> {
        let frame = self.data.frame;
        let swapchain = self.data.swapchain.ok_or(anyhow!("Swapchain is not valid. Maybe you forget to call Swapchain::create_or_recreate()"))?;
        let device = &self.data.device;
        let device_vulkan = device.device();

        self.data.in_flight_fences[frame].wait();

        let result = unsafe { device_vulkan.acquire_next_image_khr(swapchain, u64::MAX, self.data.image_available_semaphores[frame], vk::Fence::null()) };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => { return Ok(true) }
            Err(e) => return Err(anyhow!("Failed to acquire next image : {}", e)),
        };

        if self.data.images_in_flight.read()[image_index].is_valid() {
            self.data.images_in_flight.read()[image_index].wait();
        }

        self.data.images_in_flight.write()[image_index] = self.data.in_flight_fences[frame].clone();

        self.data.frame_graph.draw(frame, SwapchainImage {
            image_view: self.data.swapchain_image_views[frame],
            wait_semaphore: self.data.image_available_semaphores[frame],
            work_finished_fence: *self.data.in_flight_fences[frame].ptr(),
        });

        let changed = self.unwrap().present_to_swapchain(frame, &swapchain);

        if changed {
            return Ok(true);
        } else if let Err(e) = result {
            return Err(anyhow!("Failed to present image : {}", e));
        }
        self.data.frame = (frame + 1) % MAX_FRAMES_IN_FLIGHT;
        Ok(false)
    }

    pub fn ctx(&self) -> SwapchainCtx {
        self.data.handle()
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        let device = &self.data.device;
        device.wait_idle();

        self.destroy_swapchain().unwrap();

        unsafe {
            self.data.image_available_semaphores
                .iter()
                .for_each(|s| device.device().destroy_semaphore(*s, None));
        }
    }
}