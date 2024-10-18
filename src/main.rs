use anyhow::Error;
use core::window::AppWindow;


fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().init();
    let mut window = AppWindow::default();
    window.run()?;
    Ok(())
}