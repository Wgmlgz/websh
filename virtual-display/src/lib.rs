// src/lib.rs

#[cfg(target_os = "macos")]
mod macos_impl;

#[cfg(target_os = "macos")]
pub use macos_impl::VirtualDisplay;



#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(target_os = "windows")]
pub use windows_impl::VirtualDisplayManager;

// // Non-macOS stub
// #[cfg(not(target_os = "macos"))]
// pub struct VirtualDisplay;

// #[cfg(and(not(target_os = "macos"), not(target_os = "windows")))]
// impl VirtualDisplay {
//     pub fn new() -> Self {
//         println!("VirtualDisplay is only supported on macOS.");
//         Self
//     }
// }
