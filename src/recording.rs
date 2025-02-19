// use std::fs::File;
use std::io::{BufReader, Read, Write};
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
use tokio::runtime::Handle;
use tokio::task;
use std::io::ErrorKind::WouldBlock;
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

    while !stop.load(Ordering::Acquire) {
        match capturer.frame(Duration::from_millis(0)) {
            Ok(frame) => {
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
    // This bool can be used to stop the loop from another place if you want.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Clone track and stop-flag for the blocking thread:
    let track_clone = Arc::clone(&track);
    let stop_clone = Arc::clone(&stop_flag);

    // We need the current Tokio handle so we can call async `track.write_sample(...)`
    // from within the blocking code via `handle.block_on(...)`.
    let handle = Handle::current();

    // Spawn a blocking thread to do the capturing:
    task::spawn_blocking(move || {
        if let Err(e) = capture_loop(track_clone, stop_clone, &handle) {
            eprintln!("Capture loop error: {:?}", e);
        }
    });

    Ok(())
}

/// The actual capture/encode loop. Runs on a dedicated blocking thread.
/// For each encoded VPX frame, it does a blocking call to
/// `track.write_sample(...)`.
fn capture_loop(
    track: Arc<TrackLocalStaticSample>,
    stop: Arc<AtomicBool>,
    handle: &Handle,
) -> Result<()> {
    let mut displays = scrap::Display::all()?;
    let display = displays.remove(0);

    let (width, height) = (display.width() as u32, display.height() as u32);
    let mut capturer = Capturer::new(display)?;

    // Configure the VPX encoder
    let vpx_codec = scrap::vpxcodec::VpxVideoCodecId::VP8;
    let mut vpx = scrap::vpxcodec::VpxEncoder::new(
        scrap::codec::EncoderCfg::VPX(scrap::vpxcodec::VpxEncoderConfig {
            width,
            height,
            quality: 100000000000000.0,
            codec: vpx_codec,
            keyframe_interval: None,
        }),
        /* verbose = */ false,
    )?;

    // Buffers for BGRA->YUV conversion
    let mut yuv_buffer = Vec::new();
    let mut mid_data = Vec::new();

    // Track frame timing so we can set the duration
    let start_time = Instant::now();
    let mut last_instant: Option<Instant> = None;

    // Main capture loop
    while !stop.load(Ordering::Acquire) {
        match capturer.frame(Duration::from_millis(0)) {
            Ok(frame_bgra) => {
                let now = Instant::now();
                let elapsed_ms = (now - start_time).as_millis() as i64;

                // Convert BGRA -> YUV in-place
                frame_bgra.to(vpx.yuvfmt(), &mut yuv_buffer, &mut mid_data)?;

                // Encode into VPX
                let encoded_frames = vpx.encode(elapsed_ms, &yuv_buffer, STRIDE_ALIGN)?;

                // Calculate the real "wall-clock" duration between frames
                let duration = if let Some(prev) = last_instant {
                    now.duration_since(prev)
                } else {
                    Duration::from_millis(0)
                };
                last_instant = Some(now);

                // For each encoded frame, write to the webrtc track
                for enc_frame in encoded_frames {
                    // NOTE: track.write_sample() copies internally, so we can
                    // safely pass a reference or ephemeral buffer. This is
                    // about as close as we can get to “no copy”.
                    let sample = Sample {
                        data: Bytes::copy_from_slice(enc_frame.data),
                        duration,
                        ..Default::default()
                    };

                    // Block on the async call to `track.write_sample(...)`
                    let _ = handle.block_on(async {
                        track.write_sample(&sample).await
                    });
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // If no frame is ready, just sleep briefly to avoid spinning
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                eprintln!("Capture error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}