use std::collections::HashSet;
use std::ffi::c_void;
use std::ops::Deref;
use anyhow::Error;
use tracing::{debug, error, trace, warn};
use vulkanalia::{vk, Entry};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::vk::{EntryV1_0, HasBuilder};
use vulkanalia::window as vk_window;
use winit::window::Window;
use types::rwarc::RwArc;
use crate::device::{Device, PhysicalDevice};
use crate::surface::Surface;

pub(crate) const VALIDATION_LAYER: vk::ExtensionName = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

pub struct GfxConfig {
    pub validation_layers: bool,
    pub required_extensions: Vec<vk::ExtensionName>,
}

pub struct Instance {
    instance: vulkanalia::Instance,
    surface: RwArc<Surface>,
    entry: Entry,
    device: Device,
}

impl Instance {
    pub fn new(config: &mut GfxConfig, window: &Window) -> Result<Self, Error> {
        let entry = unsafe {
            let loader = LibloadingLoader::new(LIBRARY)?;
            Entry::new(loader).map_err(|b| anyhow::anyhow!("{}", b))?
        };
        // Required extensions
        let mut extensions = vk_window::get_required_instance_extensions(window)
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();
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
            .api_version(vk::make_version(1, 0, 0));
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

        let instance = unsafe {

            // Create
            entry.create_instance(&info, None)?
        };

        let mut surface = Surface::new(&instance, window)?;
        let device = Device::new(&instance, &surface, &config)?;
        let mut instance = Self { instance, entry, device, surface: RwArc::new(surface) };
        instance.surface.write().create_or_recreate_swapchain(&instance, &window)?;
        Ok(instance)
    }

    pub fn surface(&self) -> &RwArc<Surface> {
        &self.surface
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn device_mut(&mut self) -> &mut Device {
        &mut self.device
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.surface.write().destroy(&self, &self.device);
        self.device.destroy();
    }
}

impl Deref for Instance {
    type Target = vulkanalia::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
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
        panic!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        debug!("({:?}) {}", type_, message);
    } else {
        trace!("({:?}) {}", type_, message);
    }

    vk::FALSE
}