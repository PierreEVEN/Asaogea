use glam::{DVec2, Vec2};
use std::collections::HashMap;
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::Key;

#[derive(Default)]
pub struct MouseState {
    position: DVec2,
    scroll_delta: DVec2,
    buttons: HashMap<MouseButton, bool>,
}

#[derive(Default)]
pub struct KeyboardState {
    keys: HashMap<Key, bool>,
}

#[derive(Default)]
pub struct InputManager {
    mouse_state: MouseState,
    keyboard_state: KeyboardState,
}

impl InputManager {
    pub fn consume_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::ActivationTokenDone { .. } => {}
            WindowEvent::Resized(_) => {}
            WindowEvent::Moved(_) => {}
            WindowEvent::CloseRequested => {}
            WindowEvent::Destroyed => {}
            WindowEvent::DroppedFile(_) => {}
            WindowEvent::HoveredFile(_) => {}
            WindowEvent::HoveredFileCancelled => {}
            WindowEvent::Focused(_) => {}
            WindowEvent::KeyboardInput { event, .. } => {
                self.keyboard_state.keys.insert(event.logical_key.clone(), event.state.is_pressed());
            }
            WindowEvent::ModifiersChanged(_) => {}
            WindowEvent::Ime(_) => {}
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_state.position = DVec2::new(position.x, position.y)
            }
            WindowEvent::CursorEntered { .. } => {}
            WindowEvent::CursorLeft { .. } => {}
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        self.mouse_state.scroll_delta = DVec2::new(*x as f64 * 12f64, *y as f64 * 12f64)
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        self.mouse_state.scroll_delta = DVec2::new(delta.x, delta.y)
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_state.buttons.insert(button.clone(), state.is_pressed());
            }
            WindowEvent::PinchGesture { .. } => {}
            WindowEvent::PanGesture { .. } => {}
            WindowEvent::DoubleTapGesture { .. } => {}
            WindowEvent::RotationGesture { .. } => {}
            WindowEvent::TouchpadPressure { .. } => {}
            WindowEvent::AxisMotion { .. } => {}
            WindowEvent::Touch(_) => {}
            WindowEvent::ScaleFactorChanged { .. } => {}
            WindowEvent::ThemeChanged(_) => {}
            WindowEvent::Occluded(_) => {}
            WindowEvent::RedrawRequested => {}
        }
    }
    pub fn begin_frame(&mut self) {
        self.mouse_state.scroll_delta = DVec2::default();
    }

    pub fn is_key_pressed(&self, key: &Key) -> bool {
        if let Some(key) = self.keyboard_state.keys.get(key) { *key } else { false }
    }

    pub fn is_mouse_button_pressed(&self, key: &MouseButton) -> bool {
        if let Some(key) = self.mouse_state.buttons.get(key) { *key } else { false }
    }

    pub fn mouse_position(&self) -> &DVec2 {
        &self.mouse_state.position
    }

    pub fn scroll_delta(&self) -> &DVec2 {
        &self.mouse_state.scroll_delta
    }
}
