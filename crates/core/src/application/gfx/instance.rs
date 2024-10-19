use std::collections::HashSet;
use std::ffi::c_void;
use anyhow::{anyhow, Error};
use tracing::{debug, error, trace, warn};
use vulkanalia::{vk, Entry};
use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::vk::{EntryV1_0, HasBuilder};
use vulkanalia::window as vk_window;
use crate::application::window::{CtxAppWindow};

pub(crate) const VALIDATION_LAYER: vk::ExtensionName = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

pub struct GfxConfig {
    pub validation_layers: bool,
    pub required_extensions: Vec<vk::ExtensionName>,
}

pub struct Instance {
    instance: Option<vulkanalia::Instance>,
    _entry: Entry,
}

impl Instance {
    pub fn new(ctx: &CtxAppWindow, config: &mut GfxConfig) -> Result<Self, Error> {
        let entry = unsafe {
            let loader = LibloadingLoader::new(LIBRARY)?;
            Entry::new(loader).map_err(|b| anyhow::anyhow!("{}", b))?
        };
        // Required extensions
        let mut extensions = vk_window::get_required_instance_extensions(ctx.window.ptr()?)
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

        let instance = unsafe {entry.create_instance(&info, None)?};

        let instance = Self { instance: Some(instance), _entry: entry };
        Ok(instance)
    }

    pub fn ptr(&self) -> Result<&vulkanalia::Instance, Error> {
        self.instance.as_ref().ok_or(anyhow!("Instance have been destroyed"))
    }

    pub fn destroy(&mut self) -> Result<(), Error> {
        self.instance = None;
        Ok(())
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if self.instance.is_some() {
            panic!("Instance have not been destroyed using Instance::destroy()");
        }
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