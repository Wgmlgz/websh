use bytes::Bytes;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::thread;
use tokio::sync::mpsc::Receiver;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct Session {
    pub to_pty: mpsc::Sender<Bytes>,        // To send data to PTY
    pub from_pty: broadcast::Sender<Bytes>, // To receive data from PTY
    pub done_tx: mpsc::Sender<()>,           // To signal done
}

pub type SessionMap = Arc<Mutex<HashMap<String, Session>>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ShellMsg {
    pub resize: Option<PtySize>,
    pub input: Option<String>,
    pub output: Option<String>,
}

pub async fn handle_pty(
    tx: broadcast::Sender<Bytes>,  // From PTY to clients
    mut rx: mpsc::Receiver<Bytes>, // From clients to PTY
    mut done_rx: Receiver<()>,
) {
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

                        let json = serde_json::to_string(&ShellMsg {
                            resize: None,
                            input: None,
                            output: Some(msg),
                        })
                        .unwrap();

                        // Use blocking send to the async channel (note this can block the thread if the channel is full)
                        match tx.send(json.into()) {
                            Ok(_) => (),
                            Err(e) => {
                                log::error!("Failed to send message to async context: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read from PTY: {}", e);
                        break;
                    }
                }
            }
        });
    });

    // Asynchronously receive messages and write to PTY
    tokio::spawn(async move {
        while let Some(json) = rx.recv().await {
            let sus = json.to_vec();
            let sus = sus.as_slice();
            let msg: ShellMsg =
                serde_json::from_str(String::from_utf8_lossy(sus).to_string().as_str()).unwrap();
            if let Some(msg) = msg.input {
                writer.write_all(msg.as_bytes()).unwrap();
                writer.flush().unwrap();
            };

            if let Some(size) = msg.resize {
                pair.master.resize(size).unwrap();
            };
        }
    });

    // Wait for the child process to exit
    thread::spawn(move || {
        let _ = child.wait();
    });

    tokio::select! {
        _ = done_rx.recv() => {
            log::info!("Received done signal");
        }
        // _ = tokio::signal::ctrl_c() => {
        //     log::info!("Received ctrlc");
        // }
    };

    // loop {
    //     tokio::time::sleep(Duration::from_millis(1000)).await;
    //     dbg!("pending");
    // }
}
