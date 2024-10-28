use crate::core::window::{WindowCtx, WindowCtxMut};
use crate::engine::EngineCtx;

pub trait Application {
    fn instantiate(&mut self, engine: &mut WindowCtxMut);
    fn create_window(&mut self, window: &mut WindowCtxMut);
    fn pre_draw_window(&mut self, engine: &WindowCtx);
    fn tick(&mut self, engine: &EngineCtx);
    fn destroy(&mut self);
}