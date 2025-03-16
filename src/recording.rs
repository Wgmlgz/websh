use anyhow::anyhow;
use anyhow::Result;
use bytes::Bytes;
use std::io::ErrorKind::WouldBlock;
use std::sync::Arc;
use std::time::{self, Instant};
use tokio::runtime::Handle;
use tokio::task;
use tokio::time::Duration;
use webrtc::api::media_engine::{
    MediaEngine, MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_VP8, MIME_TYPE_VP9,
};
use webrtc::media::Sample;

use scrap::codec::{EncoderApi, EncoderCfg};
use scrap::{Capturer, Display, TraitCapturer, STRIDE_ALIGN};
use std::sync::atomic::{AtomicBool, Ordering};
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

// GStreamer crates
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

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
                // MIME_TYPE_AV1.to_owned(),
                MIME_TYPE_H264.to_owned(),
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
        if let Err(e) = capture_loop_gstreamer(track_clone, stop_clone, &handle) {
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
            quality: 100.0,
            codec: vpx_codec,
            keyframe_interval: None,
        }),
        false,
    )
    .unwrap();

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
                    let _ = handle.block_on(async { track.write_sample(&sample).await });
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {}
            Err(e) => {
                eprintln!("Capture error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

fn capture_loop_gstreamer(
    track: Arc<TrackLocalStaticSample>,
    stop: Arc<AtomicBool>,
    handle: &Handle,
) -> Result<()> {
    // Initialize GStreamer

    // Example pipeline (30 fps capture):
    // dxgiscreencapturesrc ! video/x-raw,framerate=30/1 ! videoconvert ! vp8enc ! appsink
    // The appsink is where we'll pull the encoded frames.
    //
    // Adjust to your needs (bitrate, other enc parameters, etc.)
    let pipeline_str = r#"
    d3d11screencapturesrc
        ! video/x-raw,framerate=60/1
        ! videoscale
        ! video/x-raw,width=[1,1920],height=[1,1080]
        ! videoconvert
        ! nvh264enc
            preset=p1
            tune=ultra-low-latency
            zerolatency=true
        ! h264parse
            config-interval=-1 
        ! appsink name=appsink emit-signals=true sync=false
    "#;

    // Build and downcast to a Pipeline
    let pipeline = gst::parse::launch(pipeline_str)?;
    let pipeline = pipeline
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("Failed to cast parsed element to Pipeline"))?;

    // Retrieve the appsink by name
    let appsink = pipeline
        .by_name("appsink")
        .ok_or_else(|| anyhow!("Failed to find appsink in pipeline"))?
        .dynamic_cast::<gst_app::AppSink>()
        .map_err(|_| anyhow!("Failed to cast element to AppSink"))?;

    // Start playing
    pipeline.set_state(gst::State::Playing)?;

    let start_time = Instant::now();
    let mut last_instant: Option<Instant> = None;

    // Pull encoded samples from the appsink in a loop
    while !stop.load(Ordering::Acquire) {
        // This will block until a new sample is ready, or end-of-stream/error
        match appsink.pull_sample() {
            Err(e) => {
                dbg!(e);
                // The pipeline might have hit EOS or an error
                break;
            }
            Ok(gst_sample) => {
                // Compute duration between frames for the webrtc::Sample
                let now = Instant::now();
                let duration = if let Some(prev) = last_instant {
                    now.duration_since(prev)
                } else {
                    now.duration_since(start_time) // or 0 if you prefer
                };
                last_instant = Some(now);

                // Extract the actual encoded buffer
                let buffer = gst_sample
                    .buffer()
                    .ok_or_else(|| anyhow!("Failed to get buffer from GStreamer sample"))?;

                // Map it as read-only to get the encoded data
                let map = buffer
                    .map_readable()
                    .map_err(|_| anyhow!("Failed to map GStreamer buffer as readable"))?;

                // Copy into a Bytes (webrtc-rs requires Bytes for the sample data)
                let encoded_bytes = Bytes::copy_from_slice(map.as_slice());

                // Construct a webrtc::Sample
                let sample = Sample {
                    data: encoded_bytes,
                    duration,
                    ..Default::default()
                };

                // Because we're in a blocking thread, we must "block_on" the async write.
                // If you prefer, you can queue these samples and send them on an async pipeline, etc.
                if let Err(e) = handle.block_on(async { track.write_sample(&sample).await }) {
                    eprintln!("Failed to write sample to track: {:?}", e);
                }
            }
        }
    }

    // Cleanup
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}
