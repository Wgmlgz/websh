use anyhow::Result;
use virtual_display::VirtualDisplay;
// pub mod recording;

// use recording::write_video_to_track;

#[tokio::main]
async fn main() -> Result<()> {
    let display = VirtualDisplay::new("Rust Virtual Display");
    println!("Doing other work...");

    // The display remains active while 'display' is in scope.
    // Once we exit or drop 'display', the background thread ends and
    // the OS should remove the virtual display.

    // Sleep or do real work. We'll just sleep 10 secs here:
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("Dropping VirtualDisplay explicitly...");
    drop(display);

    println!("Now the program ends, display should be removed soon.");
    // write_video_to_track().await?;

    Ok(())
}
