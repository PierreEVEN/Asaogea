use std::any::TypeId;
use std::collections::HashSet;
use std::ffi::{c_void};
use anyhow::{Error};
use tracing::{debug, error, trace, warn};
use vulkanalia::{vk, Entry};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::vk::{DebugUtilsMessengerEXT, DeviceV1_0, EntryV1_0, ExtDebugUtilsExtension, Handle, HasBuilder};
use types::resource_handle::{Resource, ResourceHandle};
use crate::core::gfx::device::{Device, DeviceCtx};
use crate::core::gfx::ui::imgui::initialize_imgui;
use crate::core::window::{WindowCtx};
use crate::engine::{EngineCtx};

pub(crate) const VALIDATION_LAYER: vk::ExtensionName = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

pub struct GfxConfig {
    pub validation_layers: bool,
    pub required_extensions: Vec<vk::ExtensionName>,
}

pub struct Instance {
    _entry: Entry,
    _messenger: DebugUtilsMessengerEXT,
    engine: EngineCtx,
    instance: vulkanalia::Instance,
    device: Resource<Device>,
    self_ctx: InstanceCtx,
}

pub type InstanceCtx = ResourceHandle<Instance>;

impl Instance {
    pub fn new(ctx: EngineCtx, config: &mut GfxConfig) -> Result<Resource<Self>, Error> {
        let entry = unsafe {
            let loader = LibloadingLoader::new(LIBRARY)?;
            Entry::new(loader).map_err(|b| anyhow::anyhow!("{}", b))?
        };
        // Required extensions
        let mut extensions = vec![
            vk::KHR_SURFACE_EXTENSION.name.as_ptr(),
            vk::KHR_WIN32_SURFACE_EXTENSION.name.as_ptr()];
        if config.validation_layers {
            extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        }

        let available_layers = unsafe {
            entry.enumerate_instance_layer_properties()?
                .iter()
                .map(|l| l.layer_name)
                .collect::<HashSet<_>>()
        };

        if config.validation_layers && !available_layers.contains(&VALIDATION_LAYER) {
            error!("Validation layer requested but not supported.");
            config.validation_layers = false;
        }

        let layers = if config.validation_layers {
            vec![VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };

        // Additional flags
        let flags = vk::InstanceCreateFlags::empty();

        let application_info = vk::ApplicationInfo::builder()
            .application_name(b"Asaoge\0")
            .application_version(vk::make_version(1, 0, 0))
            .engine_name(b"Asaoge\0")
            .engine_version(vk::make_version(1, 0, 0))
            .api_version(vk::make_version(1, 3, 296));
        let mut info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions)
            .flags(flags);

        // Setup validation layers
        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
            .user_callback(Some(debug_callback));
        if config.validation_layers {
            info = info.push_next(&mut debug_info);
        }


        let instance = unsafe { entry.create_instance(&info, None)? };

        let messenger = if config.validation_layers {
            unsafe { instance.create_debug_utils_messenger_ext(&debug_info, None)? }
        } else {
            DebugUtilsMessengerEXT::null()
        };

        let mut instance = Resource::new(Self {
            engine: ctx,
            instance,
            device: Resource::default(),
            _entry: entry,
            _messenger: messenger,
            self_ctx: Default::default(),
        });
        instance.self_ctx = instance.handle();
        
        initialize_imgui();
        
        Ok(instance)
    }

