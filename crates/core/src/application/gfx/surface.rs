use std::sync::Weak;
use crate::application::gfx::device::DeviceSharedData;
use anyhow::{anyhow, Error};
use vulkanalia::vk::{DeviceV1_0, Handle, HasBuilder, KhrSurfaceExtension, KhrWin32SurfaceExtension, SurfaceKHR, HINSTANCE};
use vulkanalia::vk;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub struct Surface {
    surface: SurfaceKHR,
    instance: Weak<vulkanalia::Instance>
}

impl Surface {
    pub fn new(instance: Weak<vulkanalia::Instance>, window: &Window) -> Result<Self, Error> {
        let surface = match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => {
                let hinstance = match handle.hinstance {
                    None => { return Err(anyhow!("Invalid hinstance")) }
                    Some(hinstance) => { hinstance }
                };
                let info = vk::Win32SurfaceCreateInfoKHR::builder()
                    .hinstance(hinstance.get() as HINSTANCE)
                    .hwnd(handle.hwnd.get() as HINSTANCE);
                unsafe { instance.upgrade().unwrap().create_win32_surface_khr(&info, None) }?
            }
            value => {
                return Err(anyhow!("Unsupported window platform : {:?}", value));
            }
        };

        Ok(Self {
            surface,
            instance,
        })
    }

    pub fn ptr(&self) -> &SurfaceKHR {
        &self.surface
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.instance.upgrade().unwrap().destroy_surface_khr(self.surface, None); }
    }
}