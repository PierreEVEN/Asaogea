use anyhow::Error;
use core::engine::Engine;
use core::options::WindowOptions;

#[no_mangle]
#[link_section = ".data"]
pub static NvOptimusEnablement: u32 = 0x00000001;

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut engine = Engine::new(WindowOptions { name: "Asaogea".to_string() })?;
    engine.run()
}