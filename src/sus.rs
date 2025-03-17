use anyhow::Result;
use virtual_display::VirtualDisplayManager;
// pub mod recording;

// use recording::write_video_to_track;

#[tokio::main]
async fn main() -> Result<()> {
    let manager = VirtualDisplayManager::new().await?;
    println!("Doing other work...");
    // display.

    manager.update_display(0, Some(1920), Some(1080), Some(120)).await?;

    // The display remains active while 'display' is in scope.
    // Once we exit or drop 'display', the background thread ends and
    // the OS should remove the virtual display.

    // Sleep or do real work. We'll just sleep 10 secs here:
    std::thread::sleep(std::time::Duration::from_secs(10));

    println!("Dropping VirtualDisplay explicitly...");
    manager.exit().await?;

    println!("Now the program ends, display should be removed soon.");
    // write_video_to_track().await?;

    Ok(())
}
