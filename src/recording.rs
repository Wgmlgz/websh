// use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{self, Instant};

use anyhow::anyhow;
use anyhow::Result;
use bytes::Bytes;
// use ffmpeg_next::threading::Config;
// use ffmpeg_next::util::format;
use std::process::{Command, Stdio};
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;
use tokio_util::codec::{BytesCodec, FramedRead};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8, MIME_TYPE_VP9};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::media::io::ivf_reader::IVFReader;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::Error;

// use rustdesk::scrap as scrap;
// use ffmpeg::codec::packet::Packet;
// use ffmpeg::codec::{self, Context as CodecContext};
// use ffmpeg::format::Pixel;
// use ffmpeg::software::scaling::{flag::Flags, Context as SwsContext};
// use ffmpeg::util::frame::video::Video as FFrame;
// use ffmpeg_next::codec::{Id, Parameters};
// use ffmpeg_next::encoder::{Encoder, Video};
// use ffmpeg_next::{self as ffmpeg, Dictionary, Rational};
// use scrap::codec::{EncoderApi, EncoderCfg};
use tokio::runtime::Runtime;

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::Arc;
// use std::time::{
//     Duration,
//     // Instant
// };
use std::{io, thread};

// use docopt::Docopt;
use scrap::codec::{EncoderApi, EncoderCfg};
// use webm::mux;
// use webm::mux::Track;

use scrap::vpxcodec as vpx_encode;
use scrap::{Capturer, Display, TraitCapturer, STRIDE_ALIGN};

// Add a single video track
pub async fn add_video(pc: &Arc<RTCPeerConnection>) -> Result<()> {
    // let video_track = Arc::new(TrackLocalStaticSample::new(
    //     RTCRtpCodecCapability {
    //         mime_type: MIME_TYPE_VP8.to_owned(),
    //         ..Default::default()
    //     },
    //     format!("video-{}", rand::random::<u32>()),
    //     format!("video-{}", rand::random::<u32>()),
    // ));

    // ----------------------------
    // Suppose you already have a webrtc-rs TrackLocalStaticSample
    // We'll just mock it here as `t`. In real code, you set it up properly.
    // ----------------------------
    // let h264_codec_cap = webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
    //     mime_type: "video/h264".to_string(),
    //     ..Default::default()
    // };

    let video_track = Arc::new(TrackLocalStaticSample::new(
        // h264_codec_cap,
        RTCRtpCodecCapability {
            mime_type:
            //  if is_vp9 {
                // MIME_TYPE_VP9.to_owned(),
            // } else {
                MIME_TYPE_VP8.to_owned(),
            // },
            ..Default::default()
        },
        format!("video-{}", rand::random::<u32>()),
        format!("video-{}", rand::random::<u32>()),
    ));

    let rtp_sender = match pc
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => rtp_sender,
        Err(err) => panic!("{}", err),
    };

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    {
        tokio::spawn(async move {
            let res = write_video_to_track2(video_track).await;
            // write_video_to_track( video_track).await;
            dbg!(res);
        });
    }

    println!("Video track has been added");
    Ok(())
}

// Remove a single sender
async fn remove_video(pc: &Arc<RTCPeerConnection>) -> Result<()> {
    let senders = pc.get_senders().await;
    if !senders.is_empty() {
        if let Err(err) = pc.remove_track(&senders[0]).await {
            panic!("{}", err);
        }
    }

    println!("Video track has been removed");
    Ok(())
}
#[derive(Debug)]
struct CapturedFrame {
    data: Bytes,
    timestamp: Instant,
}

pub fn record(tx: mpsc::Sender<CapturedFrame>) -> Result<()> {
    use std::io::ErrorKind::WouldBlock;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mut displays = scrap::Display::all().unwrap();
    let display = displays.remove(0);
    let (width, height) = (display.width() as u32, display.height() as u32);

    let vpx_codec = scrap::vpxcodec::VpxVideoCodecId::VP8;

    // Setup the encoder (omitted error handling, see your code).
    let quality = 1.0;
    let mut vpx = scrap::vpxcodec::VpxEncoder::new(
        scrap::codec::EncoderCfg::VPX(scrap::vpxcodec::VpxEncoderConfig {
            width,
            height,
            quality,
            codec: vpx_codec,
            keyframe_interval: None,
        }),
        false,
    )
    .unwrap();

    let mut capturer = scrap::Capturer::new(display).unwrap();

    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();
    use scrap::Frame;
    use scrap::TraitPixelBuffer;
    // let child = Command::new("ffplay")
    //     .args(&[
    //         "-f",
    //         "rawvideo",
    //         "-pixel_format",
    //         "bgr0",
    //         "-video_size",
    //         &format!("{}x{}", width, height),
    //         "-framerate",
    //         "60",
    //         "-",
    //     ])
    //     .stdin(Stdio::piped())
    //     .spawn()
    //     .expect("This example requires ffplay.");
    // let mut out = child.stdin.unwrap();

    while !stop.load(Ordering::Acquire) {
        match capturer.frame(Duration::from_millis(0)) {
            Ok(frame) => {
                // Write the frame, removing end-of-row padding.
                // {
                //     let Frame::PixelBuffer(frame) = frame else {
                //         continue;
                //     };
                //     let stride = frame.stride()[0];
                //     let rowlen: usize = (4 * width).try_into().unwrap();
                //     for row in frame.data().chunks(stride) {
                //         let row = &row[..rowlen];
                //         out.write_all(row).unwrap();
                //     }
                // }

                let now = Instant::now();
                let elapsed_ms = (now - start).as_millis() as i64;

                // Convert BGRA -> YUV, then encode
                frame.to(vpx.yuvfmt(), &mut yuv, &mut mid_data).unwrap();
                let encoded_frames = vpx.encode(elapsed_ms, &yuv, scrap::STRIDE_ALIGN).unwrap();

                for enc_frame in encoded_frames {
                    let frame_data = Bytes::copy_from_slice(enc_frame.data);

                    // Send the encoded data + the capture timestamp
                    // (we’ll use `now` to measure the frame duration in the writer).
                    tx.blocking_send(CapturedFrame {
                        data: frame_data,
                        timestamp: now,
                    })
                    .unwrap();
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // Just skip if no frame is ready yet
            }
            Err(e) => {
                eprintln!("Capturer error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

pub async fn write_video_to_track2(track: Arc<TrackLocalStaticSample>) -> Result<()> {
    // Our channel will carry frames plus the timestamp
    let (tx, mut rx) = mpsc::channel::<CapturedFrame>(1);

    // Spawn blocking thread that reads from display
    tokio::task::spawn_blocking(|| {
        // If recording fails, you might want to handle that error more gracefully
        if let Err(e) = record(tx) {
            eprintln!("record() error: {:?}", e);
        }
    });

    // We will keep track of the previous Instant to measure the real duration
    let mut last_timestamp = None;

    while let Some(frame) = rx.recv().await {
        let duration = if let Some(prev_ts) = last_timestamp {
            frame.timestamp.duration_since(prev_ts)
        } else {
            // First frame, can treat as zero
            Duration::from_millis(0)
        };
        last_timestamp = Some(frame.timestamp);

        track
            .write_sample(&Sample {
                data: frame.data,
                duration,
                ..Default::default()
            })
            .await?;
    }

    Ok(())
}

