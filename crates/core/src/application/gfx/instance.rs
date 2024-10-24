use std::any::TypeId;
use std::collections::HashSet;
use std::ffi::{c_void};
use std::sync::{Arc, Weak};
use anyhow::{Error};
use tracing::{debug, error, trace, warn};
use vulkanalia::{vk, Entry};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::vk::{DebugUtilsMessengerEXT, DeviceV1_0, EntryV1_0, ExtDebugUtilsExtension, Handle, HasBuilder};
use types::rwslock::RwSLock;
use crate::application::gfx::device::{Device, DeviceCtx};
use crate::application::window::{WindowCtx};
use crate::engine::{EngineCtx};

pub(crate) const VALIDATION_LAYER: vk::ExtensionName = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

pub struct GfxConfig {
    pub validation_layers: bool,
    pub required_extensions: Vec<vk::ExtensionName>,
}

pub struct Instance {
    data: Arc<InstanceData>,
    _entry: Entry,
    _messenger: DebugUtilsMessengerEXT,
}

#[derive(Clone)]
pub struct InstanceCtx(Weak<InstanceData>);
impl InstanceCtx {
    pub fn get(&self) -> Arc<InstanceData> {
        self.0.upgrade().unwrap()
    }
}
pub struct InstanceData {
    engine: EngineCtx,
    instance: Option<vulkanalia::Instance>,
    device: RwSLock<Option<Device>>,
}
impl InstanceData {
    pub fn device(&self) -> DeviceCtx {
        self.device.read().unwrap().as_ref().unwrap().ctx()
    }
}

impl InstanceData {
    pub fn engine(&self) -> &EngineCtx {
        &self.engine
    }
    pub fn instance(&self) -> &vulkanalia::Instance {
        self.instance.as_ref().unwrap()
    }
}


impl Instance {
    pub fn new(ctx: EngineCtx, config: &mut GfxConfig) -> Result<Self, Error> {
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

        let instance = Self {
            data: Arc::new(InstanceData {
                engine: ctx,
                instance: Some(instance),
                device: RwSLock::new(None),
            }),
            _entry: entry,
            _messenger: messenger,
        };
        Ok(instance)
    }

    pub fn ctx(&self) -> InstanceCtx {
        InstanceCtx(Arc::downgrade(&self.data))
    }

    pub fn set_vk_object_name<T: vk::Handle + 'static + Copy>(ctx: &DeviceCtx, object: T, handle: u64, name: &str) -> T {
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

        let string_name = format!("{}", name);

        unsafe {
            let instance = ctx.get().get();
            instance.instance().set_debug_utils_object_name_ext(ctx.get().device().handle(), &
                vk::DebugUtilsObjectNameInfoEXT::builder()
                    .object_type(object_type)
                    .object_handle(handle)
                    .object_name(string_name.as_bytes())
                    .build()).unwrap();
        }

        object
    }


    pub fn get_or_create_device(&mut self, ctx: WindowCtx) -> DeviceCtx {
        if let Some(device) = self.data.device.read().unwrap().as_ref() {
            return device.ctx();
        }
        let device = Device::new(self.ctx(),
                                 &ctx.get().read().surface(),
                                 &GfxConfig {
                                     validation_layers: true,
                                     required_extensions: vec![vk::KHR_SWAPCHAIN_EXTENSION.name],
                                 }).unwrap();
        let ctx = device.ctx();
        *self.data.device.write().unwrap() = Some(device);
        ctx
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