use anyhow::{anyhow, Error};
use std::ops::Deref;
use vulkanalia::vk::{CommandBuffer, DeviceV1_0, Handle, HasBuilder, Image, KhrSurfaceExtension, KhrSwapchainExtension, KhrWin32SurfaceExtension, SurfaceKHR, HINSTANCE};
use vulkanalia::{vk, Instance};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use crate::device::Device;

pub struct Surface {
    surface: Option<SurfaceKHR>,
    swapchain: Option<vk::SwapchainKHR>,
    swapchain_images: Vec<Image>,
    swapchain_image_views: Vec<vk::ImageView>,

    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,

    temp_command_buffer: Vec<CommandBuffer>
}

impl Surface {
    pub fn new(instance: &Instance, window: &Window) -> Result<Self, Error> {
        let surface = match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => {
                let hinstance = match handle.hinstance {
                    None => { return Err(anyhow!("Invalid hinstance")) }
                    Some(hinstance) => { hinstance }
                };
                let info = vk::Win32SurfaceCreateInfoKHR::builder()
                    .hinstance(hinstance.get() as HINSTANCE)
                    .hwnd(handle.hwnd.get() as HINSTANCE);
                unsafe { instance.create_win32_surface_khr(&info, None) }?
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
            image_available_semaphore: Default::default(),
            render_finished_semaphore: Default::default(),
            temp_command_buffer: vec![],
        })
    }

    pub fn create_swapchain(&mut self, window: &Window, device: &Device) -> Result<(), Error> {
        if self.swapchain.is_some() {
            return Err(anyhow!("Swapchain have already been created"));
        }
        let surface_format = device.get_swapchain_surface_format();
        let present_mode = device.get_swapchain_present_mode();
        let extent = device.get_swapchain_extent(window);
        let image_count = std::cmp::min(device.capabilities().min_image_count + 1,
                                        device.capabilities().max_image_count);

        let mut queue_family_indices = vec![];
        let image_sharing_mode = if device.queue_families_indices().graphics != device.queue_families_indices().present {
            queue_family_indices.push(device.queue_families_indices().graphics);
            queue_family_indices.push(device.queue_families_indices().present);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };
        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(self.surface.expect("Surface have not been created yet !"))
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(device.capabilities().current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());
        let swapchain = unsafe { device.ptr().create_swapchain_khr(&info, None) }?;
        self.swapchain = Some(swapchain);
        self.update_swapchain_images(device)?;


        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        unsafe { self.image_available_semaphore = device.ptr().create_semaphore(&semaphore_info, None)?; }
        unsafe { self.render_finished_semaphore = device.ptr().create_semaphore(&semaphore_info, None)?; }

        self.temp_command_buffer = device.command_pool().allocate(device, self.swapchain_images.len() as u32)?;


        Ok(())
    }

    fn update_swapchain_images(&mut self, device: &Device) -> Result<(), Error> {
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
                    .format(device.get_swapchain_surface_format().format)
                    .components(components)
                    .subresource_range(subresource_range);

                unsafe { device.ptr().create_image_view(&info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    pub fn render(&self, device: &Device) -> Result<(), Error> {
        let swapchain = self.swapchain.ok_or(anyhow!("Swapchain is not valid"))?;
        let image_index = unsafe {
            device
                .ptr()
                .acquire_next_image_khr(swapchain, u64::MAX, self.image_available_semaphore, vk::Fence::null())
        }?
            .0 as usize;

        /*
        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(self.swapchain_extent);
        let color_clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };
        let clear_values = &[color_clear_value];
        let info = vk::RenderPassBeginInfo::builder()
            .render_pass(data.render_pass)
            .framebuffer(data.framebuffers[i])
            .render_area(render_area)
            .clear_values(clear_values);
        device.cmd_begin_render_pass(
            *command_buffer, &info, vk::SubpassContents::INLINE);

        device.cmd_end_render_pass(*command_buffer);
        device.end_command_buffer(*command_buffer)?;




*/







        let wait_semaphores = &[self.image_available_semaphore];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.temp_command_buffer[image_index as usize]];
        let signal_semaphores = &[self.render_finished_semaphore];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);
        unsafe {
            device.ptr().queue_submit(*device.graphic_queue(), &[submit_info], vk::Fence::null())?;
        }

        Ok(())
    }

    pub fn destroy(&mut self, instance: &Instance, device: &vulkanalia::Device) {
        unsafe {
            unsafe { device.device_wait_idle().unwrap(); }
            self.swapchain_image_views
                .iter()
                .for_each(|v| device.destroy_image_view(*v, None));
            self.swapchain_image_views.clear();
            if let Some(swapchain) = self.swapchain.take() {
                device.destroy_semaphore(self.render_finished_semaphore, None);
                device.destroy_semaphore(self.image_available_semaphore, None);
                device.destroy_swapchain_khr(swapchain, None);
            }
            instance.destroy_surface_khr(self.surface.take().expect("This surface is already destroyed"), None);
        }
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
        if self.surface.is_some() {
            panic!("Surface have not been destroyed using Surface::destroy()");
        }
    }
}