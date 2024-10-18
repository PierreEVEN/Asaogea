
#[derive(Clone)]
pub struct VulkanOptions {
    pub validation_layers: bool,
}

impl Default for VulkanOptions {
    fn default() -> Self {
        Self {
            validation_layers: false,
        }
    }
}

#[derive(Clone)]
pub struct WindowOptions {
    pub name: String,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            name: "Asaogea".to_string(),
        }
    }
}

#[derive(Default, Clone)]
pub struct Options {
    pub vulkan: VulkanOptions,
    pub windows: WindowOptions,
}