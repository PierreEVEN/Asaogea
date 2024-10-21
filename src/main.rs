
use anyhow::Error;
use core::engine::Engine;
use core::options::{WindowOptions};

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut engine = Engine::new(WindowOptions { name: "Asaogea".to_string() })?;
    engine.run()
}