use std::sync::{Arc};
use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, Extent2D, Handle, HasBuilder, Image, ImageView, KhrSwapchainExtension};
use types::resource_handle::{Resource, ResourceHandle};
use types::rwarc::RwArc;
use crate::application::gfx::device::{DeviceCtx, Fence, QueueFlag, SwapchainSupport};
use crate::application::gfx::frame_graph::frame_graph::{FrameGraphInstance, FrameGraphTargetInstance};
use crate::application::gfx::frame_graph::frame_graph_definition::FrameGraph;
use crate::application::window::WindowCtx;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub type SwapchainCtx = ResourceHandle<Swapchain>;

fn get_swapchain_surface_format(swapchain_support: &SwapchainSupport) -> vk::SurfaceFormatKHR {
    swapchain_support.formats
        .iter()
        .cloned()
        .find(|f| {
            f.format == vk::Format::B8G8R8A8_SRGB && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .unwrap_or_else(|| swapchain_support.formats[0])
}

pub struct Swapchain {
    swapchain: Option<vk::SwapchainKHR>,
    swapchain_images: Vec<Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_extent: Extent2D,

    image_available_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<Arc<Fence>>,
    images_in_flight: RwArc<Vec<Arc<Fence>>>,
    frame_graph: Resource<FrameGraphInstance>,
    frame: usize,

    device: DeviceCtx,
    window: WindowCtx,

    surface_format: vk::Format,

    self_ctx: SwapchainCtx,
}

impl Swapchain {
    pub fn new(device: DeviceCtx, window_ctx: WindowCtx) -> Result<Resource<Self>, Error> {
        let swapchain_support = SwapchainSupport::get(
            device.get().instance(),
            window_ctx.surface().ptr(),
            *device.physical_device().ptr())?;
        let surface_format = get_swapchain_surface_format(&swapchain_support);

        let mut swapchain = Resource::new(Self {
            swapchain: None,
            swapchain_images: vec![],
            swapchain_image_views: vec![],
            image_available_semaphores: vec![],
            in_flight_fences: vec![],
            images_in_flight: RwArc::new(vec![]),
            frame_graph: Resource::default(),
            frame: Default::default(),
            device,
            window: window_ctx,
            surface_format: surface_format.format,
            self_ctx: SwapchainCtx::default(),
            swapchain_extent: Default::default(),
        });
        swapchain.self_ctx = swapchain.handle();
        Ok(swapchain)
    }

    pub fn create_renderer(&mut self, frame_graph: FrameGraph) {
        self.create_or_recreate_swapchain().unwrap();
        self.frame_graph = FrameGraphInstance::new(self.device.clone(), frame_graph, FrameGraphTargetInstance::Swapchain(self.self_ctx.clone()));
    }

    pub fn create_or_recreate_swapchain(&mut self) -> Result<(), Error> {
        if self.swapchain.is_some() {
            self.destroy_swapchain()?;
        }

        let swapchain_support = SwapchainSupport::get(
            self.device.get().instance(),
            self.window.surface().ptr(),
            *self.device.physical_device().ptr())?;

        let surface_format = get_swapchain_surface_format(&swapchain_support);
        let present_mode = Self::get_swapchain_present_mode(&swapchain_support);
        let image_count = std::cmp::min(swapchain_support.capabilities.min_image_count + 1,
                                        swapchain_support.capabilities.max_image_count);

        let mut queue_family_indices = vec![];
        let graphic_queue = self.device.queues().find_queue(&QueueFlag::Graphic).expect("Missing required graphic queue");
        let present_queue = self.device.queues().find_queue(&QueueFlag::Present).expect("Missing required present queue");

        let new_size = Extent2D { width: self.window.width()?, height: self.window.height()? };

        let image_sharing_mode = if graphic_queue.index() != present_queue.index() {
            queue_family_indices.push(graphic_queue.index() as u32);
            queue_family_indices.push(present_queue.index() as u32);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };
        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(*self.window.surface().ptr())
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(new_size)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(swapchain_support.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());
        let swapchain = unsafe { self.device.device().create_swapchain_khr(&info, None) }?;
        self.swapchain = Some(swapchain);

        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        unsafe {
            for _ in 0..MAX_FRAMES_IN_FLIGHT {
                let semaphore = self.device.device().create_semaphore(&semaphore_info, None)?;
                self.image_available_semaphores.push(semaphore);
                let device = self.device().clone();
                self.in_flight_fences.push(Arc::new(Fence::new_signaled(device)));
            }
        }

        self.swapchain_images = unsafe { self.device.device().get_swapchain_images_khr(self.swapchain.expect("The swapchain have not been initialized yet"))? };
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
                    .format(get_swapchain_surface_format(&swapchain_support).format)
                    .components(components)
                    .subresource_range(subresource_range);
                unsafe { self.device.device().create_image_view(&info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        *self.images_in_flight.write() = self.swapchain_images
            .iter()
            .map(|_| Arc::new(Fence::default()))
            .collect();

        if self.frame_graph.is_valid() {
            self.frame_graph.resize(self.swapchain_extent.width as usize, self.swapchain_extent.height as usize, &self.swapchain_image_views);
        }
        Ok(())
    }

    pub fn get_swapchain_present_mode(swapchain_support: &SwapchainSupport) -> vk::PresentModeKHR {
        swapchain_support.present_modes
            .iter()
            .cloned()
            .find(|m| *m == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO)
    }

    pub fn get_swapchain_images(&self) -> &Vec<ImageView> {
        &self.swapchain_image_views
    }

    fn destroy_swapchain(&mut self) -> Result<(), Error> {
        self.device.wait_idle();
        unsafe {
            if let Some(swapchain) = self.swapchain.take() {
                self.device.device().destroy_swapchain_khr(swapchain, None);
            }
        }
        Ok(())
    }

    pub fn render(&mut self) -> Result<bool, Error> {
        let frame = self.frame;
        let swapchain = self.swapchain.ok_or(anyhow!("Swapchain is not valid. Maybe you forget to call Swapchain::create_or_recreate()"))?;
        let device = &self.device;
        let device_vulkan = device.device();

        self.in_flight_fences[frame].wait();

        let result = unsafe { device_vulkan.acquire_next_image_khr(swapchain, u64::MAX, self.image_available_semaphores[frame], vk::Fence::null()) };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => { return Ok(true) }
            Err(e) => return Err(anyhow!("Failed to acquire next image : {}", e)),
        };

        if self.images_in_flight.read()[image_index].is_valid() {
            self.images_in_flight.read()[image_index].wait();
        }

        self.images_in_flight.write()[image_index] = self.in_flight_fences[frame].clone();

        self.frame_graph.draw(frame);




        let signal_semaphores = vec![self.frame_graph.present_pass().render_finished_semaphore(image_index)];
        let swapchains = vec![swapchain];
        let image_indices = vec![image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores.as_slice())
            .swapchains(swapchains.as_slice())
            .image_indices(image_indices.as_slice())
            .build();

        let result = self.device.queues().present(&present_info);

        if result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR) || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR) {
            return Ok(true);
        } else if let Err(e) = result {
            return Err(anyhow!("Failed to present image : {}", e));
        }
        self.frame = (frame + 1) % MAX_FRAMES_IN_FLIGHT;
        Ok(false)
    }
    pub fn device(&self) -> &DeviceCtx {
        &self.device
    }
    pub fn window(&self) -> &WindowCtx {
        &self.window
    }
    pub fn format(&self) -> vk::Format { self.surface_format }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        self.device.wait_idle();

        self.destroy_swapchain().unwrap();

        unsafe {
            self.image_available_semaphores
                .iter()
                .for_each(|s| self.device.device().destroy_semaphore(*s, None));
        }
    }
}