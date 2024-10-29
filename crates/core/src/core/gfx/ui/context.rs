use std::collections::HashMap;
use imgui::sys::{igCreateContext, igGetDrawData, igNewFrame, igRender, igSetCurrentContext, ImDrawData, ImFontAtlas};
use crate::core::gfx::ui::ui::Ui;
use crate::core::gfx::ui::window::Window;

pub struct ImGuiContext {
    raw: *mut imgui::sys::ImGuiContext,
    ui: Ui,
}

pub struct SuspendedContext {
    active: ImGuiContext,
}

impl SuspendedContext {
    pub fn activate(self) -> ImGuiContext {
        let active = self.active;
        unsafe { igSetCurrentContext(active.raw); }
        active
    }
}

impl ImGuiContext {
    pub fn raw(&self) -> &mut imgui::sys::ImGuiContext {
        unsafe { self.raw.as_mut().unwrap() }
    }

    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    pub fn new(shared_font_atlas: *mut ImFontAtlas) -> Self {
        let raw = unsafe {
            let raw = igCreateContext(shared_font_atlas);
            igSetCurrentContext(raw);
            raw
        };

        Self {
            raw,
            ui: Ui::new(),
        }
    }

    pub fn render(&self) -> &ImDrawData {
        unsafe { igRender(); }
        unsafe { &*(igGetDrawData()) }
    }

    pub fn new_frame(&self) {
        unsafe { igNewFrame(); }
    }

    pub fn suspend(self) -> SuspendedContext {
        SuspendedContext { active: self }
    }
}
