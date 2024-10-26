
#[derive(Clone)]
pub struct RenderingOption {
    pub validation_layers: bool,
    pub image_count: usize
}

impl Default for RenderingOption {
    fn default() -> Self {
        Self {
            validation_layers: true,
            image_count: 2,
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
    pub rendering: RenderingOption,
    pub main_window: WindowOptions,
}