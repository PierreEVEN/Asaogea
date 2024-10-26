use anyhow::Error;
use core::engine::Engine;
use core::options::{Options, WindowOptions, RenderingOption};

#[no_mangle]
#[link_section = ".data"]
pub static NvOptimusEnablement: u32 = 0x00000001;

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut engine = Engine::new(Options {
        rendering: RenderingOption {
            validation_layers: true,
            image_count: 2,
        },
        main_window: WindowOptions {
            name: "Asaogea".to_string()
        },
    })?;
    engine.run()
}