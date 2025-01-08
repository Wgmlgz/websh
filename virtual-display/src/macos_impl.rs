// src/macos_impl.rs (only compiled on macOS)
use cocoa::base::{id, nil};
use cocoa::foundation::{NSArray, NSAutoreleasePool, NSString};
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// Link to macOS frameworks
#[link(name = "Cocoa", kind = "framework")]
extern "C" {}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {}

// The bindgen-generated code:
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct VirtualDisplay {
    // Keep track of the background thread handle
    thread_handle: Option<JoinHandle<()>>,

    // We might store data we need to ensure doesn't get dropped
    // until this struct is dropped. For instance, references to
    // the descriptor or display object. We'll store them in an Arc
    // so they remain valid for the background thread's lifetime.
    state: Arc<Mutex<VirtualDisplayState>>,
}

/// A simple wrapper for an ObjC pointer. By implementing Send (and maybe Sync),
/// we assert that it's safe to move this across threads. This is unsafe if
/// the underlying object is not truly thread-safe.
#[derive(Debug)]
pub struct ObjcPtr(*mut Object);

impl ObjcPtr {
    pub fn new(ptr: *mut Object) -> Self {
        ObjcPtr(ptr)
    }

    pub fn as_ptr(&self) -> *mut Object {
        self.0
    }
}

// We claim it's Send. If the underlying ObjC object is not thread-safe, this is UB.
unsafe impl Send for ObjcPtr {}
// Whether you also need Sync depends on your usage. Usually you might not want it unless you truly read it from multiple threads.

pub struct VirtualDisplayState {
    descriptor: ObjcPtr,
    display: ObjcPtr,
    settings: ObjcPtr,
}

impl VirtualDisplay {
    pub fn new(display_name: &str) -> Self {
        // Create an autorelease pool (short-lived, just for creation)
        let _pool = unsafe { NSAutoreleasePool::new(nil) };

        // 1) CGVirtualDisplayDescriptor
        let descriptor_cls =
            Class::get("CGVirtualDisplayDescriptor").expect("CGVirtualDisplayDescriptor not found");
        let descriptor: *mut Object = unsafe {
            let alloc: *mut Object = msg_send![descriptor_cls, alloc];
            msg_send![alloc, init]
        };

        // Configure descriptor
        unsafe {
            let name = NSString::alloc(nil).init_str(display_name);
            let _: () = msg_send![descriptor, setName: name];
            let _: () = msg_send![descriptor, setMaxPixelsWide: 1920u32];
            let _: () = msg_send![descriptor, setMaxPixelsHigh: 1080u32];

            let size = core_graphics::geometry::CGSize::new(1600.0, 900.0);
            let _: () = msg_send![descriptor, setSizeInMillimeters: size];

            let _: () = msg_send![descriptor, setProductID: 0x1234u32];
            let _: () = msg_send![descriptor, setVendorID: 0x3456u32];
            let _: () = msg_send![descriptor, setSerialNum: 0x0001u32];
        }

        // 2) CGVirtualDisplay
        let display_cls = Class::get("CGVirtualDisplay").expect("CGVirtualDisplay not found");
        let display: *mut Object = unsafe {
            let alloc: *mut Object = msg_send![display_cls, alloc];
            msg_send![alloc, initWithDescriptor: descriptor]
        };

        // 3) CGVirtualDisplaySettings
        let settings_cls =
            Class::get("CGVirtualDisplaySettings").expect("CGVirtualDisplaySettings not found");
        let settings: *mut Object = unsafe {
            let alloc: *mut Object = msg_send![settings_cls, alloc];
            msg_send![alloc, init]
        };

        unsafe {
            // hiDPI = 1
            let _: () = msg_send![settings, setHiDPI: 1u32];
        }

        // 4) CGVirtualDisplayMode (1920x1080@60)
        let mode_cls = Class::get("CGVirtualDisplayMode").expect("CGVirtualDisplayMode not found");
        let mode: *mut Object = unsafe {
            let alloc: *mut Object = msg_send![mode_cls, alloc];
            msg_send![alloc, initWithWidth:1920u64
                                    height:1080u64
                               refreshRate:60.0f64]
        };

        // Put it in an NSArray
        let modes_array: *mut Object = unsafe { NSArray::arrayWithObjects(nil, &[mode]) };

        // settings.modes = [mode]
        unsafe {
            let _: () = msg_send![settings, setModes: modes_array];
        }

        // 5) applySettings
        let success: bool = unsafe { msg_send![display, applySettings: settings] };
        println!("applySettings success? {}", success);

        // 6) Get the displayID
        let display_id: CGDirectDisplayID = unsafe { msg_send![display, displayID] };
        println!("Created virtual display. ID = {}", display_id);

        // Prepare the state
        // Then in your constructor:
        let descriptor_ptr = ObjcPtr::new(descriptor);
        let display_ptr = ObjcPtr::new(display);
        let settings_ptr = ObjcPtr::new(settings);

        let state = VirtualDisplayState {
            descriptor: descriptor_ptr,
            display: display_ptr,
            settings: settings_ptr,
        };

        let shared_state = Arc::new(Mutex::new(state));

        // 7) Spawn a background thread that runs a CFRunLoop or something similar
        //    to keep the display alive. We'll do a simple CFRunLoop approach:
        let thread_state = Arc::clone(&shared_state);
        let handle = thread::spawn(move || {
            // We can create an autorelease pool on this thread
            let _pool = unsafe { NSAutoreleasePool::new(nil) };
            println!("Virtual display run loop started.");

            // This is the simplest approach: run a "loop" until the main struct is dropped.
            // We'll poll a mutex or some atomic flag if we want to exit.
            loop {
                // You could call CFRunLoopRunInMode here, or do event dispatching, etc.
                // For demonstration, just sleep and keep the thread alive.
                thread::sleep(Duration::from_secs(1));

                // dbg!(Arc::strong_count(&thread_state));
                // If we wanted a condition to break out, we'd check some shared atomic/flag
                if Arc::strong_count(&thread_state) == 1 {
                    // Means the main VirtualDisplay was dropped
                    println!("No more references; run loop thread exiting.");
                    break;
                }
            }

            // When we exit the loop, the autorelease pool drains, releasing the display
            println!("Virtual display run loop ended.");
        });

        VirtualDisplay {
            thread_handle: Some(handle),
            state: shared_state,
        }
    }
}
