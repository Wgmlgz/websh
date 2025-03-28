use std::default;
use std::sync::Arc;

use bytes::Bytes;
// use crossbeam::utils;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use virtual_display::VirtualDisplayManager;

use crate::recording::add_video;
use crate::signal::Signaling;
use crate::state::State;
use crate::utils::to_json;
use anyhow::{anyhow, Result};
use tokio::sync::{broadcast, mpsc};
use ts_rs::TS;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlMsg {
    id: i32,
    body: ControlMsgBody,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ControlMsgBody {
    #[default]
    Empty,
    StartVideo(StartVideoMsg),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
pub struct ErrorMsg {
    msg: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ControlResBody {
    #[default]
    Empty,
    Error(ErrorMsg),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ControlResMsg {
    id: i32,
    body: ControlResBody,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, TS)]
pub struct StartVideoMsg {
    display_id: u32,
    width: Option<u32>,
    height: Option<u32>,
    refresh_rate: Option<u32>,
}

fn parse_msg(json: Bytes) -> Result<ControlMsg> {
    let sus = json.to_vec();
    let sus = sus.as_slice();
    let msg: ControlMsg = serde_json::from_str(String::from_utf8_lossy(sus).to_string().as_str())?;
    Ok(msg)
}

impl<T> State<T>
where
    T: Signaling + std::marker::Sync + 'static,
{
    async fn control_inner_loop(
        manager: Arc<VirtualDisplayManager>,
        pc: Arc<RTCPeerConnection>,
        msg: ControlMsg,
        mut done_rx: broadcast::Receiver<()>,
    ) -> Result<ControlResBody> {
        match msg.body {
            ControlMsgBody::Empty => todo!(),
            ControlMsgBody::StartVideo(start_video_msg) => {
                dbg!(&"sus");
                let id = start_video_msg.display_id;
                manager
                    .update_display(
                        id,
                        start_video_msg.width,
                        start_video_msg.height,
                        start_video_msg.refresh_rate,
                    )
                    .await?;
                let p: Arc<RTCPeerConnection> = pc.clone();
                let mut done_rx_clone = done_rx.resubscribe();
                tokio::spawn(async move {
                    if let Err(e) = done_rx_clone.recv().await.map_err(|e| anyhow!(e)).and_then(|_| {
                        tokio::spawn(async move {
                            if let Err(e) = manager.remove_display(id).await {
                                log::error!("Failed to remove display: {}", e);
                            }
                        });
                        Ok(())
                    }) {
                        log::error!("Some error idc 1: {}", e);
                    }
                });
                add_video(p, start_video_msg, done_rx).await?;
            }
        }

        Ok(ControlResBody::Empty)
    }

    pub async fn handle_control(
        self: Arc<Self>,
        pc: Arc<RTCPeerConnection>,
        tx: broadcast::Sender<Bytes>,  // From server to clients
        mut rx: mpsc::Receiver<Bytes>, // From clients to server
        mut done_rx: broadcast::Receiver<()>,
    ) {
        let manager = self.display_manager.clone();

        let done_rx_copy = done_rx.resubscribe();

        tokio::spawn(async move {
            while let Some(json) = rx.recv().await {
                let manager = manager.clone();
                let done_rx = done_rx_copy.resubscribe();
                let parse_res = parse_msg(json);

                let Ok(msg) = parse_res else {
                    log::error!(
                        "Failed to send return message: {}",
                        parse_res.unwrap_err().to_string()
                    );
                    continue;
                };
                let id = msg.id;
                dbg!("ok nig");
                let res =
                    Self::control_inner_loop(manager, pc.clone(), msg, done_rx.resubscribe()).await;

                let res = match res {
                    Ok(res) => res,
                    Err(err) => ControlResBody::Error(ErrorMsg {
                        msg: err.to_string(),
                    }),
                };

                dbg!(&res);
                if let Err(e) =
                    to_json(&ControlResMsg { id, body: res }).and_then(|json| -> Result<usize> {
                        tx.send(json.into()).map_err(|e| anyhow!(e))
                    })
                {
                    log::error!("Failed to send return message: {}", e);
                }
            }
        });

        tokio::select! {
            _ = done_rx.recv() => {
                log::info!("Received done signal");
            }
        };
    }
}
