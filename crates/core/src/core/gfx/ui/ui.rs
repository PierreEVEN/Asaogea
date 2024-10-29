use std::collections::HashMap;
use imgui::sys::{igBegin, igEnd, ImGuiWindowFlags};
use types::resource_handle::Resource;
use crate::core::gfx::ui::window::Window;

pub struct Ui {
    windows: HashMap<String, Vec<Resource<Box<dyn Window>>>>,
}

impl Ui {
    pub fn new() -> Self {
        Self { windows: Default::default() }
    }

    pub fn begin(&self, name: &str, open: &mut bool, flags: ImGuiWindowFlags) -> bool {
        unsafe { igBegin(name.as_ptr() as *const imgui::sys::cty::c_char, open as *mut bool, flags) }
    }

    pub fn end(&self) {
        unsafe { igEnd() }
    }

    pub fn open_window<T: 'static + Window>(&mut self, window: T) {
        self.windows.insert(window.get_name(), vec![Resource::new(Box::new(window))]);
    }

    pub fn render_window(&mut self) {
        let window_id = 0;

        for (name, window) in &self.windows {
            window.render(self);
        }
    }

    pub fn close_window(&mut self, window: &dyn Window) {
        self.windows.remove(&window.get_name());
    }
}