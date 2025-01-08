// src/lib.rs

#[cfg(target_os = "macos")]
mod macos_impl;

#[cfg(target_os = "macos")]
pub use macos_impl::VirtualDisplay;

// Non-macOS stub
#[cfg(not(target_os = "macos"))]
pub struct VirtualDisplay;

#[cfg(not(target_os = "macos"))]
impl VirtualDisplay {
    pub fn new() -> Self {
        println!("VirtualDisplay is only supported on macOS.");
        Self
    }
}
