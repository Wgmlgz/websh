// build.rs

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn macos() {
    println!("cargo:rerun-if-changed=CGVirtualDisplayPrivate.h");

    let sdk_path = Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .expect("Failed to run xcrun")
        .stdout;
    let sdk_path = String::from_utf8(sdk_path).unwrap().trim().to_owned();
    let frameworks_path = format!("{}/System/Library/Frameworks", sdk_path);

    let builder = bindgen::Builder::default()
        .header("CGVirtualDisplayPrivate.h")
        .objc_extern_crate(true)
        .clang_arg("-x")
        .clang_arg("objective-c")
        .clang_arg("-fobjc-arc")
        .clang_arg("-fblocks")
        .clang_arg(format!("-isysroot{}", sdk_path))
        .clang_arg(format!("-F{}", frameworks_path))
        .clang_arg("-framework")
        .clang_arg("Cocoa")
        .clang_arg("-framework")
        .clang_arg("Foundation")
        .allowlist_type("CGVirtualDisplay.*")
        .allowlist_function("CGDisplayMoveCursorToPoint");

    let bindings = builder.generate().expect("Unable to generate bindings");

    // Convert to string
    let code = bindings.to_string();

    // A naive single-line replacement might fail if there's extra whitespace or a newline.
    // Let's do a more robust approach by line:
    let mut processed_code = Vec::new();
    for line in code.lines() {
        // If the line starts with `unsafe extern "C"` or contains it,
        // remove the "unsafe " part
        let fixed = line.replace("unsafe extern \"C\"", "extern \"C\"");
        processed_code.push(fixed);
    }
    let final_code = processed_code.join("\n");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    std::fs::write(&out_path, final_code).expect("Couldn't write bindings!");
}

fn windows() {}
fn main() {
    if cfg!(target_os = "macos") {
        macos();
        return;
    }
    if cfg!(target_os = "windows") {}
}
