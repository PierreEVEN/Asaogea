use crate::core::gfx::ui::ui::Ui;

pub trait Window {
    fn render(&self, ui: &mut Ui);
    fn get_name(&self) -> String;
    fn keep_open(&self) -> bool;
    fn close(&self);
}

struct ProfilerWindow {
    
}