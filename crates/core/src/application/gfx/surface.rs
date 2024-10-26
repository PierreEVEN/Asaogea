use crate::application::gfx::instance::InstanceCtx;
use anyhow::{anyhow, Error};
use vulkanalia::vk;
use vulkanalia::vk::{HasBuilder, KhrSurfaceExtension, KhrWin32SurfaceExtension, SurfaceKHR, HINSTANCE};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use types::resource_handle::{Resource, ResourceHandle};

pub type SurfaceCtx = ResourceHandle<Surface>;

pub struct Surface {
    surface: SurfaceKHR,
    instance: InstanceCtx
}

impl Surface {
    pub fn new(ctx: InstanceCtx, window: &Window) -> Result<Resource<Self>, Error> {
        let surface = match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => {
                let hinstance = match handle.hinstance {
                    None => { return Err(anyhow!("Invalid hinstance")) }
                    Some(hinstance) => { hinstance }
                };
                let info = vk::Win32SurfaceCreateInfoKHR::builder()
                    .hinstance(hinstance.get() as HINSTANCE)
                    .hwnd(handle.hwnd.get() as HINSTANCE);
                unsafe { ctx.get().instance().create_win32_surface_khr(&info, None) }?
            }
            value => {
                return Err(anyhow!("Unsupported window platform : {:?}", value));
            }
        };

        Ok(Resource::new(Self {
            surface,
            instance: ctx,
        }))
    }

    pub fn ptr(&self) -> &SurfaceKHR {
        &self.surface
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.instance.get().instance().destroy_surface_khr(self.surface, None); }
    }
}