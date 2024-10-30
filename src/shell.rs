use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;

pub async fn handle_pty(tx: Sender<String>, mut rx: Receiver<String>) {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let mut child = pair
        .slave
        .spawn_command(CommandBuilder::new(if cfg!(target_os = "windows") {
            "powershell"
        } else {
            "sh"
        }))
        .unwrap();

    let reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let reader = Arc::new(Mutex::new(reader));

    // Asynchronously read from PTY and send to main thread
    tokio::spawn(async move {
        let reader_clone = Arc::clone(&reader);

        // Run the entire reading and message sending in a blocking task
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 1024];
            loop {
                let read_result = {
                    let mut reader_guard = reader_clone.lock().unwrap();
                    reader_guard.read(&mut buf)
                };

                match read_result {
                    Ok(n) if n == 0 => continue,
                    Ok(n) => {
                        let msg = String::from_utf8_lossy(&buf[..n]).to_string();
                        // Use blocking send to the async channel (note this can block the thread if the channel is full)
                        match tx.blocking_send(msg) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!("Failed to send message to async context: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read from PTY: {}", e);
                        break;
                    }
                }
            }
        });
    });

    // Asynchronously receive messages and write to PTY
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            writer.write_all(msg.as_bytes()).unwrap();
            writer.flush().unwrap();
        }
    });

    // Wait for the child process to exit
    thread::spawn(move || {
        let _ = child.wait();
    });

    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        dbg!("pending");
    }
}
