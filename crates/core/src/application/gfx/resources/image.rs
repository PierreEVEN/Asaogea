use crate::application::gfx::command_buffer::CommandBuffer;
use crate::application::gfx::device::DeviceSharedData;
use crate::application::gfx::resources::buffer::{Buffer, BufferAccess};
use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{DeviceV1_0, FenceCreateInfo, HasBuilder};
use vulkanalia_vma::Alloc;

pub struct Image {
    image: Option<vk::Image>,
    allocation: Option<vulkanalia_vma::Allocation>,
    view: Option<vk::ImageView>,
    create_infos: ImageCreateOptions,
    current_layout: vk::ImageLayout,
    ctx: DeviceSharedData
}

pub struct ImageCreateOptions {
    pub image_type: vk::ImageType,
    pub format: vk::Format,
    pub usage: vk::ImageUsageFlags,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mips_levels: u32,
    pub is_depth: bool,
}


impl Image {
    pub fn new(ctx: DeviceSharedData, create_infos: ImageCreateOptions) -> Result<Self, Error> {
        let infos = vk::ImageCreateInfo::builder()
            .image_type(create_infos.image_type)
            .format(create_infos.format)
            .extent(vk::Extent3D { width: create_infos.width, height: create_infos.height, depth: create_infos.depth })
            .mip_levels(create_infos.mips_levels)
            .array_layers(1)
            .samples(vk::SampleCountFlags::_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(create_infos.usage | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build();

        let allocation_options = vulkanalia_vma::AllocationOptions::default();
        println!("Create image");
        let (image, allocation) = unsafe { ctx.allocator().create_image(infos, &allocation_options) }?;

        let image_view_ci = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::_2D)
            .format(create_infos.format)
            .components(vk::ComponentMapping { r: vk::ComponentSwizzle::R, g: vk::ComponentSwizzle::G, b: vk::ComponentSwizzle::B, a: vk::ComponentSwizzle::A })
            .subresource_range(vk::ImageSubresourceRange::builder()
                .aspect_mask(if create_infos.is_depth { vk::ImageAspectFlags::DEPTH } else { vk::ImageAspectFlags::COLOR })
                .base_mip_level(0)
                .level_count(create_infos.mips_levels)
                .base_array_layer(0)
                .layer_count(1)
                .build())
            .build();

        let image_view = unsafe { ctx.device().create_image_view(&image_view_ci, None)? };

        Ok(Self {
            image: Some(image),
            allocation: Some(allocation),
            view: Some(image_view),
            create_infos,
            current_layout: vk::ImageLayout::UNDEFINED,
            ctx,
        })
    }

    pub fn set_data(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut transfer_buffer = Buffer::new(self.ctx.clone(), data.len(), crate::application::gfx::resources::buffer::BufferCreateInfo { usage: vk::BufferUsageFlags::TRANSFER_SRC, access: BufferAccess::CpuToGpu })?;

        transfer_buffer.set_data( 0, data)?;

        let mut command_buffer = CommandBuffer::new(self.ctx.clone())?;
        command_buffer.begin_one_time()?;

        self.set_image_layout(command_buffer.ptr()?, vk::ImageLayout::TRANSFER_DST_OPTIMAL)?;
        // GPU copy command
        unsafe {
            self.ctx.device().cmd_copy_buffer_to_image(
                *command_buffer.ptr()?,
                *transfer_buffer.ptr()?,
                self.image.ok_or(anyhow!("invalid image"))?,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::builder()
                    .buffer_offset(0)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(vk::ImageSubresourceLayers::builder()
                        .aspect_mask(if self.create_infos.is_depth { vk::ImageAspectFlags::DEPTH } else { vk::ImageAspectFlags::COLOR })
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build())
                    .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                    .image_extent(vk::Extent3D { width: self.create_infos.width, height: self.create_infos.height, depth: self.create_infos.depth })
                    .build()]);
        }

        self.set_image_layout(command_buffer.ptr()?, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)?;
        command_buffer.end()?;

        let submit_info = vk::SubmitInfo::builder()
            .command_buffers(&[*command_buffer.ptr()?])
            .build();

        let infos = FenceCreateInfo::builder();
        let fence = unsafe { self.ctx.device().create_fence(&infos, None) }?;

        unsafe { self.ctx.device().queue_submit(self.ctx.queues().transfer, &[submit_info], fence) }?;

        unsafe { self.ctx.device().wait_for_fences(&[fence], true, u64::MAX)?; }

        unsafe { self.ctx.device().destroy_fence(fence, None) };

        Ok(())
    }


    fn set_image_layout(&mut self, command_buffer: &vk::CommandBuffer, new_layout: vk::ImageLayout) -> Result<(), Error> {
        let mut barrier = vk::ImageMemoryBarrier::builder()
            .old_layout(self.current_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(*self.image.as_ref().ok_or(anyhow!("Invalid image during layout update"))?)
            .subresource_range(vk::ImageSubresourceRange::builder()
                .aspect_mask(if self.create_infos.is_depth { vk::ImageAspectFlags::DEPTH } else { vk::ImageAspectFlags::COLOR })
                .base_mip_level(0)
                .level_count(self.create_infos.mips_levels)
                .base_array_layer(0)
                .layer_count(1)
                .build())
            .build();

        let source_destination_stages = if self.current_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::empty();
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            (vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER)
        } else if self.current_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            (vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER)
        } else {
            panic!("Unsupported layout transition");
        };

        self.current_layout = new_layout;

        unsafe {
            let memory_barriers: [vk::MemoryBarrier; 0] = [];
            let buffer_memory_barriers: [vk::BufferMemoryBarrier; 0] = [];
            self.ctx.device().cmd_pipeline_barrier(
                *command_buffer,
                source_destination_stages.0,
                source_destination_stages.1,
                vk::DependencyFlags::empty(),
                &memory_barriers,
                &buffer_memory_barriers,
                &[barrier]);
        }
        Ok(())
    }

    pub fn view(&self) -> Result<&vk::ImageView, Error> {
        self.view.as_ref().ok_or(anyhow!("Invalid image view"))
    }

    pub fn layout(&self) -> &vk::ImageLayout {
        &self.current_layout
    }

    pub fn image(&self) -> Result<&vk::Image, Error> {
        self.image.as_ref().ok_or(anyhow!("Invalid image"))
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        println!("Destroy image");
        unsafe { self.ctx.allocator().destroy_image(self.image.take().unwrap(), self.allocation.unwrap()) };

        unsafe { self.ctx.device().destroy_image_view(self.view.take().unwrap(), None) };

    }
}