    pub fn device(&self) -> DeviceCtx {
        self.device.handle()
    }
    pub fn engine(&self) -> &EngineCtx {
        &self.engine
    }
    pub fn ptr(&self) -> &vulkanalia::Instance {
        &self.instance
    }
    pub fn set_vk_object_name<T: Handle + 'static + Copy>(ctx: &DeviceCtx, object: T, handle: u64, name: &str) -> T {
        let object_type =
            if TypeId::of::<vk::Instance>() == TypeId::of::<T>() {
                vk::ObjectType::INSTANCE
            } else if TypeId::of::<vk::PhysicalDevice>() == TypeId::of::<T>() {
                vk::ObjectType::PHYSICAL_DEVICE
            } else if TypeId::of::<vk::Device>() == TypeId::of::<T>() {
                vk::ObjectType::DEVICE
            } else if TypeId::of::<vk::Queue>() == TypeId::of::<T>() {
                vk::ObjectType::QUEUE
            } else if TypeId::of::<vk::Semaphore>() == TypeId::of::<T>() {
                vk::ObjectType::SEMAPHORE
            } else if TypeId::of::<vk::CommandBuffer>() == TypeId::of::<T>() {
                vk::ObjectType::COMMAND_BUFFER
            } else if TypeId::of::<vk::Fence>() == TypeId::of::<T>() {
                vk::ObjectType::FENCE
            } else if TypeId::of::<vk::DeviceMemory>() == TypeId::of::<T>() {
                vk::ObjectType::DEVICE_MEMORY
            } else if TypeId::of::<vk::Buffer>() == TypeId::of::<T>() {
                vk::ObjectType::BUFFER
            } else if TypeId::of::<vk::Image>() == TypeId::of::<T>() {
                vk::ObjectType::IMAGE
            } else if TypeId::of::<vk::Event>() == TypeId::of::<T>() {
                vk::ObjectType::EVENT
            } else if TypeId::of::<vk::QueryPool>() == TypeId::of::<T>() {
                vk::ObjectType::QUERY_POOL
            } else if TypeId::of::<vk::BufferView>() == TypeId::of::<T>() {
                vk::ObjectType::BUFFER_VIEW
            } else if TypeId::of::<vk::ImageView>() == TypeId::of::<T>() {
                vk::ObjectType::IMAGE_VIEW
            } else if TypeId::of::<vk::ShaderModule>() == TypeId::of::<T>() {
                vk::ObjectType::SHADER_MODULE
            } else if TypeId::of::<vk::PipelineCache>() == TypeId::of::<T>() {
                vk::ObjectType::PIPELINE_CACHE
            } else if TypeId::of::<vk::PipelineLayout>() == TypeId::of::<T>() {
                vk::ObjectType::PIPELINE_LAYOUT
            } else if TypeId::of::<vk::RenderPass>() == TypeId::of::<T>() {
                vk::ObjectType::RENDER_PASS
            } else if TypeId::of::<vk::Pipeline>() == TypeId::of::<T>() {
                vk::ObjectType::PIPELINE
            } else if TypeId::of::<vk::DescriptorSetLayout>() == TypeId::of::<T>() {
                vk::ObjectType::DESCRIPTOR_SET_LAYOUT
            } else if TypeId::of::<vk::Sampler>() == TypeId::of::<T>() {
                vk::ObjectType::SAMPLER
            } else if TypeId::of::<vk::DescriptorPool>() == TypeId::of::<T>() {
                vk::ObjectType::DESCRIPTOR_POOL
            } else if TypeId::of::<vk::DescriptorSet>() == TypeId::of::<T>() {
                vk::ObjectType::DESCRIPTOR_SET
            } else if TypeId::of::<vk::Framebuffer>() == TypeId::of::<T>() {
                vk::ObjectType::FRAMEBUFFER
            } else if TypeId::of::<vk::CommandPool>() == TypeId::of::<T>() {
                vk::ObjectType::COMMAND_POOL
            } else if TypeId::of::<vk::SurfaceKHR>() == TypeId::of::<T>() {
                vk::ObjectType::SURFACE_KHR
            } else if TypeId::of::<vk::SwapchainKHR>() == TypeId::of::<T>() {
                vk::ObjectType::SWAPCHAIN_KHR
            } else {
                panic!("unhandled object type id")
            };

        let string_name = name.to_string();

        unsafe {
            ctx.instance().ptr().set_debug_utils_object_name_ext(ctx.device().handle(), &
                vk::DebugUtilsObjectNameInfoEXT::builder()
                    .object_type(object_type)
                    .object_handle(handle)
                    .object_name(string_name.as_bytes())
                    .build()).unwrap();
        }

        object
    }


    pub fn get_device(&self) -> &Resource<Device> {
        &self.device
    }

    pub fn create_device(&mut self, ctx: WindowCtx) -> DeviceCtx {
        if self.device.is_valid() {
            return self.device.handle();
        }
        let device = Device::new(self.self_ctx.clone(),
                                 &ctx.surface(),
                                 &GfxConfig {
                                     validation_layers: true,
                                     required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
                                 }).unwrap();
        let ctx = device.handle();
        self.device = device;
        ctx
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.device = Resource::default();
    }
}

/// Logs debug messages.
extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    type_: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    let data = unsafe { *data };
    let message = unsafe { std::ffi::CStr::from_ptr(data.message) }.to_string_lossy();

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        error!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        debug!("({:?}) {}", type_, message);
    } else {
        trace!("({:?}) {}", type_, message);
    }

    vk::FALSE
}